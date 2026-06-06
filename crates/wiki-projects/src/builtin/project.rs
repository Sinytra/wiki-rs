use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ProjectResolver;
use crate::builtin::recipe_types::get_builtin_recipe_type;
use crate::recipe_types::resolve_workbenches;
use wiki_db::entity::{project, project_version};
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::{DomainError, DomainResult};
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{ContentFileTree, FileTree, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project, ProjectPage};
use wiki_domain::response::{ProjectInfo, ProjectVersionData};
use wiki_system::{LangService, DEFAULT_LOCALE};

pub struct BuiltinProject {
    record: project::Model,
    version: project_version::Model,
    lang: Arc<LangService>,
    repo: Arc<ProjectRepo>,
    resolver: Arc<ProjectResolver>,
}

impl BuiltinProject {
    pub fn new(
        record: project::Model,
        version: project_version::Model,
        lang: Arc<LangService>,
        repo: Arc<ProjectRepo>,
        resolver: Arc<ProjectResolver>,
    ) -> Self {
        Self {
            record,
            version,
            lang,
            repo,
            resolver,
        }
    }

    pub fn record(&self) -> &project::Model {
        &self.record
    }

    pub fn version(&self) -> &project_version::Model {
        &self.version
    }
}

#[async_trait]
impl Project for BuiltinProject {
    fn id(&self) -> &str {
        &self.record.id
    }

    fn locale(&self) -> &str {
        DEFAULT_LOCALE
    }

    fn locales(&self) -> BTreeSet<String> {
        let mut s = BTreeSet::new();
        s.insert(DEFAULT_LOCALE.to_owned());
        s
    }

    async fn available_versions(&self) -> DomainResult<HashMap<String, String>> {
        Ok(HashMap::new())
    }

    async fn has_version(&self, _version: &str) -> DomainResult<bool> {
        Ok(false)
    }

    async fn read_docs_page(&self, _slug: &str) -> DomainResult<(ProjectPage, Frontmatter)> {
        Err(DomainError::NotFound)
    }

    async fn read_content_page(&self, _ref: &str) -> DomainResult<ProjectPage> {
        Err(DomainError::NotFound)
    }

    async fn item_content_pages(
        &self,
        _params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ItemContentPage>> {
        Ok(PaginatedData::empty())
    }

    async fn tags(&self, _params: TableQueryParams) -> DomainResult<PaginatedData<FullTagData>> {
        Ok(PaginatedData::empty())
    }

    async fn tag_items(
        &self,
        _tag: &str,
        _params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullItemData>> {
        Ok(PaginatedData::empty())
    }

    async fn recipes(
        &self,
        _params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullRecipeData>> {
        Ok(PaginatedData::empty())
    }

    async fn versions(
        &self,
        _params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ProjectVersionData>> {
        Ok(PaginatedData::empty())
    }

    async fn item_name(&self, loc: &str) -> DomainResult<FullItemData> {
        let name = self
            .lang
            .get_item_name(None, loc)
            .await?
            .ok_or(DomainError::NotFound)?;
        Ok(FullItemData {
            id: loc.to_owned(),
            name,
            page_ref: None,
        })
    }

    async fn read_lang_key(&self, namespace: &str, key: &str) -> DomainResult<Option<String>> {
        let location = format!("{namespace}:{key}");
        Ok(self.lang.get_item_name(None, &location).await?)
    }

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Option<GameRecipeType>> {
        Ok(get_builtin_recipe_type(location))
    }

    async fn recipe_type_workbenches(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Vec<ResolvedItem>> {
        resolve_workbenches(&self.repo, &self.resolver, location, None).await
    }

    async fn recipe(&self, _id: &str) -> DomainResult<Option<ResolvedGameRecipe>> {
        Ok(None)
    }

    async fn recipes_for_page(&self, _page_ref: &str) -> DomainResult<Vec<ResolvedGameRecipe>> {
        Ok(Vec::new())
    }

    async fn obtainable_items_by(&self, _page_ref: &str) -> DomainResult<Vec<ResolvedItem>> {
        Ok(Vec::new())
    }

    async fn project_info(&self) -> DomainResult<ProjectInfo> {
        Ok(ProjectInfo::default())
    }

    async fn directory_tree(&self) -> DomainResult<FileTree> {
        Err(DomainError::NotFound)
    }

    async fn project_contents(&self) -> DomainResult<ContentFileTree> {
        Err(DomainError::NotFound)
    }

    fn asset(&self, _location: &ResourceLocation) -> Option<PathBuf> {
        None
    }
}
