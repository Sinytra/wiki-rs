use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
}

// TODO DB Sink
#[derive(Default)]
pub struct MemoryIssueSink {
    issues: Mutex<Vec<ProjectIssue>>,
}

impl MemoryIssueSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take(&self) -> Vec<ProjectIssue> {
        std::mem::take(&mut *self.issues.lock().unwrap())
    }

    pub fn snapshot(&self) -> Vec<ProjectIssue> {
        self.issues.lock().unwrap().clone()
    }
}

impl IssueSink for MemoryIssueSink {
    fn add(&self, issue: ProjectIssue) {
        self.issues.lock().unwrap().push(issue);
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
