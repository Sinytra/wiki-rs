use crate::error::ExternalResult;
use chrono::{DateTime, Utc};
use serde::Serialize;
use strum::AsRefStr;

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

#[derive(Clone)]
pub struct EventRelay {
    http: reqwest::Client,
    endpoint: Option<String>,
    secret: Option<String>,
}

impl EventRelay {
    pub fn new(http: reqwest::Client, endpoint: Option<String>, secret: Option<String>) -> Self {
        let endpoint = endpoint.filter(|u| !u.trim().is_empty());
        let secret = secret.filter(|s| !s.trim().is_empty());
        Self {
            http,
            endpoint,
            secret,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.endpoint.is_some()
    }

    pub async fn send(&self, payload: EventPayload) -> ExternalResult<()> {
        let Some(endpoint) = self.endpoint.as_deref() else {
            return Ok(());
        };

        let event = RelayEvent::now(payload);
        let mut request = self.http.post(endpoint).json(&event);
        if let Some(secret) = self.secret.as_deref() {
            request = request.bearer_auth(secret);
        }

        request.send().await?.error_for_status()?;

        tracing::debug!(event = event.payload.as_ref(), "relayed event");
        Ok(())
    }
}
