use crate::curseforge::CurseForge;
use crate::error::ExternalResult;
use crate::modrinth::Modrinth;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::warn;
use wiki_domain::cache::MemoryCache;
pub use wiki_domain::project::ProjectType;

const CACHE_TTL: Duration = Duration::from_secs(3 * 24 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformProject {
    pub slug: String,
    pub name: String,
    pub source_url: String,
    pub project_type: ProjectType,
    pub icon_url: Option<String>,
    pub platform: String,
}

#[derive(Clone)]
pub struct Platforms {
    pub modrinth: Modrinth,
    pub curseforge: CurseForge,
    cache: MemoryCache,
}

impl Platforms {
    pub fn new(modrinth: Modrinth, curseforge: CurseForge, cache: MemoryCache) -> Self {
        Self {
            modrinth,
            curseforge,
            cache,
        }
    }

    pub fn available_platforms(&self) -> Vec<&'static str> {
        vec![crate::modrinth::PLATFORM, crate::curseforge::PLATFORM]
    }

    pub async fn get_first_project(
        &self,
        slugs: &HashMap<String, String>,
    ) -> ExternalResult<Option<PlatformProject>> {
        for platform in self.available_platforms() {
            if let Some(slug) = slugs.get(platform)
                && let Ok(Some(project)) = self.get_project(platform, slug).await
            {
                return Ok(Some(project));
            }
        }
        Ok(None)
    }

    pub async fn get_project(
        &self,
        platform: &str,
        slug: &str,
    ) -> ExternalResult<Option<PlatformProject>> {
        let key = format!("platform:{platform}:{slug}");

        match self.cache.get_json::<PlatformProject>(&key).await {
            Ok(Some(cached)) => {
                return Ok(Some(cached));
            }
            Ok(None) => {}
            Err(e) => warn!("failed to read platform project cache: {e}"),
        }

        let result = match platform {
            crate::modrinth::PLATFORM => self.modrinth.get_project(slug).await?,
            crate::curseforge::PLATFORM => self.curseforge.get_project(slug).await?,
            _ => return Ok(None),
        };

        if let Some(project) = &result
            && let Err(e) = self.cache.set_json(&key, project, CACHE_TTL).await
        {
            warn!("failed to write platform project cache: {e}");
        }

        Ok(result)
    }
}
