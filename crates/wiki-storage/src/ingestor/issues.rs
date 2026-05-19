use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sea_orm::DatabaseConnection;
use tracing::{error, warn};
use wiki_db::query;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

#[derive(Debug, Clone)]
pub struct ProjectIssue {
    pub level: ProjectIssueLevel,
    pub kind: ProjectIssueType,
    pub subject: ProjectError,
    pub details: Option<String>,
    pub file: Option<PathBuf>,
}

pub trait IssueSink: Send + Sync {
    fn add(&self, issue: ProjectIssue);
    fn has_errors(&self) -> bool;
}

pub struct DbIssueSink {
    db: DatabaseConnection,
    deployment_id: String,
    version_name: Option<String>,
    has_errors: AtomicBool,
}

impl DbIssueSink {
    pub fn new(
        db: DatabaseConnection,
        deployment_id: impl Into<String>,
        version_name: Option<String>,
    ) -> Self {
        Self {
            db,
            deployment_id: deployment_id.into(),
            version_name,
            has_errors: AtomicBool::new(false),
        }
    }
}

impl IssueSink for DbIssueSink {
    fn add(&self, issue: ProjectIssue) {
        if issue.level == ProjectIssueLevel::Error {
            self.has_errors.store(true, Ordering::Relaxed);
        }

        let ProjectIssue { level, kind, subject, details, file } = issue;
        let file = file.map(|p| p.to_string_lossy().into_owned());
        let log_detail = details
            .as_ref()
            .map(|d| format!(" '{d}' "))
            .unwrap_or_default();
        let log_file = file
            .as_ref()
            .map(|f| format!(" in file {f}"))
            .unwrap_or_default();
        warn!("[Issue] {kind} / {subject}{log_detail}{log_file}");

        let db = self.db.clone();
        let deployment_id = self.deployment_id.clone();
        let version_name = self.version_name.clone();

        tokio::spawn(async move {
            if query::project_issue::get_project_issue(
                &db,
                &deployment_id,
                level,
                kind,
                file.as_deref(),
            )
            .await
            .is_ok()
            {
                return;
            }

            let new = query::project_issue::NewProjectIssue {
                deployment_id: &deployment_id,
                level,
                issue_type: kind,
                subject,
                details: details.as_deref(),
                file: file.as_deref(),
                version_name: version_name.as_deref(),
            };

            if let Err(e) = query::project_issue::add_project_issue(&db, new).await {
                error!(deployment = %deployment_id, "Failed to persist project issue: {e}");
            }
        });
    }

    fn has_errors(&self) -> bool {
        self.has_errors.load(Ordering::Relaxed)
    }
}

pub struct LoggingIssueSink {
    has_errors: AtomicBool,
}

impl Default for LoggingIssueSink {
    fn default() -> Self {
        Self { has_errors: AtomicBool::new(false) }
    }
}

impl LoggingIssueSink {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IssueSink for LoggingIssueSink {
    fn add(&self, issue: ProjectIssue) {
        if issue.level == ProjectIssueLevel::Error {
            self.has_errors.store(true, Ordering::Relaxed);
        }
        let ProjectIssue { level: _, kind, subject, details, file } = issue;
        let log_detail = details
            .as_ref()
            .map(|d| format!(" '{d}' "))
            .unwrap_or_default();
        let log_file = file
            .as_ref()
            .map(|f| format!(" in file {}", f.display()))
            .unwrap_or_default();
        warn!("[Issue] {kind} / {subject}{log_detail}{log_file}");
    }

    fn has_errors(&self) -> bool {
        self.has_errors.load(Ordering::Relaxed)
    }
}

pub struct FileIssues<'a> {
    sink: &'a dyn IssueSink,
    file: PathBuf,
}

impl<'a> FileIssues<'a> {
    pub fn new(sink: &'a dyn IssueSink, file: impl Into<PathBuf>) -> Self {
        Self { sink, file: file.into() }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn add(
        &self,
        level: ProjectIssueLevel,
        kind: ProjectIssueType,
        subject: ProjectError,
        details: impl Into<Option<String>>,
    ) {
        self.sink.add(ProjectIssue {
            level,
            kind,
            subject,
            details: details.into(),
            file: Some(self.file.clone()),
        });
    }

    pub fn error(&self, subject: ProjectError, details: impl Into<String>) {
        self.add(
            ProjectIssueLevel::Error,
            ProjectIssueType::Ingestor,
            subject,
            Some(details.into()),
        );
    }

    pub fn warn(&self, subject: ProjectError, details: impl Into<String>) {
        self.add(
            ProjectIssueLevel::Warning,
            ProjectIssueType::Ingestor,
            subject,
            Some(details.into()),
        );
    }
}
