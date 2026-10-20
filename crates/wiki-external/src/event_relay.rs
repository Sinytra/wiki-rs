use crate::error::ExternalResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use strum::AsRefStr;
use tracing::warn;
use wiki_domain::cache::MemoryCache;

const POPULAR_CACHE_KEY: &str = "popular_projects";
const POPULAR_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const POPULAR_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
pub struct RelayEvent {
    #[serde(flatten)]
    pub payload: EventPayload,
    pub timestamp: DateTime<Utc>,
}

impl RelayEvent {
    pub fn now(payload: EventPayload) -> Self {
        Self {
            payload,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, AsRefStr)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EventPayload {
    ProjectCreated(ProjectEvent),
    ProjectDeleted(ProjectEvent),
    ReportCreated(ReportEvent),
    PurgeCache(PurgeCacheEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectEvent {
    pub id: String,
    pub name: String,
    pub project_type: String,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub platforms: Vec<(String, String)>,
    pub user: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurgeCacheEvent {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportEvent {
    pub report_type: String,
    pub reason: String,
    pub submitter_id: String,
    pub project_id: String,
    pub created_at: String,
}

#[derive(Deserialize)]
struct PopularResponse {
    ids: Vec<String>,
}

#[derive(Clone)]
pub struct EventRelay {
    http: reqwest::Client,
    base_url: Option<String>,
    secret: Option<String>,
    cache: MemoryCache,
}

impl EventRelay {
    pub fn new(
        http: reqwest::Client,
        base_url: Option<String>,
        secret: Option<String>,
        cache: MemoryCache,
    ) -> Self {
        let base_url = base_url
            .filter(|u| !u.trim().is_empty())
            .map(|u| u.trim_end_matches('/').to_owned());
        let secret = secret.filter(|s| !s.trim().is_empty());
        Self {
            http,
            base_url,
            secret,
            cache,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.base_url.is_some()
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.secret.as_deref() {
            Some(secret) => request.bearer_auth(secret),
            None => request,
        }
    }

    pub async fn send(&self, payload: EventPayload) -> ExternalResult<()> {
        let Some(base_url) = self.base_url.as_deref() else {
            return Ok(());
        };

        let event = RelayEvent::now(payload);
        let request = self.http.post(format!("{base_url}/events")).json(&event);

        self.authorize(request).send().await?.error_for_status()?;

        tracing::debug!(event = event.payload.as_ref(), "relayed event");
        Ok(())
    }

    pub async fn get_popular_project_ids(&self) -> ExternalResult<Option<Vec<String>>> {
        let Some(base_url) = self.base_url.as_deref() else {
            return Ok(None);
        };

        match self.cache.get_json::<Vec<String>>(POPULAR_CACHE_KEY).await {
            Ok(Some(cached)) => return Ok(Some(cached)),
            Ok(None) => {}
            Err(e) => warn!("failed to read popular projects cache: {e}"),
        }

        let request = self
            .http
            .get(format!("{base_url}/popular-projects"))
            .timeout(POPULAR_REQUEST_TIMEOUT);

        let response: PopularResponse = self
            .authorize(request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Err(e) = self
            .cache
            .set_json(POPULAR_CACHE_KEY, &response.ids, POPULAR_CACHE_TTL)
            .await
        {
            warn!("failed to write popular projects cache: {e}");
        }

        Ok(Some(response.ids))
    }
}
