pub mod authors;
pub mod browse;
pub mod docs;
pub mod game;
pub mod moderation;
pub mod system;

use axum::middleware::from_fn_with_state;
use axum::routing::{delete, get, post, put};
use axum::Router;
use axum_login::login_required;

use crate::auth::AuthBackend;
use crate::middleware;
use crate::state::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(public_routes())
        .merge(protected_routes(state))
        .merge(client_user_routes())
}

/// Require no form of auth whatsoever
fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/docs/{project}/asset/{*path}", get(docs::asset))
}

/// Require at least an API key
fn protected_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(api_routes())
        .merge(user_routes())
        .merge(admin_routes(state.clone()))
        .route_layer(from_fn_with_state(state, middleware::require_api_key))
}

/// Require an API key
fn api_routes() -> Router<AppState> {
    Router::new()
        // Browse
        .route("/api/v1/browse", get(browse::browse))
        // Public project IDs
        .route("/api/v1/projects", get(authors::public::list_ids))
        .route("/api/v1/projects/bulk", post(authors::public::get_projects_bulk))
        // Docs
        .route("/api/v1/docs/{project}", get(docs::project_info))
        .route("/api/v1/docs/{project}/page/{*path}", get(docs::page))
        .route("/api/v1/docs/{project}/tree", get(docs::tree))
        // Game content
        .route("/api/v1/content/{project}", get(game::contents))
        .route("/api/v1/content/{project}/{id}", get(game::content_item))
        .route("/api/v1/content/{project}/{id}/recipe", get(game::content_item_recipe))
        .route("/api/v1/content/{project}/{id}/usage", get(game::content_item_usage))
        .route("/api/v1/content/{project}/{id}/name", get(game::content_item_name))
        .route("/api/v1/content/{project}/recipe/{recipe}", get(game::recipe))
        .route("/api/v1/content/{project}/recipe-type/{type}", get(game::recipe_type))
        // System (locales is public)
        .route("/api/v1/system/locales", get(system::get_locales))
}

/// Require user session but no API key
/// These are called from the browser, hence the lack of API auth
fn client_user_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/dev/projects", post(authors::lifecycle::create))
        .route("/api/v1/dev/projects", put(authors::lifecycle::update_source))
        .route_layer(login_required!(AuthBackend))
}

/// Require API key and user session
fn user_routes() -> Router<AppState> {
    Router::new()
        // Moderation (submit requires auth, not admin)
        .route("/api/v1/moderation/reports", post(moderation::submit_report))
        // Lifecycle
        .route("/api/v1/dev/projects", get(authors::lifecycle::list_user_projects))
        .route("/api/v1/dev/projects/{id}", get(authors::lifecycle::get_project))
        .route("/api/v1/dev/projects/{id}", put(authors::lifecycle::update))
        .route("/api/v1/dev/projects/{id}", delete(authors::lifecycle::remove))
        .route("/api/v1/dev/projects/{id}/deploy", post(authors::lifecycle::deploy_project))
        // Management
        .route("/api/v1/dev/projects/{id}/members", get(authors::manage::list_members))
        .route("/api/v1/dev/projects/{id}/members", post(authors::manage::add_member))
        .route("/api/v1/dev/projects/{id}/members", delete(authors::manage::remove_member))
        .route("/api/v1/dev/projects/{id}/flags/{flag}", delete(authors::manage::remove_flag))
        .route("/api/v1/dev/projects/{id}/deployments", get(authors::manage::get_deployments))
        // Deployments
        .route("/api/v1/dev/deployments/{id}", get(authors::manage::get_deployment))
        .route("/api/v1/dev/deployments/{id}", delete(authors::manage::delete_deployment))
        // Content
        .route("/api/v1/dev/projects/{id}/versions", get(authors::content::get_versions))
        .route("/api/v1/dev/projects/{id}/content/pages", get(authors::content::get_content_pages))
        .route("/api/v1/dev/projects/{id}/content/tags", get(authors::content::get_content_tags))
        .route("/api/v1/dev/projects/{id}/content/tags/{*tag}", get(authors::content::get_tag_items))
        .route("/api/v1/dev/projects/{id}/content/recipes", get(authors::content::get_recipes))
        // Issues
        .route("/api/v1/dev/projects/{id}/issues", get(authors::manage::get_issues))
        .route("/api/v1/dev/projects/{id}/issues", post(authors::manage::add_issue))
        .route_layer(login_required!(AuthBackend))
}

/// Require admin user sessions
fn admin_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // System (admin)
        .route("/api/v1/system/info", get(system::get_system_info))
        .route("/api/v1/system/imports", get(system::get_data_imports))
        .route("/api/v1/system/import", post(system::import_data))
        .route("/api/v1/system/projects", get(system::list_all_projects))
        .route("/api/v1/system/keys", get(system::get_access_keys))
        .route("/api/v1/system/keys", post(system::create_access_key))
        .route("/api/v1/system/keys/{id}", delete(system::delete_access_key))
        // Moderation (admin)
        .route("/api/v1/moderation/reports", get(moderation::list_reports))
        .route("/api/v1/moderation/reports/{id}", get(moderation::get_report))
        .route("/api/v1/moderation/reports/{id}", post(moderation::rule_report))
        .route_layer(from_fn_with_state(state, middleware::require_admin))
        .route_layer(login_required!(AuthBackend))
}
