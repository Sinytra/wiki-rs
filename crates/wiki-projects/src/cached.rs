use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::warn;

use wiki_domain::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::error::{DomainError, DomainResult};
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::pagination::{PaginatedData, TableQueryParams};
use wiki_domain::project::{ContentFileTree, FileTree, FullItemData, FullRecipeData, FullTagData, ItemContentPage, Project, ProjectPage};
use wiki_domain::response::{ProjectInfo, ProjectVersionData};
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

    async fn get_or_resolve<T, F, Fut>(&self, key: String, supplier: F) -> DomainResult<T>
    where
        T: Serialize + DeserializeOwned + Clone + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = DomainResult<T>> + Send + 'static,
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
            .await?;

        if serialized.is_empty() {
            return self.fallback_supplier(key.as_str()).await;
        }

        serde_json::from_str(&serialized)
            .map_err(|e| DomainError::Internal(format!("cache decode: {e}")))
    }

    async fn fallback_supplier<T: DeserializeOwned>(&self, _key: &str) -> DomainResult<T> {
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

    fn locales(&self) -> BTreeSet<String> {
        self.inner.locales()
    }

    async fn available_versions(&self) -> DomainResult<HashMap<String, String>> {
        self.inner.available_versions().await
    }

    async fn has_version(&self, version: &str) -> DomainResult<bool> {
        self.inner.has_version(version).await
    }

    async fn read_docs_index_page(&self) -> DomainResult<(ProjectPage, Frontmatter)> {
        self.inner.read_docs_index_page().await
    }
    
    async fn read_docs_page(&self, path: &str) -> DomainResult<(ProjectPage, Frontmatter)> {
        self.inner.read_docs_page(path).await
    }

    async fn read_content_page(&self, id: &str) -> DomainResult<ProjectPage> {
        self.inner.read_content_page(id).await
    }

    async fn item_content_pages(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ItemContentPage>> {
        self.inner.item_content_pages(params).await
    }

    async fn tags(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullTagData>> {
        self.inner.tags(params).await
    }

    async fn tag_items(
        &self,
        tag: &str,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullItemData>> {
        self.inner.tag_items(tag, params).await
    }

    async fn recipes(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullRecipeData>> {
        self.inner.recipes(params).await
    }

    async fn versions(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ProjectVersionData>> {
        self.inner.versions(params).await
    }

    async fn item_name(&self, loc: &str) -> DomainResult<FullItemData> {
        self.inner.item_name(loc).await
    }

    async fn read_lang_key(
        &self,
        namespace: &str,
        key: &str,
    ) -> DomainResult<Option<String>> {
        self.inner.read_lang_key(namespace, key).await
    }

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Option<GameRecipeType>> {
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
    ) -> DomainResult<Vec<ResolvedItem>> {
        let key = self.cache_key_with("recipe_type_workbenches", &location.to_string());
        let inner = Arc::clone(&self.inner);
        let location = location.clone();
        self.get_or_resolve(key, move || async move {
            inner.recipe_type_workbenches(&location).await
        })
        .await
    }

    async fn recipe(&self, id: &str) -> DomainResult<Option<ResolvedGameRecipe>> {
        let key = self.cache_key_with("recipe", id);
        let inner = Arc::clone(&self.inner);
        let id = id.to_owned();
        self.get_or_resolve(key, move || async move { inner.recipe(&id).await })
            .await
    }

    async fn recipes_for_page(
        &self,
        page_ref: &str,
    ) -> DomainResult<Vec<ResolvedGameRecipe>> {
        self.inner.recipes_for_page(page_ref).await
    }

    async fn obtainable_items_by(&self, page_ref: &str) -> DomainResult<Vec<ResolvedItem>> {
        self.inner.obtainable_items_by(page_ref).await
    }

    async fn project_info(&self) -> DomainResult<ProjectInfo> {
        let key = self.cache_key("project_info");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.project_info().await })
            .await
    }

    async fn directory_tree(&self) -> DomainResult<FileTree> {
        let key = self.cache_key("directory_tree");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.directory_tree().await })
            .await
    }

    async fn project_contents(&self) -> DomainResult<ContentFileTree> {
        let key = self.cache_key("content_tree");
        let inner = Arc::clone(&self.inner);
        self.get_or_resolve(key, move || async move { inner.project_contents().await })
            .await
    }

    fn item_asset(&self, location: &ResourceLocation) -> Option<PathBuf> {
        self.inner.item_asset(location)
    }

    fn asset(&self, location: &ResourceLocation) -> Option<PathBuf> {
        self.inner.asset(location)
    }
}
