use serde::Deserialize;
use crate::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AddIssueRequestBody {
    pub level: ProjectIssueLevel,
    pub r#type: ProjectIssueType,
    pub subject: ProjectError,
    pub details: String,
    pub path: Option<String>,
}
