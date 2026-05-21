use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::warn;

use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::DomainError;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{
    FileTree, Frontmatter, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project,
    ProjectPage,
};
use wiki_domain::response::ProjectInfo;
use wiki_storage::cache::ProjectCacheProvider;
use wiki_storage::task_manager::TaskManager;
use wiki_system::MemoryCache;

const DEFAULT_EXPIRE_SECS: u64 = 14 * 24 * 60 * 60;

#[derive(Clone)]
pub struct CachedProject {
    inner: Arc<dyn Project>,
    cache: MemoryCache,
    cache_keys: ProjectCacheProvider,
    in_flight: Arc<TaskManager>,
}

impl CachedProject {
    pub fn new(inner: Arc<dyn Project>, cache: MemoryCache) -> Self {
        Self {
            cache_keys: ProjectCacheProvider::new(inner.id().to_owned()),
            inner,
            cache,
            in_flight: Arc::new(TaskManager::new()),
        }
    }

    fn cache_key(&self, base: &str) -> String {
        self.cache_keys.cache_key(base)
    }

    fn cache_key_with(&self, base: &str, specifier: &str) -> String {
        self.cache_keys.cache_key_with(base, specifier)
    }

    async fn get_or_resolve<T, F, Fut>(&self, key: String, supplier: F) -> Result<T, DomainError>
    where
        T: Serialize + DeserializeOwned + Clone + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, DomainError>> + Send + 'static,
    {
        if let Ok(Some(raw)) = self.cache.get(&key).await
            && let Ok(parsed) = serde_json::from_str::<T>(&raw)
        {
            return Ok(parsed);
        }

        let cache = self.cache.clone();
        let key_for_task = key.clone();
        let coord = Arc::clone(&self.in_flight);

        let serialized: String = coord
            .run_or_join(key.clone(), move || async move {
                let value = supplier().await;
                let res = match value {
                    Ok(v) => match serde_json::to_string(&v) {
                        Ok(s) => {
                            if let Err(e) = cache
                                .set(&key_for_task, &s, Duration::from_secs(DEFAULT_EXPIRE_SECS))
                                .await
                            {
                                warn!("failed to write project cache: {e}");
                            }
                            s
                        }
                        Err(_) => String::new(),
                    },
                    Err(_) => String::new(),
                };
                Ok(res)
            })
            .await
            .map_err(|e| DomainError::Internal(e.to_string()))?;

        if serialized.is_empty() {
            return self.fallback_supplier(key.as_str()).await;
        }

        serde_json::from_str(&serialized)
            .map_err(|e| DomainError::Internal(format!("cache decode: {e}")))
    }

    async fn fallback_supplier<T: DeserializeOwned>(&self, _key: &str) -> Result<T, DomainError> {
        Err(DomainError::Internal(
            "cached supplier returned error".into(),
        ))
    }
}

#[async_trait]
impl Project for CachedProject {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn locale(&self) -> &str {
        self.inner.locale()
    }

    fn has_locale(&self, locale: &str) -> bool {
        self.inner.has_locale(locale)
    }

    fn locales(&self) -> BTreeSet<String> {
        self.inner.locales()
    }

    async fn available_versions(&self) -> Result<HashMap<String, String>, DomainError> {
        self.inner.available_versions().await
    }

    async fn has_version(&self, version: &str) -> Result<bool, DomainError> {
        self.inner.has_version(version).await
    }

    fn page_path(&self, path: &str) -> Option<String> {
        self.inner.page_path(path)
    }

    fn page_title(&self, path: &str) -> Option<String> {
        self.inner.page_title(path)
    }

    fn read_page(&self, path: &str) -> Result<ProjectPage, DomainError> {
        self.inner.read_page(path)
    }

    async fn read_content_page(&self, id: &str) -> Result<ProjectPage, DomainError> {
        self.inner.read_content_page(id).await
    }

    fn page_attributes(&self, path: &str) -> Option<Frontmatter> {
        self.inner.page_attributes(path)
    }

    async fn item_content_pages(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<ItemContentPage>, DomainError> {
        self.inner.item_content_pages(params).await
    }

    async fn tags(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<FullTagData>, DomainError> {
        self.inner.tags(params).await
    }

    async fn tag_items(
        &self,
        tag: &str,
        params: TableQueryParams,
    ) -> Result<PaginatedData<FullItemData>, DomainError> {
        self.inner.tag_items(tag, params).await
    }

    async fn recipes(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<FullRecipeData>, DomainError> {
        self.inner.recipes(params).await
    }

    async fn versions(
        &self,
        params: TableQueryParams,
    ) -> Result<PaginatedData<serde_json::Value>, DomainError> {
        self.inner.versions(params).await
    }

    async fn item_name(&self, loc: &str) -> Result<FullItemData, DomainError> {
        self.inner.item_name(loc).await
    }

    async fn read_item_properties(&self, id: &str) -> Result<serde_json::Value, DomainError> {
        self.inner.read_item_properties(id).await
    }

    async fn read_lang_key(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<String>, DomainError> {
        self.inner.read_lang_key(namespace, key).await
    }

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> Result<Option<GameRecipeType>, DomainError> {
        let key = self.cache_key_with("recipe_type", &location.to_string());
        let inner = Arc::clone(&self.inner);
        let location = location.clone();
        self.get_or_resolve(
            key,
            move || async move { inner.recipe_type(&location).await },
        )
        .await
    }

    async fn recipe_type_workbenches(
        &self,
        location: &ResourceLocation,
    ) -> Result<Vec<ResolvedItem>, DomainError> {
        let key = self.cache_key_with("recipe_type_workbenches", &location.to_string());
        let inner = Arc::clone(&self.inner);
        let location = location.clone();
        self.get_or_resolve(key, move || async move {
            inner.recipe_type_workbenches(&location).await
        })
        .await
    }

    async fn recipe(&self, id: &str) -> Result<Option<ResolvedGameRecipe>, DomainError> {
        let key = self.cache_key_with("recipe", id);
        let inner = Arc::clone(&self.inner);
        let id = id.to_owned();
        self.get_or_resolve(key, move || async move { inner.recipe(&id).await })
            .await
    }

    async fn recipes_for_item(
        &self,
        item_loc: &str,
    ) -> Result<Vec<ResolvedGameRecipe>, DomainError> {
        self.inner.recipes_for_item(item_loc).await
    }

    async fn obtainable_items_by(&self, item_loc: &str) -> Result<Vec<ResolvedItem>, DomainError> {
        self.inner.obtainable_items_by(item_loc).await
    }

    async fn project_info(&self) -> Result<ProjectInfo, DomainError> {
        let key = self.cache_key("project_info");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.project_info().await })
            .await
    }

    async fn directory_tree(&self) -> Result<FileTree, DomainError> {
        let key = self.cache_key("directory_tree");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.directory_tree().await })
            .await
    }

    async fn project_contents(&self) -> Result<FileTree, DomainError> {
        let key = self.cache_key("content_tree");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.project_contents().await })
            .await
    }

    fn asset(&self, location: &ResourceLocation) -> Option<PathBuf> {
        self.inner.asset(location)
    }
}
