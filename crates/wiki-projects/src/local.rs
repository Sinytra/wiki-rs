use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use wiki_db::entity::{project, project_version};
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::DomainError;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::FileType;
use wiki_domain::project::{
    FileTree, Frontmatter, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project,
    ProjectPage,
};
use wiki_domain::response::{ProjectInfo, ProjectLicense, ProjectLicenses, ProjectVersionData};
use wiki_storage::format::{DOCS_FILE_EXT, ProjectFormat};
use wiki_storage::git as git_provider;
use wiki_storage::ingestor::recipes::types::StubRecipeType;
use wiki_system::DEFAULT_LOCALE;

use crate::pages;
use crate::recipe_resolver::RecipeResolver;
use crate::recipe_types::{resolve_content_usage, resolve_workbenches};
use crate::resolver::ProjectResolver;

pub struct LocalProject {
    record: project::Model,
    version: project_version::Model,
    format: ProjectFormat,
    repo: Arc<ProjectRepo>,
    resolver: Arc<ProjectResolver>,
    locale: Option<String>,
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
        version: project_version::Model,
        checkout_path: PathBuf,
        repo: Arc<ProjectRepo>,
        resolver: Arc<ProjectResolver>,
        locale: Option<String>,
    ) -> Self {
        let format = ProjectFormat::new(checkout_path).with_locale(locale.clone());
        Self {
            record,
            version,
            format,
            repo,
            resolver,
            locale,
        }
    }

    pub fn record(&self) -> &project::Model {
        &self.record
    }

    pub fn version(&self) -> &project_version::Model {
        &self.version
    }

    pub fn checkout_path(&self) -> &std::path::Path {
        self.format.root()
    }

    pub fn format(&self) -> &ProjectFormat {
        &self.format
    }

    pub fn repo(&self) -> &ProjectRepo {
        &self.repo
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

    fn has_locale(&self, locale: &str) -> bool {
        self.locales().contains(locale)
    }

    fn locales(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let path = self.format.locales_path();
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

    async fn available_versions(&self) -> Result<HashMap<String, String>, DomainError> {
        let versions = self
            .repo
            .get_versions()
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;
        Ok(versions
            .into_iter()
            .filter_map(|v| v.name.map(|n| (n, v.branch)))
            .collect())
    }

    async fn has_version(&self, version: &str) -> Result<bool, DomainError> {
        Ok(self.available_versions().await?.contains_key(version))
    }

    fn page_path(&self, path: &str) -> Option<String> {
        let filename = format!("{}.{DOCS_FILE_EXT}", path.trim_start_matches('/'));
        let resolved = self.format.localized_file_path(&filename);
        if !resolved.exists() {
            return None;
        }
        let rel = resolved.strip_prefix(self.format.root()).ok()?;
        Some(rel.to_string_lossy().into_owned())
    }

    fn page_title(&self, path: &str) -> Option<String> {
        pages::read_page_title(&self.format, path)
    }

    fn read_page(&self, path: &str) -> Result<ProjectPage, DomainError> {
        let file_path = self
            .format
            .localized_file_path(path.trim_start_matches('/'));
        let content = fs::read_to_string(&file_path).map_err(|_| DomainError::NotFound)?;
        let edit_url = git_provider::format_edit_url(
            &self.record.source_repo,
            &self.record.source_branch,
            self.record.source_path.trim_start_matches('/'),
            path.trim_end_matches('/'),
        );
        Ok(ProjectPage { content, edit_url })
    }

    async fn read_content_page(&self, id: &str) -> Result<ProjectPage, DomainError> {
        let path = self
            .repo
            .get_project_content_path(id)
            .await
            .map_err(|_| DomainError::NotFound)?;
        self.read_page(&path)
    }

    fn page_attributes(&self, path: &str) -> Option<Frontmatter> {
        pages::read_page_attributes(&self.format, path)
    }

    async fn item_content_pages(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<ItemContentPage>, DomainError> {
        let raw = self
            .repo
            .get_project_items_dev(&params.query, params.page)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

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
                .and_then(|p| self.page_attributes(p))
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

    async fn tags(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<FullTagData>, DomainError> {
        let raw = self
            .repo
            .get_project_tags_dev(&params.query, params.page)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

        let mut out = Vec::with_capacity(raw.data.len());
        for row in raw.data {
            let items = self
                .repo
                .get_project_tag_items_flat(row.id)
                .await
                .map_err(|e| DomainError::Internal(e.to_string()))?;
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
    ) -> Result<PaginatedData<FullItemData>, DomainError> {
        let raw = self
            .repo
            .get_project_tag_items_dev(tag, &params.query, params.page)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

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

    async fn recipes(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<FullRecipeData>, DomainError> {
        let raw = self
            .repo
            .get_project_recipes_dev(&params.query, params.page)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

        let mut out = Vec::with_capacity(raw.data.len());
        for recipe in raw.data {
            let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
            let resolved = recipe_resolver.resolve(&recipe).await?;
            let data = serde_json::to_value(&resolved)
                .map_err(|e| DomainError::Internal(format!("encode recipe: {e}")))?;
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
    ) -> Result<PaginatedData<ProjectVersionData>, DomainError> {
        let raw = self
            .repo
            .get_versions_dev(&params.query, params.page)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;
        let data: Vec<ProjectVersionData> = raw.data.iter().map(|v| v.into()).collect();
        Ok(PaginatedData {
            total: raw.total,
            pages: raw.pages,
            size: raw.size,
            data,
        })
    }

    async fn item_name(&self, loc: &str) -> Result<FullItemData, DomainError> {
        let parsed = ResourceLocation::parse(loc).ok_or(DomainError::NotFound)?;

        let item_key = format!("item.{}.{}", parsed.namespace, parsed.path);
        let block_key = format!("block.{}.{}", parsed.namespace, parsed.path);

        let mut localized = self.read_lang_key(&parsed.namespace, &item_key).await?;
        if localized.is_none() {
            localized = self.read_lang_key(&parsed.namespace, &block_key).await?;
        }

        let path = self.repo.get_project_content_path(loc).await.ok();

        match localized {
            Some(name) => Ok(FullItemData {
                id: loc.to_owned(),
                name,
                path,
            }),
            None => {
                if let Some(ref p) = path
                    && let Some(title) = self.page_title(p)
                {
                    return Ok(FullItemData {
                        id: loc.to_owned(),
                        name: title,
                        path: path.clone(),
                    });
                }
                Err(DomainError::NotFound)
            }
        }
    }

    // TODO Use non-json type
    async fn read_item_properties(&self, id: &str) -> Result<serde_json::Value, DomainError> {
        let path = self.format.item_properties_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(serde_json::Value::Null);
        };
        let parsed: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                warn!(path = %path.display(), "invalid item properties json: {e}");
                return Ok(serde_json::Value::Null);
            }
        };
        Ok(parsed.get(id).cloned().unwrap_or(serde_json::Value::Null))
    }

    async fn read_lang_key(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<String>, DomainError> {
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
    ) -> Result<Option<GameRecipeType>, DomainError> {
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
    ) -> Result<Vec<ResolvedItem>, DomainError> {
        resolve_workbenches(&self.repo, &self.resolver, location, self.locale.as_deref()).await
    }

    async fn recipe(&self, id: &str) -> Result<Option<ResolvedGameRecipe>, DomainError> {
        let Ok(recipe) = self.repo.get_project_recipe(id).await else {
            return Ok(None);
        };
        let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
        Ok(Some(recipe_resolver.resolve(&recipe).await?))
    }

    async fn recipes_for_item(
        &self,
        item_loc: &str,
    ) -> Result<Vec<ResolvedGameRecipe>, DomainError> {
        let recipes = self
            .repo
            .get_recipes_for_item(item_loc)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

        let recipe_resolver = RecipeResolver::new(self.resolver.clone(), self.locale.clone());
        let mut out = Vec::with_capacity(recipes.len());
        for recipe in recipes {
            match recipe_resolver.resolve(&recipe).await {
                Ok(recipe) => out.push(recipe),
                Err(e) => {
                    warn!(
                        project = %self.id(),
                        item = item_loc,
                        recipe = %recipe.loc,
                        "error resolving recipe for item: {e}"
                    )
                }
            }
        }
        Ok(out)
    }

    async fn obtainable_items_by(&self, item_loc: &str) -> Result<Vec<ResolvedItem>, DomainError> {
        let rows = self
            .repo
            .get_obtainable_items_by(item_loc)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;
        Ok(resolve_content_usage(&self.resolver, rows, self.locale.as_deref()).await)
    }

    async fn project_info(&self) -> Result<ProjectInfo, DomainError> {
        let metadata = self
            .format
            .read_metadata_async()
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

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

    async fn directory_tree(&self) -> Result<FileTree, DomainError> {
        Ok(pages::directory_tree(&self.format, self.format.root()))
    }

    async fn project_contents(&self) -> Result<FileTree, DomainError> {
        let path = self.format.content_dir();
        if !path.exists() {
            return Err(DomainError::NotFound);
        }
        let mut tree = pages::directory_tree(&self.format, &path);
        pages::add_page_metadata(&self.format, &mut tree);
        Ok(tree)
    }

    fn asset(&self, location: &ResourceLocation) -> Option<PathBuf> {
        let primary = self.format.assets_path(location);
        if primary.exists() {
            return Some(primary);
        }
        // Legacy fallback: item/<ns>/<path>
        let legacy = self.format.assets_path(&ResourceLocation::new(
            "item",
            format!("{}/{}", location.namespace, location.path),
        ));
        if legacy.exists() {
            return Some(legacy);
        }
        None
    }
}
