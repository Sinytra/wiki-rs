use std::collections::HashMap;
use sea_orm::{DatabaseConnection, EntityTrait};
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::response::{DeploymentInfo, DevProjectData, ProjectIssueInfo, ProjectSummary, ProjectVersionData, ReportInfo};
use wiki_domain::visibility::{ProjectFlag, ProjectStatus};

use crate::entity::{deployment, project, project_issue, project_version, report};
use crate::error::DbResult;

// TODO Json column
fn parse_flags(s: Option<&str>) -> Vec<ProjectFlag> {
    s.and_then(|f| serde_json::from_str(f).ok())
        .unwrap_or_default()
}

impl From<&project::Model> for ProjectSummary {
    fn from(record: &project::Model) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            r#type: record.r#type,
            platforms: record.platforms.0.clone(),
            is_community: record.is_community,
            source_repo: if record.is_public { Some(record.source_repo.clone()) } else { None },
            created_at: record.created_at,
        }
    }
}

impl From<&project::Model> for DevProjectData {
    fn from(record: &project::Model) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            r#type: record.r#type,
            platforms: record.platforms.0.clone(),
            is_community: record.is_community,
            mod_id: record.modid.clone(),
            created_at: record.created_at,
            source_repo: record.source_repo.clone(),
            source_branch: record.source_branch.clone(),
            source_path: record.source_path.clone(),
            visibility: record.visibility,
            is_public: record.is_public,
            flags: parse_flags(record.flags.as_deref()),
            // Temporary values
            status: ProjectStatus::Healthy,
            has_active_deployment: false,
            access_level: ProjectMemberRole::Member,
            revision: None,
            issue_stats: HashMap::new(),
            has_failing_deployment: false,
            version_names: Vec::new(),
        }
    }
}

impl From<&deployment::Model> for DeploymentInfo {
    fn from(d: &deployment::Model) -> Self {
        Self {
            id: d.id.clone(),
            project_id: d.project_id.clone(),
            revision: d.revision.clone(),
            status: d.status,
            active: d.active,
            user_id: d.user_id.clone(),
            source_repo: d.source_repo.clone(),
            source_branch: d.source_branch.clone(),
            source_path: d.source_path.clone(),
            created_at: d.created_at,
            issues: Vec::new(),
        }
    }
}

impl From<&project_issue::Model> for ProjectIssueInfo {
    fn from(i: &project_issue::Model) -> Self {
        Self {
            id: i.id.clone(),
            level: i.level,
            deployment_id: i.deployment_id.clone(),
            r#type: i.r#type,
            subject: i.subject,
            details: i.details.clone(),
            file: i.file.clone(),
            version_name: i.version_name.clone(),
            created_at: i.created_at,
        }
    }
}

pub async fn report_info_from_model(
    db: &DatabaseConnection,
    r: &report::Model,
) -> DbResult<ReportInfo> {
    let version = match r.version_id {
        Some(v) => {
            project_version::Entity::find_by_id(v)
                .one(db)
                .await?
                .and_then(|v| v.name)
        }
        None => None
    };

    Ok(ReportInfo {
        id: r.id.clone(),
        r#type: r.r#type,
        reason: r.reason,
        body: r.body.clone(),
        status: r.status,
        submitter_id: r.submitter_id.clone(),
        project_id: r.project_id.clone(),
        path: r.path.clone(),
        locale: r.locale.clone(),
        version,
        created_at: r.created_at,
    })
}

impl From<&project_version::Model> for ProjectVersionData {
    fn from(r: &project_version::Model) -> Self {
        Self {
            name: r.name.clone(),
            branch: r.branch.clone()
        }
    }
}