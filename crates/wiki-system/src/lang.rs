use std::sync::Arc;
use std::time::Duration;

use tracing::error;
use wiki_domain::content::ResourceLocation;

use crate::cache::MemoryCache;
use crate::cacheable::TaskCoordinator;
use crate::error::SystemResult;
use crate::game_data::GameDataSource;

pub const DEFAULT_LOCALE: &str = "en_us";

const LANG_KEY_PREFIXES: &[&str] = &["item.minecraft.", "block.minecraft."];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadStatus {
    Ok,
    NotFound,
    Internal,
}

pub struct LangService {
    cache: MemoryCache,
    game_data: Arc<dyn GameDataSource>,
    loader: TaskCoordinator<String, LoadStatus>,
}

impl LangService {
    pub fn new(cache: MemoryCache, game_data: Arc<dyn GameDataSource>) -> Self {
        Self {
            cache,
            game_data,
            loader: TaskCoordinator::new(),
        }
    }

    pub async fn get_item_name(
        &self,
        locale: Option<&str>,
        location: &str,
    ) -> SystemResult<Option<String>> {
        let lang = locale.unwrap_or(DEFAULT_LOCALE).to_owned();
        let status = self.ensure_loaded(&lang).await;

        match status {
            LoadStatus::Ok => {}
            LoadStatus::NotFound if lang != DEFAULT_LOCALE => {
                return Box::pin(self.get_item_name(None, location)).await;
            }
            _ => return Ok(None),
        }

        let Some(loc) = ResourceLocation::parse(location) else {
            return Ok(None);
        };
        if loc.namespace != ResourceLocation::DEFAULT_NAMESPACE {
            return Ok(None);
        }

        let cache_key = format!("lang:{lang}:minecraft:{}", loc.path);
        self.cache.get(&cache_key).await
    }

    async fn ensure_loaded(&self, lang: &str) -> LoadStatus {
        let cache_key = format!("lang:{lang}");
        if let Ok(Some(_)) = self.cache.get(&cache_key).await {
            return LoadStatus::Ok;
        }

        let cache = self.cache.clone();
        let game_data = Arc::clone(&self.game_data);
        let lang_owned = lang.to_owned();
        let key = cache_key.clone();

        self.loader
            .run_or_join(cache_key, move || async move {
                Self::load(&cache, game_data.as_ref(), &lang_owned, &key).await
            })
            .await
    }

    async fn load(
        cache: &MemoryCache,
        game_data: &dyn GameDataSource,
        lang: &str,
        cache_key: &str,
    ) -> LoadStatus {
        let Some(entries) = game_data.get_lang(lang).await else {
            return LoadStatus::NotFound;
        };

        for (key, value) in &entries {
            for prefix in LANG_KEY_PREFIXES {
                if let Some(sub_key) = key.strip_prefix(*prefix)
                    && !sub_key.contains('.')
                {
                    let lang_cache_key = format!("lang:{lang}:minecraft:{sub_key}");
                    if let Err(e) = cache.set(&lang_cache_key, value, Duration::ZERO).await {
                        error!("failed to write lang cache entry: {e}");
                        return LoadStatus::Internal;
                    }
                    break;
                }
            }
        }

        if let Err(e) = cache.set(cache_key, "_", Duration::ZERO).await {
            error!("failed to write lang sentinel: {e}");
            return LoadStatus::Internal;
        }

        LoadStatus::Ok
    }
}
