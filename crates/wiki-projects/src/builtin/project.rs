use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use wiki_db::entity::{project, project_version};
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::DomainError;
use wiki_domain::ids::ProjectId;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{
    FileTree, Frontmatter, FullItemData, FullRecipeData, FullTagData, ItemContentPage, ItemData,
    Project, ProjectPage,
};
use wiki_system::{DEFAULT_LOCALE, LangService};
use crate::builtin::recipe_types::get_builtin_recipe_type;
use crate::ProjectResolver;
use crate::recipe_types::resolve_workbenches;

pub const BUILTIN_PROJECT_ID: &str = "minecraft";

pub struct BuiltinProject {
    id: ProjectId,
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
        let id = ProjectId::new(BUILTIN_PROJECT_ID);
        Self { id, record, version, lang, repo, resolver }
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
    fn id(&self) -> &ProjectId {
        &self.id
    }

    fn locale(&self) -> &str {
        DEFAULT_LOCALE
    }

    fn has_locale(&self, locale: &str) -> bool {
        locale == DEFAULT_LOCALE
    }

    fn locales(&self) -> BTreeSet<String> {
        let mut s = BTreeSet::new();
        s.insert(DEFAULT_LOCALE.to_owned());
        s
    }

    async fn available_versions(&self) -> Result<HashMap<String, String>, DomainError> {
        Ok(HashMap::new())
    }

    async fn has_version(&self, _version: &str) -> Result<bool, DomainError> {
        Ok(false)
    }

    fn page_path(&self, _path: &str) -> Option<String> {
        None
    }

    fn page_title(&self, _path: &str) -> Option<String> {
        None
    }

    fn read_page(&self, _path: &str) -> Result<ProjectPage, DomainError> {
        Err(DomainError::NotFound)
    }

    async fn read_content_page(&self, _id: &str) -> Result<ProjectPage, DomainError> {
        Err(DomainError::NotFound)
    }

    fn page_attributes(&self, _path: &str) -> Option<Frontmatter> {
        None
    }

    async fn item_content_pages(
        &self,
        _params: TableQueryParams,
    ) -> Result<PaginatedData<ItemContentPage>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn tags(
        &self,
        _params: TableQueryParams,
    ) -> Result<PaginatedData<FullTagData>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn tag_items(
        &self,
        _tag: &str,
        _params: TableQueryParams,
    ) -> Result<PaginatedData<FullItemData>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn recipes(
        &self,
        _params: TableQueryParams,
    ) -> Result<PaginatedData<FullRecipeData>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn versions(
        &self,
        _params: TableQueryParams,
    ) -> Result<PaginatedData<serde_json::Value>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn item_name(&self, loc: &str) -> Result<ItemData, DomainError> {
        let name = self
            .lang
            .get_item_name(None, loc)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?
            .ok_or(DomainError::NotFound)?;
        Ok(ItemData { name, path: None })
    }

    async fn read_item_properties(&self, _id: &str) -> Result<serde_json::Value, DomainError> {
        Ok(serde_json::Value::Null)
    }

    async fn read_lang_key(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<String>, DomainError> {
        let location = format!("{namespace}:{key}");
        self.lang
            .get_item_name(None, &location)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))
    }

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> Result<Option<GameRecipeType>, DomainError> {
        Ok(get_builtin_recipe_type(location))
    }

    async fn recipe_type_workbenches(
        &self,
        location: &ResourceLocation,
    ) -> Result<Vec<ResolvedItem>, DomainError> {
        resolve_workbenches(&self.repo, &self.resolver, location, None).await
    }

    async fn recipe(&self, _id: &str) -> Result<Option<ResolvedGameRecipe>, DomainError> {
        Ok(None)
    }

    async fn directory_tree(&self) -> Result<FileTree, DomainError> {
        Err(DomainError::NotFound)
    }

    async fn project_contents(&self) -> Result<FileTree, DomainError> {
        Err(DomainError::NotFound)
    }

    fn asset(&self, _location: &ResourceLocation) -> Option<PathBuf> {
        None
    }
}
