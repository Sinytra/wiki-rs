use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ProjectResolver;
use crate::builtin::recipe_types::get_builtin_recipe_type;
use crate::recipe_types::{resolve_content_usage, resolve_workbenches};
use wiki_db::entity::{project, project_version};
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::DomainError;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{ContentFileTree, FileTree, Frontmatter, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project, ProjectPage};
use wiki_domain::response::{ProjectInfo, ProjectVersionData};
use wiki_system::{DEFAULT_LOCALE, LangService};

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
    ) -> Result<PaginatedData<ProjectVersionData>, DomainError> {
        Ok(PaginatedData::empty())
    }

    async fn item_name(&self, loc: &str) -> Result<FullItemData, DomainError> {
        let name = self
            .lang
            .get_item_name(None, loc)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?
            .ok_or(DomainError::NotFound)?;
        Ok(FullItemData {
            id: loc.to_owned(),
            name,
            path: None,
        })
    }

    async fn read_item_properties(&self, _id: &str) -> Result<HashMap<String, serde_json::Value>, DomainError> {
        Ok(HashMap::default())
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

    async fn recipes_for_item(
        &self,
        _item_loc: &str,
    ) -> Result<Vec<ResolvedGameRecipe>, DomainError> {
        Ok(Vec::new())
    }

    async fn obtainable_items_by(&self, item_loc: &str) -> Result<Vec<ResolvedItem>, DomainError> {
        let rows = self
            .repo
            .get_obtainable_items_by(item_loc)
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;
        Ok(resolve_content_usage(&self.resolver, rows, None).await)
    }

    async fn project_info(&self) -> Result<ProjectInfo, DomainError> {
        Ok(ProjectInfo::default())
    }

    async fn directory_tree(&self) -> Result<FileTree, DomainError> {
        Err(DomainError::NotFound)
    }

    async fn project_contents(&self) -> Result<ContentFileTree, DomainError> {
        Err(DomainError::NotFound)
    }

    fn asset(&self, _location: &ResourceLocation) -> Option<PathBuf> {
        None
    }
}
