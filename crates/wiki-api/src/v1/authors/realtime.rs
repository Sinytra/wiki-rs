use std::collections::HashSet;
use std::convert::Infallible;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use serde::Deserialize;
use wiki_db::query;
use wiki_storage::realtime::{Subscriber, SubscriberScope};

use crate::error::{ApiError, ApiResult};
use crate::extractors::Authenticated;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DeploymentEventParams {
    #[serde(default)]
    pub global: bool,
}

#[tracing::instrument(name = "Streaming deployment events", skip_all, fields(params = ?params))]
pub async fn deployment_events(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Query(params): Query<DeploymentEventParams>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let global_scope = params.global
        && query::user::is_admin(&state.db, &user.id)
            .await
            .unwrap_or(false);

    let scope = if global_scope {
        SubscriberScope::Global
    } else {
        let projects = query::user::get_user_projects(&state.db, &user.id)
            .await
            .map_err(|_| ApiError::Internal("failed to load user projects".into()))?;
        let ids: HashSet<String> = projects.into_iter().map(|p| p.id).collect();
        SubscriberScope::Projects(ids)
    };

    let subscriber = state.connections.subscribe(scope);
    let stream = futures::stream::unfold(subscriber, |mut sub: Subscriber| async move {
        let event = sub.receiver.recv().await?;
        let sse = match Event::default().event("deployment").json_data(&event) {
            Ok(sse) => sse,
            Err(e) => {
                tracing::error!("failed to serialize deployment event: {e}");
                return None;
            }
        };
        Some((Ok(sse), sub))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
