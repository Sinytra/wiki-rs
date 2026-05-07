use std::collections::HashMap;

use sea_orm::entity::prelude::*;
use sea_orm::{Condition, FromQueryResult, Order, QueryOrder, QuerySelect};

use crate::entity::{deployment, project_issue};
use crate::error::{DbError, DbResult};

pub async fn get_project_issue(
    db: &DatabaseConnection,
    deployment_id: &str,
    level: &str,
    issue_type: &str,
    file: Option<&str>,
) -> DbResult<project_issue::Model> {
    let mut condition = Condition::all()
        .add(project_issue::Column::DeploymentId.eq(deployment_id))
        .add(project_issue::Column::Level.eq(level))
        .add(project_issue::Column::Type.eq(issue_type));

    if let Some(f) = file {
        condition = condition.add(project_issue::Column::File.eq(f));
    }

    project_issue::Entity::find()
        .filter(condition)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_deployment_issues(
    db: &DatabaseConnection,
    deployment_id: &str,
) -> DbResult<Vec<project_issue::Model>> {
    Ok(project_issue::Entity::find()
        .filter(project_issue::Column::DeploymentId.eq(deployment_id))
        .order_by(
            Expr::cust("array_position(array['error', 'warning'], level)"),
            Order::Asc,
        )
        .all(db)
        .await?)
}

#[derive(Debug, FromQueryResult)]
struct IssueStatRow {
    level: String,
    count: i64,
}

pub async fn get_active_project_issue_stats(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<HashMap<String, i64>> {
    let active_deployment_subquery = sea_orm::QueryTrait::into_query(
        deployment::Entity::find()
            .filter(deployment::Column::ProjectId.eq(project_id))
            .filter(deployment::Column::Active.eq(true))
            .select_only()
            .column(deployment::Column::Id),
    );

    let rows = project_issue::Entity::find()
        .filter(project_issue::Column::DeploymentId.in_subquery(active_deployment_subquery))
        .select_only()
        .column(project_issue::Column::Level)
        .column_as(project_issue::Column::Level.count(), "count")
        .group_by(project_issue::Column::Level)
        .into_model::<IssueStatRow>()
        .all(db)
        .await?;

    Ok(rows.into_iter().map(|r| (r.level, r.count)).collect())
}
