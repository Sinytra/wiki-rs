use std::collections::HashMap;

use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, Condition, FromQueryResult, Order, QueryOrder, QuerySelect, Set};
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

use crate::entity::{deployment, project_issue};
use crate::error::{DbError, DbResult};

pub struct NewProjectIssue<'a> {
    pub deployment_id: &'a str,
    pub level: ProjectIssueLevel,
    pub issue_type: ProjectIssueType,
    pub subject: ProjectError,
    pub details: Option<&'a str>,
    pub file: Option<&'a str>,
    pub version_name: Option<&'a str>,
}

#[tracing::instrument(name = "Adding project issue", skip(db, issue))]
pub async fn add_project_issue(
    db: &DatabaseConnection,
    issue: NewProjectIssue<'_>,
) -> DbResult<project_issue::Model> {
    let model = project_issue::ActiveModel {
        id: ActiveValue::NotSet,
        deployment_id: Set(issue.deployment_id.to_owned()),
        level: Set(issue.level),
        r#type: Set(issue.issue_type),
        subject: Set(issue.subject),
        details: Set(issue.details.map(|s| s.to_owned())),
        file: Set(issue.file.map(|s| s.to_owned())),
        version_name: Set(issue.version_name.map(|s| s.to_owned())),
        created_at: ActiveValue::NotSet,
    };
    Ok(model.insert(db).await?)
}

#[tracing::instrument(name = "Getting project issue", skip(db))]
pub async fn get_project_issue(
    db: &DatabaseConnection,
    deployment_id: &str,
    level: ProjectIssueLevel,
    issue_type: ProjectIssueType,
    file: Option<&str>,
) -> DbResult<project_issue::Model> {
    let mut condition = Condition::all()
        .add(project_issue::Column::DeploymentId.eq(deployment_id))
        .add(project_issue::Column::Level.eq(level.as_ref()))
        .add(project_issue::Column::Type.eq(issue_type.as_ref()));

    if let Some(f) = file {
        condition = condition.add(project_issue::Column::File.eq(f));
    }

    project_issue::Entity::find()
        .filter(condition)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting deployment issues", skip(db))]
pub async fn get_deployment_issues(
    db: &DatabaseConnection,
    deployment_id: &str,
) -> DbResult<Vec<project_issue::Model>> {
    Ok(project_issue::Entity::find()
        .filter(project_issue::Column::DeploymentId.eq(deployment_id))
        .order_by(
            Expr::cust(format!(
                "array_position(array['{}', '{}'], level)",
                ProjectIssueLevel::Error,
                ProjectIssueLevel::Warning
            )),
            Order::Asc,
        )
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Checking deployment for errors", skip(db))]
pub async fn deployment_has_errors(db: &DatabaseConnection, deployment_id: &str) -> DbResult<bool> {
    let exists = project_issue::Entity::find()
        .filter(project_issue::Column::DeploymentId.eq(deployment_id))
        .filter(project_issue::Column::Level.eq(ProjectIssueLevel::Error))
        .exists(db)
        .await?;
    Ok(exists)
}

#[derive(Debug, FromQueryResult)]
struct IssueStatRow {
    level: String,
    count: i64,
}

#[tracing::instrument(name = "Getting active project issue stats", skip(db))]
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
