use tracing::warn;
use wiki_domain::cache::MemoryCache;

const KEY_PREFIX: &str = "pcache";

#[derive(Debug, Clone)]
pub struct ProjectCacheProvider {
    project_id: String,
}

impl ProjectCacheProvider {
    pub fn new(project_id: String) -> Self {
        Self { project_id }
    }

    pub async fn clear_for_project(cache: &MemoryCache, project_id: &str) {
        let prefix = format!("{KEY_PREFIX}:{project_id}");
        if let Err(e) = cache.erase_all(&prefix).await {
            warn!("failed to clear project cache: {e}");
        }
    }

    pub fn cache_key(&self, base: &str) -> String {
        format!("{KEY_PREFIX}:{}:{}", self.project_id.as_str(), base)
    }

    pub fn cache_key_with(&self, base: &str, specifier: &str) -> String {
        format!(
            "{KEY_PREFIX}:{}:{}:{}",
            self.project_id.as_str(),
            base,
            specifier
        )
    }
}
