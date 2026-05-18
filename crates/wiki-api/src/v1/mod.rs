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
        .route("/docs/{project}/asset/{*path}", get(docs::asset))
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
        .route("/browse", get(browse::browse))
        // Public project IDs
        .route("/projects", get(authors::public::list_ids))
        .route("/projects/bulk", post(authors::public::get_projects_bulk))
        // Docs
        .route("/docs/{project}", get(docs::project_info))
        .route("/docs/{project}/page/{*path}", get(docs::page))
        .route("/docs/{project}/tree", get(docs::tree))
        // Game content
        .route("/content/{project}", get(game::contents))
        .route("/content/{project}/{id}", get(game::content_item))
        .route("/content/{project}/{id}/recipe", get(game::content_item_recipe))
        .route("/content/{project}/{id}/usage", get(game::content_item_usage))
        .route("/content/{project}/{id}/name", get(game::content_item_name))
        .route("/content/{project}/recipe/{recipe}", get(game::recipe))
        .route("/content/{project}/recipe-type/{type}", get(game::recipe_type))
        // System (locales is public)
        .route("/system/locales", get(system::get_locales))
}

/// Require user session but no API key
/// These are called from the browser, hence the lack of API auth
fn client_user_routes() -> Router<AppState> {
    Router::new()
        .route("/dev/projects", post(authors::lifecycle::create))
        .route("/dev/projects", put(authors::lifecycle::update_source))
        .route_layer(login_required!(AuthBackend))
}

/// Require API key and user session
fn user_routes() -> Router<AppState> {
    Router::new()
        // Moderation (submit requires auth, not admin)
        .route("/moderation/reports", post(moderation::submit_report))
        // Lifecycle
        .route("/dev/projects", get(authors::lifecycle::list_user_projects))
        .route("/dev/projects/{project}", get(authors::lifecycle::get_project))
        .route("/dev/projects/{project}", put(authors::lifecycle::update))
        .route("/dev/projects/{project}", delete(authors::lifecycle::remove))
        .route("/dev/projects/{project}/deploy", post(authors::lifecycle::deploy_project))
        // Management
        .route("/dev/projects/{project}/members", get(authors::manage::list_members))
        .route("/dev/projects/{project}/members", post(authors::manage::add_member))
        .route("/dev/projects/{project}/members", delete(authors::manage::remove_member))
        .route("/dev/projects/{project}/flags/{flag}", delete(authors::manage::remove_flag))
        .route("/dev/projects/{project}/deployments", get(authors::manage::get_deployments))
        // Deployments
        .route("/dev/deployments/{id}", get(authors::manage::get_deployment))
        .route("/dev/deployments/{id}", delete(authors::manage::delete_deployment))
        // Content
        .route("/dev/projects/{project}/versions", get(authors::content::get_versions))
        .route("/dev/projects/{project}/content/pages", get(authors::content::get_content_pages))
        .route("/dev/projects/{project}/content/tags", get(authors::content::get_content_tags))
        .route("/dev/projects/{project}/content/tags/{*tag}", get(authors::content::get_tag_items))
        .route("/dev/projects/{project}/content/recipes", get(authors::content::get_recipes))
        // Issues
        .route("/dev/projects/{project}/issues", get(authors::manage::get_issues))
        .route("/dev/projects/{project}/issues", post(authors::manage::add_issue))
        .route_layer(login_required!(AuthBackend))
}

/// Require admin user sessions
fn admin_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // System (admin)
        .route("/system/info", get(system::get_system_info))
        .route("/system/imports", get(system::get_data_imports))
        .route("/system/import", post(system::import_data))
        .route("/system/projects", get(system::list_all_projects))
        .route("/system/keys", get(system::get_access_keys))
        .route("/system/keys", post(system::create_access_key))
        .route("/system/keys/{id}", delete(system::delete_access_key))
        // Moderation (admin)
        .route("/moderation/reports", get(moderation::list_reports))
        .route("/moderation/reports/{id}", get(moderation::get_report))
        .route("/moderation/reports/{id}", post(moderation::rule_report))
        .route_layer(from_fn_with_state(state, middleware::require_admin))
        .route_layer(login_required!(AuthBackend))
}
