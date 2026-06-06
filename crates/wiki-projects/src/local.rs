use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use wiki_db::entity::project;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::{DomainError, DomainResult};
use wiki_domain::pages::metadata::{Frontmatter, Infobox, InfoboxTab};
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{ContentFileTree, FileType, ProjectPage};
use wiki_domain::project::{
    FileTree, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project,
};
use wiki_domain::response::{ProjectInfo, ProjectLicense, ProjectLicenses, ProjectVersionData};
use wiki_storage::error::StorageResult;
use wiki_storage::format::{ProjectFormat, create_project_format};
use wiki_storage::git as git_provider;
use wiki_storage::ingestor::markdown::collect_links;
use wiki_storage::ingestor::recipes::types::StubRecipeType;
use wiki_system::DEFAULT_LOCALE;

use crate::links::resolve_page_links;
use crate::recipe_resolver::RecipeResolver;
use crate::recipe_types::{resolve_content_usage, resolve_workbenches};
use crate::resolver::ProjectResolver;

pub struct LocalProject {
    record: project::Model,
    format: Arc<dyn ProjectFormat>,
    repo: Arc<ProjectRepo>,
    resolver: Arc<ProjectResolver>,
    locale: Option<String>,
}

fn merge_infobox(default: Infobox, user: Option<Infobox>) -> Infobox {
    let Some(user) = user else {
        return default;
    };
    let inventory = if user.inventory.is_empty() {
        default.inventory
    } else {
        user.inventory
    };
    Infobox {
        title: user.title.or(default.title),
        tabs: user.tabs.or(default.tabs),
        inventory,
    }
}

fn count_pages(tree: &FileTree) -> u64 {
    tree.iter()
        .map(|e| match e.r#type {
            FileType::File => 1,
            FileType::Dir => count_pages(&e.children),
        })
        .sum()
}

impl LocalProject {
    pub fn new(
        record: project::Model,
        checkout_path: PathBuf,
        repo: Arc<ProjectRepo>,
        resolver: Arc<ProjectResolver>,
        locale: Option<String>,
    ) -> StorageResult<Self> {
        let format = create_project_format(checkout_path, locale.clone())?;
        Ok(Self {
            record,
            format,
            repo,
            resolver,
            locale,
        })
    }

    async fn build_default_infobox(&self, frontmatter: &Frontmatter) -> Infobox {
        let ids: &[String] = &frontmatter.id;

        let mut tabs = Vec::with_capacity(ids.len());
        for id in ids {
            let name = self.item_name(id).await.map(|d| d.name).unwrap_or_default();

            tabs.push(InfoboxTab {
                name,
                display: vec![id.clone()],
            });
        }

        // Use FM icon for single-item pages
        if tabs.len() == 1
            && let Some(ref icon) = frontmatter.icon
        {
            tabs[0].display = vec![icon.clone()];
        }

        Infobox {
            title: frontmatter.title.clone(),
            tabs: Some(tabs),
            inventory: ids.to_vec(),
        }
    }

    async fn read_item_properties(
        &self,
        ids: &[String],
    ) -> DomainResult<HashMap<String, HashMap<String, serde_json::Value>>> {
        let path = self.format.item_properties_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(HashMap::default());
        };
        let parsed: HashMap<String, HashMap<String, serde_json::Value>> =
            match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    warn!(path = %path.display(), "invalid item properties json: {e}");
                    return Ok(HashMap::default());
                }
            };

        let mut out = HashMap::with_capacity(ids.len());
        for id in ids {
            let Some(props) = parsed.get(id) else {
                continue;
            };
            out.insert(id.clone(), props.clone());
        }
        Ok(out)
    }

    async fn read_page(&self, page_path: &Path) -> DomainResult<(ProjectPage, Frontmatter)> {
        let raw = self.format.read_page(page_path)?;

        let mut frontmatter = raw.frontmatter;
        frontmatter.title = self.format.read_page_title_at(&frontmatter, page_path);

        let raw_links = collect_links(&raw.tree);
        let builtin = self.resolver.builtin().await?;
        let links = resolve_page_links(
            self.format.as_ref(),
            &self.repo,
            self,
            builtin.as_ref(),
            self.record.modid.as_deref(),
            &raw_links,
        )
        .await?;

        let edit_path = self.format.rel_path_with_ext(page_path);
        let edit_url = git_provider::format_edit_url(
            &self.record.source_repo,
            &self.record.source_branch,
            self.record.source_path.trim_start_matches('/'),
            edit_path.trim_end_matches("/"),
        );

        let page = ProjectPage {
            frontmatter: frontmatter.clone(),
            content: raw.content,
            edit_url,
            properties: HashMap::new(),
            links,
        };
        Ok((page, frontmatter))
    }
}

#[async_trait]
impl Project for LocalProject {
    fn id(&self) -> &str {
        &self.record.id
    }

    fn locale(&self) -> &str {
        self.locale.as_deref().unwrap_or(DEFAULT_LOCALE)
    }

    fn locales(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let path = self.format.translated_root();
        if let Ok(read) = fs::read_dir(&path) {
            for entry in read.flatten() {
                if let Ok(ft) = entry.file_type()
                    && ft.is_dir()
                {
                    out.insert(entry.file_name().to_string_lossy().into_owned());
                }
            }
        }
        out
    }

    async fn available_versions(&self) -> DomainResult<HashMap<String, String>> {
        let versions = self.repo.get_versions().await?;
        Ok(versions
            .into_iter()
            .filter_map(|v| v.name.map(|n| (n, v.branch)))
            .collect())
    }

    async fn has_version(&self, version: &str) -> DomainResult<bool> {
        Ok(self.available_versions().await?.contains_key(version))
    }

    async fn read_docs_page(&self, slug: &str) -> DomainResult<(ProjectPage, Frontmatter)> {
        self.read_page(&self.format.docs_page_path(slug)).await
    }

    async fn read_content_page(&self, p_ref: &str) -> DomainResult<ProjectPage> {
        let slug = self
            .repo
            .get_project_page_path(p_ref)
            .await
            .map_err(|_| DomainError::NotFound)?;
        let page_path = self.format.content_page_path(&slug);

        let (mut page, raw_fm) = self.read_page(&page_path).await?;

        let default_infobox = self.build_default_infobox(&raw_fm).await;
        page.frontmatter.infobox = Some(merge_infobox(
            default_infobox,
            page.frontmatter.infobox.take(),
        ));

        page.properties = self
            .read_item_properties(&page.frontmatter.id)
            .await
            .unwrap_or_default();

        Ok(page)
    }

    async fn item_content_pages(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ItemContentPage>> {
        let raw = self
            .repo
            .get_project_items_dev(&params.query, params.page)
            .await?;

        let mut out = Vec::with_capacity(raw.data.len());
        for entry in raw.data {
            let name = self
                .resolver
                .resolve_item_name(&entry.project_id, &entry.loc, self.locale.as_deref())
                .await
                .unwrap_or_default();
            let icon = entry
                .path
                .as_deref()
                .map(|slug| self.format.content_page_path(slug))
                .and_then(|page_path| self.format.try_read_frontmatter_at(&page_path))
                .and_then(|fm| fm.icon);
            out.push(ItemContentPage {
                id: entry.loc,
                name,
                icon,
                path: entry.path,
            });
        }
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data: out,
        })
    }

    async fn tags(&self, params: TableQueryParams) -> DomainResult<PaginatedData<FullTagData>> {
        let raw = self
            .repo
            .get_project_tags_dev(&params.query, params.page)
            .await?;

        let mut out = Vec::with_capacity(raw.data.len());
        for row in raw.data {
            let items = self.repo.get_project_tag_items_flat(row.id).await?;
            out.push(FullTagData {
                id: row.loc,
                items: items.into_iter().map(|i| i.loc).collect(),
            });
        }
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data: out,
        })
    }

    async fn tag_items(
        &self,
        tag: &str,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullItemData>> {
        let raw = self
            .repo
            .get_project_tag_items_dev(tag, &params.query, params.page)
            .await?;

        let mut out = Vec::with_capacity(raw.data.len());
        for entry in raw.data {
            let name = self
                .resolver
                .resolve_item_name(&entry.project_id, &entry.loc, self.locale.as_deref())
                .await
                .unwrap_or_default();
            out.push(FullItemData {
                id: entry.loc,
                name,
                page_ref: entry.path,
            });
        }
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data: out,
        })
    }

    async fn recipes(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullRecipeData>> {
        let raw = self
            .repo
            .get_project_recipes_dev(&params.query, params.page)
            .await?;

        let mut out = Vec::with_capacity(raw.data.len());
        for recipe in raw.data {
            let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
            let data = recipe_resolver.resolve(&recipe).await?;
            out.push(FullRecipeData {
                id: recipe.loc.clone(),
                data,
            });
        }
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data: out,
        })
    }

    async fn versions(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ProjectVersionData>> {
        let raw = self
            .repo
            .get_versions_dev(&params.query, params.page)
            .await?;
        let data: Vec<ProjectVersionData> = raw.data.iter().map(|v| v.into()).collect();
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data,
        })
    }

    async fn item_name(&self, loc: &str) -> DomainResult<FullItemData> {
        let parsed = ResourceLocation::parse(loc).ok_or(DomainError::NotFound)?;

        let item_key = format!("item.{}.{}", parsed.namespace, parsed.path);
        let block_key = format!("block.{}.{}", parsed.namespace, parsed.path);

        let mut localized = self.read_lang_key(&parsed.namespace, &item_key).await?;
        if localized.is_none() {
            localized = self.read_lang_key(&parsed.namespace, &block_key).await?;
        }

        let page = self.repo.get_project_item_page_ref(loc).await.ok();

        match localized {
            Some(name) => Ok(FullItemData {
                id: loc.to_owned(),
                name,
                page_ref: page.map(|p| p.r#ref),
            }),
            None => {
                if let Some(ref row) = page
                    && let Some(title) = self
                        .format
                        .read_page_title(&self.format.content_page_path(&row.path))
                {
                    return Ok(FullItemData {
                        id: loc.to_owned(),
                        name: title,
                        page_ref: Some(row.r#ref.clone()),
                    });
                }
                Err(DomainError::NotFound)
            }
        }
    }

    async fn read_lang_key(&self, namespace: &str, key: &str) -> DomainResult<Option<String>> {
        let path = self.format.language_file_path(namespace, self.locale());
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(None);
        };
        let parsed: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        Ok(parsed.get(key).and_then(|v| v.as_str()).map(str::to_owned))
    }

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Option<GameRecipeType>> {
        let path = self
            .format
            .data_root()
            .join(&location.namespace)
            .join("recipe_type")
            .join(format!("{}.json", location.path));
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(None);
        };
        let stub = match serde_json::from_str::<StubRecipeType>(&text) {
            Ok(v) => v,
            Err(e) => {
                warn!(path = %path.display(), "invalid recipe type json: {e}");
                return Ok(None);
            }
        };

        let lang_key = format!("recipe_type.{}.{}", location.namespace, location.path);
        let localized_name = self.read_lang_key(&location.namespace, &lang_key).await?;

        Ok(Some(GameRecipeType {
            id: location.to_string(),
            localized_name,
            background: stub.background,
            input_slots: stub.input_slots,
            output_slots: stub.output_slots,
        }))
    }

    async fn recipe_type_workbenches(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Vec<ResolvedItem>> {
        resolve_workbenches(&self.repo, &self.resolver, location, self.locale.as_deref()).await
    }

    async fn recipe(&self, id: &str) -> DomainResult<Option<ResolvedGameRecipe>> {
        let Ok(recipe) = self.repo.get_project_recipe(id).await else {
            return Ok(None);
        };
        let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
        Ok(Some(recipe_resolver.resolve(&recipe).await?))
    }

    async fn recipes_for_page(&self, page_ref: &str) -> DomainResult<Vec<ResolvedGameRecipe>> {
        let recipes = self.repo.get_recipes_for_page_ref(page_ref).await?;

        let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
        let mut out = Vec::with_capacity(recipes.len());
        for recipe in recipes {
            match recipe_resolver.resolve(&recipe).await {
                Ok(recipe) => out.push(recipe),
                Err(e) => {
                    warn!(
                        project = %self.id(),
                        page_ref = page_ref,
                        recipe = %recipe.loc,
                        "error resolving recipe for page: {e}"
                    )
                }
            }
        }
        Ok(out)
    }

    async fn obtainable_items_by(&self, page_ref: &str) -> DomainResult<Vec<ResolvedItem>> {
        let rows = self.repo.get_obtainable_items_for_page(page_ref).await?;
        Ok(resolve_content_usage(&self.resolver, rows, self.locale.as_deref()).await)
    }

    async fn project_info(&self) -> DomainResult<ProjectInfo> {
        let metadata = self.format.read_metadata_async().await?;

        let licenses = metadata
            .licenses
            .map(|l| ProjectLicenses {
                project: l.project.map(|p| ProjectLicense {
                    id: p.id,
                    name: p.name,
                    url: p.url,
                }),
            })
            .unwrap_or(ProjectLicenses { project: None });

        let content_count = self.repo.get_project_content_count().await.unwrap_or(0) as u64;

        let page_count = self
            .directory_tree()
            .await
            .map(|t| count_pages(&t))
            .unwrap_or(0);

        Ok(ProjectInfo {
            page_count,
            content_count,
            licenses,
        })
    }

    async fn directory_tree(&self) -> DomainResult<FileTree> {
        Ok(self.format.docs_tree())
    }

    async fn project_contents(&self) -> DomainResult<ContentFileTree> {
        let path = self.format.contents_root();
        if !path.exists() {
            return Err(DomainError::NotFound);
        }
        Ok(self.format.content_tree(&self.repo).await?)
    }

    fn asset(&self, location: &ResourceLocation) -> Option<PathBuf> {
        let primary = self.format.asset_path(location);
        if primary.exists() {
            return Some(primary);
        }
        // Legacy fallback: item/<ns>/<path>
        let legacy = self.format.asset_path(&ResourceLocation::new(
            "item",
            format!("{}/{}", location.namespace, location.path),
        ));
        if legacy.exists() {
            return Some(legacy);
        }
        None
    }
}
