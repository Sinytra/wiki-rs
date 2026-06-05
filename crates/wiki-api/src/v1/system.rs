use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use std::borrow::ToOwned;
use std::sync::Arc;
use tracing::debug;
use wiki_db::query;
use wiki_db::query::deployment::get_active_deployment;
use wiki_domain::response::{
    AccessKeyBrief, AccessKeyInfo, AdminProjectInfo, CreateAccessKeyResponse, DataImportInfo,
    DataMigration, LocaleInfo, SystemInfoResponse, SystemStats,
};
use wiki_domain::util::LogErr;
use wiki_domain::{PaginatedData, TableQueryParams};
use wiki_projects::management;
use wiki_system::DEFAULT_LOCALE;

use crate::error::ApiResult;
use crate::extractors::Authenticated;
use crate::state::AppState;

#[tracing::instrument(name = "Getting locales", skip_all)]
pub async fn get_locales(State(state): State<AppState>) -> ApiResult<Json<Vec<LocaleInfo>>> {
    let mut locales = state.lang.get_available_locales().await?;
    locales.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out = Vec::with_capacity(locales.len() + 1);
    out.push(LocaleInfo {
        id: "en".to_owned(),
        name: "English".to_owned(),
        code: DEFAULT_LOCALE.to_owned(),
    });
    out.extend(locales.into_iter().map(|l| LocaleInfo {
        id: l.id,
        name: l.name,
        code: l.code,
    }));

    Ok(Json(out))
}

#[tracing::instrument(name = "Getting system info", skip_all)]
pub async fn get_system_info(State(state): State<AppState>) -> ApiResult<Json<SystemInfoResponse>> {
    let imports = query::data_import::get_data_imports(&state.db, "", 1).await?;
    let latest_data = imports.data.first().map(|i| DataImportInfo {
        id: i.id,
        game_version: i.game_version.clone(),
        loader: i.loader.clone(),
        loader_version: i.loader_version.clone(),
        user_id: i.user_id.clone(),
        created_at: i.created_at,
    });

    let project_count = query::project::get_all_projects(&state.db, "", 1)
        .await
        .map(|p| p.total)
        .unwrap_or(0);
    let user_count = query::user::get_user_count(&state.db).await.unwrap_or(0);

    Ok(Json(SystemInfoResponse {
        version: state.git_version.to_owned(),
        latest_data,
        stats: SystemStats {
            projects: project_count,
            users: user_count,
        },
    }))
}

#[tracing::instrument(name = "Getting data imports", skip_all, fields(params = ?params))]
pub async fn get_data_imports(
    State(state): State<AppState>,
    Query(params): Query<TableQueryParams>,
) -> ApiResult<Json<PaginatedData<DataImportInfo>>> {
    let imports =
        query::data_import::get_data_imports(&state.db, &params.query, params.page).await?;
    let data: Vec<DataImportInfo> = imports
        .data
        .iter()
        .map(|i| DataImportInfo {
            id: i.id,
            game_version: i.game_version.clone(),
            loader: i.loader.clone(),
            loader_version: i.loader_version.clone(),
            user_id: i.user_id.clone(),
            created_at: i.created_at,
        })
        .collect();

    Ok(Json(PaginatedData {
        total: imports.total,
        pages: imports.pages,
        size: imports.size,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ImportBody {
    #[serde(default)]
    pub update_loader: bool,
}

#[tracing::instrument(name = "Importing data", skip_all, fields(body = ?body))]
pub async fn import_data(
    State(state): State<AppState>,
    Json(body): Json<ImportBody>,
) -> ApiResult<StatusCode> {
    let result = state
        .game_data
        .import_game_data(body.update_loader)
        .await
        .inspect_err_log("failed to import game data");
    match result {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[tracing::instrument(name = "Listing available migrations", skip_all)]
pub async fn available_migrations(
    State(_state): State<AppState>,
) -> ApiResult<Json<Vec<DataMigration>>> {
    let migrations = vec![
        DataMigration {
            id: "deployments".into(),
            title: "Project deployment".into(),
            desc: "Create a new deployment for all projects".into()
        },
        DataMigration {
            id: "index-search".into(),
            title: "Index project search".into(),
            desc: "Re-index all projects for search".into()
        }
    ];

    Ok(Json(migrations))
}

#[tracing::instrument(name = "Running migration", skip_all)]
pub async fn run_migration(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let projects = query::project::get_non_virtual_projects(&state.db).await?;

    if id == "deployments" {
        tokio::task::spawn(async move {
            debug!("Enqueuing {} project deployments", projects.len());
            for record in projects {
                management::enqueue_deploy(Arc::clone(&state.deployments), record, None);
            }
        });

        return Ok(StatusCode::OK);
    }

    if id == "index-search" && let Some(indexer) = state.indexer {
        let clone_indexer = indexer.clone();
        tokio::task::spawn(async move {
            debug!("Re-indexing {} projects for search", projects.len());
            for record in projects {
                if let Ok(deployment) = get_active_deployment(&state.db, &record.id).await {
                    clone_indexer.schedule_reindex(record, deployment.id);
                }
            }
        });

        return Ok(StatusCode::OK);
    }

    Ok(StatusCode::NOT_FOUND)
}

#[tracing::instrument(name = "Listing all projects", skip_all, fields(params = ?params))]
pub async fn list_all_projects(
    State(state): State<AppState>,
    Query(params): Query<TableQueryParams>,
) -> ApiResult<Json<PaginatedData<AdminProjectInfo>>> {
    let projects = query::project::get_all_projects(&state.db, &params.query, params.page).await?;
    let data: Vec<AdminProjectInfo> = projects
        .data
        .iter()
        .map(|p| AdminProjectInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            r#type: p.r#type.as_ref().to_owned(),
            visibility: p.visibility,
            mod_id: p.modid.clone(),
            created_at: p.created_at,
        })
        .collect();

    Ok(Json(PaginatedData {
        total: projects.total,
        pages: projects.pages,
        size: projects.size,
        data,
    }))
}

#[tracing::instrument(name = "Getting access keys", skip_all, fields(params = ?params))]
pub async fn get_access_keys(
    State(state): State<AppState>,
    Query(params): Query<TableQueryParams>,
) -> ApiResult<Json<PaginatedData<AccessKeyInfo>>> {
    let keys = query::access_key::get_access_keys(&state.db, &params.query, params.page).await?;
    let data: Vec<AccessKeyInfo> = keys
        .data
        .iter()
        .map(|k| AccessKeyInfo {
            id: k.id,
            name: k.name.clone(),
            user_id: k.user_id.clone(),
            expires_at: k.expires_at,
            created_at: k.created_at,
            expired: k.expires_at.map(|e| e > k.created_at).unwrap_or(false),
        })
        .collect();

    Ok(Json(PaginatedData {
        total: keys.total,
        pages: keys.pages,
        size: keys.size,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateAccessKeyBody {
    pub name: String,
    #[serde(default)]
    pub days_valid: Option<i32>,
}

#[tracing::instrument(name = "Creating access key", skip_all, fields(body = ?body))]
pub async fn create_access_key(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Json(body): Json<CreateAccessKeyBody>,
) -> ApiResult<Json<CreateAccessKeyResponse>> {
    let (key, token) = query::access_key::create_access_key(
        &state.db,
        &body.name,
        &user.id,
        body.days_valid.unwrap_or(0),
    )
    .await?;

    Ok(Json(CreateAccessKeyResponse {
        key: AccessKeyBrief {
            id: key.id,
            name: key.name,
            expires_at: key.expires_at,
        },
        token,
    }))
}

#[tracing::instrument(name = "Deleting access key", skip_all)]
pub async fn delete_access_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    query::access_key::delete_access_key(&state.db, id).await?;
    Ok(StatusCode::OK)
}
