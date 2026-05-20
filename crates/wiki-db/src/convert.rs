use std::collections::HashMap;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::response::{
    DeploymentInfo, ProjectDetails, ProjectIssueInfo, ProjectSummary, ReportInfo,
};
use wiki_domain::visibility::{ProjectFlag, ProjectStatus, ProjectVisibility, ReportStatus};

use crate::entity::{deployment, project, project_issue, report};

fn parse_flags(s: Option<&str>) -> Vec<ProjectFlag> {
    s.and_then(|f| serde_json::from_str(f).ok())
        .unwrap_or_default()
}

fn parse_visibility(s: &str) -> ProjectVisibility {
    s.parse().unwrap_or(ProjectVisibility::Unlisted)
}

fn parse_report_status(s: &str) -> ReportStatus {
    s.parse().unwrap_or(ReportStatus::New)
}

impl From<&project::Model> for ProjectSummary {
    fn from(record: &project::Model) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            r#type: record.r#type,
            platforms: record.platforms.0.clone(),
            is_community: record.is_community,
            created_at: record.created_at,
        }
    }
}

impl From<&project::Model> for ProjectDetails {
    fn from(record: &project::Model) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            r#type: record.r#type,
            platforms: record.platforms.0.clone(),
            is_community: record.is_community,
            created_at: record.created_at,
            source_repo: record.source_repo.clone(),
            source_branch: record.source_branch.clone(),
            source_path: record.source_path.clone(),
            visibility: parse_visibility(&record.visibility),
            is_public: record.is_public,
            flags: parse_flags(record.flags.as_deref()),
            // Temporary values
            status: ProjectStatus::Healthy,
            has_active_deployment: false,
            access_level: ProjectMemberRole::Member,
            revision: None,
            issue_stats: HashMap::new(),
            has_failing_deployment: false,
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
            issues: None,
        }
    }
}

impl From<&project_issue::Model> for ProjectIssueInfo {
    fn from(i: &project_issue::Model) -> Self {
        Self {
            id: i.id.clone(),
            level: i.level.clone(),
            deployment_id: i.deployment_id.clone(),
            r#type: i.r#type.clone(),
            subject: i.subject.clone(),
            details: i.details.clone(),
            file: i.file.clone(),
            version_name: i.version_name.clone(),
            created_at: i.created_at,
        }
    }
}

impl From<&report::Model> for ReportInfo {
    fn from(r: &report::Model) -> Self {
        Self {
            id: r.id.clone(),
            r#type: r.r#type.clone(),
            reason: r.reason.clone(),
            body: r.body.clone(),
            status: parse_report_status(&r.status),
            submitter_id: r.submitter_id.clone(),
            project_id: r.project_id.clone(),
            path: r.path.clone(),
            locale: r.locale.clone(),
            version_id: r.version_id,
            created_at: r.created_at,
        }
    }
}
