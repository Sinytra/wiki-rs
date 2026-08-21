use crate::ProjectResolver;
use std::sync::Arc;
use wiki_db::repo::{ProjectContent, ProjectRepo};
use wiki_domain::content::{ResolvedItem, ResourceLocation};
use wiki_domain::error::{DomainError, DomainResult};
use wiki_domain::project::ProjectOptions;

pub async fn resolve_content_usage(
    resolver: &Arc<ProjectResolver>,
    rows: Vec<ProjectContent>,
    options: &ProjectOptions,
) -> Vec<ResolvedItem> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let name = resolver
            .resolve_item_name(&row.project_id, &row.loc, options)
            .await;
        out.push(ResolvedItem {
            id: row.loc,
            name,
            project: row.project_id,
            page_ref: row.r#ref,
        });
    }
    out
}

pub async fn resolve_workbenches(
    repo: &ProjectRepo,
    resolver: &Arc<ProjectResolver>,
    location: &ResourceLocation,
    options: &ProjectOptions,
) -> DomainResult<Vec<ResolvedItem>> {
    let recipe_type = repo
        .get_recipe_type(&location.to_string())
        .await
        .map_err(|_| DomainError::NotFound)?;

    let rows = repo
        .get_recipe_type_workbenches(recipe_type.id)
        .await
        .unwrap_or_default();

    Ok(resolve_content_usage(resolver, rows, options).await)
}
