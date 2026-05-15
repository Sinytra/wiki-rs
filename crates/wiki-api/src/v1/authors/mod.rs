use serde::Deserialize;
use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::visibility::ProjectStatus;
use crate::state::AppState;

pub mod public;
pub mod lifecycle;
pub mod manage;
pub mod content;

async fn get_project_status(state: &AppState, record: &project::Model) -> ProjectStatus {
    if query::deployment::get_loading_deployment(&state.db, &record.id)
        .await
        .is_ok()
    {
        return ProjectStatus::Loading;
    }

    let active = query::deployment::get_active_deployment(&state.db, &record.id).await;
    if active.is_err() {
        return ProjectStatus::Error;
    }

    let has_failing = query::deployment::has_failing_deployment(&state.db, &record.id)
        .await
        .unwrap_or(false);

    if has_failing {
        ProjectStatus::AtRisk
    } else {
        ProjectStatus::Healthy
    }
}

#[derive(Debug, Deserialize)]
pub struct ContentParams {
    pub version: Option<String>,
    pub locale: Option<String>,
    pub query: Option<String>,
    pub page: Option<u64>,
}
