use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tracing::error;
use wiki_db::query;
use wiki_domain::response::{
    AccessKeyBrief, AccessKeyInfo, AdminProjectInfo, CreateAccessKeyResponse, DataImportInfo,
    LocaleInfo, SystemInfoResponse, SystemStats,
};
use wiki_domain::visibility::ProjectVisibility;
use wiki_domain::{PaginatedData, TableQueryParams};

use crate::error::ApiResult;
use crate::extractors::Authenticated;
use crate::state::AppState;

pub async fn get_locales(
    State(_state): State<AppState>,
) -> ApiResult<Json<Vec<LocaleInfo>>> {
    // TODO: Fetch available locales from Crowdin service
    Ok(Json(vec![LocaleInfo {
        id: "en".to_owned(),
        name: "English".to_owned(),
        locale: "en_us".to_owned(),
    }]))
}

pub async fn get_system_info(
    State(state): State<AppState>,
) -> ApiResult<Json<SystemInfoResponse>> {
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
            users: user_count
        },
    }))
}

pub async fn get_data_imports(
    State(state): State<AppState>,
    Query(params): Query<TableQueryParams>,
) -> ApiResult<Json<PaginatedData<DataImportInfo>>> {
    let imports = query::data_import::get_data_imports(&state.db, &params.query, params.page).await?;
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

pub async fn import_data(
    State(state): State<AppState>,
    Json(body): Json<ImportBody>,
) -> ApiResult<StatusCode> {
    let result = state.game_data.import_game_data(body.update_loader)
        .await
        .inspect_err(|err| {
            error!(?err, "failed to import game data");
        });
    match result {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn available_migrations(
    State(_state): State<AppState>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(vec![]))
}

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
            visibility: p.visibility.parse().unwrap_or(ProjectVisibility::Unlisted),
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
        },
        token,
    }))
}

pub async fn delete_access_key(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    query::access_key::delete_access_key(&state.db, id).await?;
    Ok(StatusCode::OK)
}
