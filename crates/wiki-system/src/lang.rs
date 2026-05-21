use std::sync::Arc;
use std::time::Duration;

use tracing::error;
use wiki_domain::content::ResourceLocation;
use wiki_external::crowdin::{Crowdin, Locale};

use crate::error::{SystemError, SystemResult};
use crate::game_data::GameDataSource;
use wiki_domain::cache::MemoryCache;
use wiki_storage::task_manager::TaskManager;

pub const DEFAULT_LOCALE: &str = "en_us";

const LANG_KEY_PREFIXES: &[&str] = &["item.minecraft.", "block.minecraft."];
const LOCALES_CACHE_KEY: &str = "crowdin:languages";
const LOCALES_CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

fn mojang_locale_remap(code: &str) -> Option<&'static str> {
    match code {
        "ms_arab" => Some("zlm_arab"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadStatus {
    Ok,
    NotFound,
    Internal,
}

pub struct LangService {
    cache: MemoryCache,
    game_data: Arc<dyn GameDataSource>,
    crowdin: Arc<Crowdin>,
    loader: TaskManager,
}

impl LangService {
    pub fn new(
        cache: MemoryCache,
        game_data: Arc<dyn GameDataSource>,
        crowdin: Arc<Crowdin>,
    ) -> Self {
        Self {
            cache,
            game_data,
            crowdin,
            loader: TaskManager::new(),
        }
    }

    pub async fn get_available_locales(&self) -> SystemResult<Vec<Locale>> {
        let mut locales: Vec<Locale> = match self.cache.get_json(LOCALES_CACHE_KEY).await? {
            Some(cached) => cached,
            None => {
                let fresh = self
                    .crowdin
                    .available_locales()
                    .await
                    .map_err(|e| SystemError::Internal(format!("crowdin: {e}")))?;
                self.cache
                    .set_json(LOCALES_CACHE_KEY, &fresh, LOCALES_CACHE_TTL)
                    .await?;
                fresh
            }
        };

        for locale in &mut locales {
            if let Some(mapped) = mojang_locale_remap(&locale.code) {
                locale.code = mapped.to_owned();
            }
        }

        Ok(locales)
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
        Ok(self.cache.get(&cache_key).await?)
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
                Ok(Self::load(&cache, game_data.as_ref(), &lang_owned, &key).await)
            })
            .await
            .unwrap_or(LoadStatus::Internal)
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
