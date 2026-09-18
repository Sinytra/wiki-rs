use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::HeaderValue;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::error::DomainError;
use wiki_domain::project::ProjectOptions;
use wiki_domain::response::AvailableWiki;
use wiki_domain::visibility::ProjectVisibility;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const ZIP_MIME: HeaderValue = HeaderValue::from_static("application/zip");

#[derive(Debug, Deserialize)]
pub struct AvailableWikisBody {
    pub game_version: String,
    #[serde(default)]
    pub mods: Vec<String>,
    pub lang: Option<String>,
}

#[tracing::instrument(name = "Listing available wikis", skip_all, fields(body = ?body))]
pub async fn available_projects(
    State(state): State<AppState>,
    Json(body): Json<AvailableWikisBody>,
) -> ApiResult<Json<Vec<AvailableWiki>>> {
    let records = query::project::find_public_by_mod_ids(&state.db, &body.mods).await?;

    let mut entries = futures::stream::iter(records)
        .map(|record| {
            let state = state.clone();
            let game_version = body.game_version.clone();
            let lang = body.lang.clone();
            async move { resolve_available(&state, record, &game_version, lang.as_deref()).await }
        })
        .buffer_unordered(8)
        .filter_map(|entry| async move { entry })
        .collect::<Vec<_>>()
        .await;

    entries.sort_unstable_by(|a, b| a.id.cmp(&b.id));

    Ok(Json(entries))
}

async fn resolve_available(
    state: &AppState,
    record: project::Model,
    game_version: &str,
    lang: Option<&str>,
) -> Option<AvailableWiki> {
    let version = query::project_version::get_version(&state.db, &record.id, Some(game_version))
        .await
        .ok()
        .and_then(|v| v.name);

    let options = ProjectOptions::new(version.clone(), lang.map(str::to_owned));
    let project = match state.resolver.resolve(&record.id, &options).await {
        Ok(project) => project,
        Err(err) => {
            tracing::debug!("Skipping project {} in wiki listing: {err}", record.id);
            return None;
        }
    };

    let lang = lang
        .filter(|l| project.locales().contains(*l))
        .map(str::to_owned);

    Some(AvailableWiki {
        id: record.id,
        name: record.name,
        version,
        lang,
    })
}

#[derive(Debug, Deserialize)]
pub struct WikiArchiveParams {
    version: Option<String>,
}

#[tracing::instrument(name = "Downloading wiki archive", skip_all, fields(project = %project_id, params = ?params))]
pub async fn download_wiki(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(params): Query<WikiArchiveParams>,
) -> ApiResult<Response> {
    let record = query::project::find_by_id(&state.db, &project_id).await?;
    if record.visibility != ProjectVisibility::Public {
        return Err(ApiError::not_found());
    }

    if let Some(version) = &params.version {
        query::project_version::get_version(&state.db, &project_id, Some(version))
            .await
            .map_err(|_| ApiError::NotFound("version_not_found".into()))?;
    }

    query::deployment::get_active_deployment(&state.db, &project_id)
        .await
        .map_err(|_| ApiError::NotFound("no_active_deployment".into()))?;

    let archive = state
        .deployments
        .archive_project(&project_id, params.version.as_deref())
        .await
        .map_err(DomainError::from)?;

    let size = archive.size().await.map_err(DomainError::from)?;
    let file = archive.open().await.map_err(DomainError::from)?;

    let stream = ReaderStream::new(file).map(move |chunk| {
        let _archive = &archive;
        chunk
    });

    let file_name = match &params.version {
        Some(version) => sanitize(&format!("{project_id}-{version}")),
        None => sanitize(&project_id),
    };
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}.zip\""))
        .map_err(|_| ApiError::internal())?;

    Ok((
        [
            (CONTENT_TYPE, ZIP_MIME),
            (CONTENT_LENGTH, HeaderValue::from(size)),
            (CONTENT_DISPOSITION, disposition),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}
