use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;

use crate::access::ProjectMemberRole;
use crate::content::GameRecipeType;
use crate::project::FileTree;
use crate::visibility::{ProjectFlag, ProjectStatus, ProjectVisibility, ReportStatus};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct BrowseProject {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub platforms: Vec<String>,
    pub is_community: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct BrowseResponse {
    pub pages: u64,
    pub total: u64,
    pub data: Vec<BrowseProject>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectInfoResponse {
    pub id: String,
    pub versions: HashMap<String, String>,
    pub tree: FileTree,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PageResponse {
    pub content: Option<String>,
    pub edit_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TreeResponse {
    pub tree: FileTree,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ContentItemResponse {
    pub content: String,
    pub edit_url: Option<String>,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ContentItemNameResponse {
    pub source: String,
    pub id: String,
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RecipeTypeResponse {
    pub r#type: GameRecipeType,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LocaleInfo {
    pub id: String,
    pub name: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DataImportInfo {
    pub id: i64,
    pub game_version: String,
    pub loader: String,
    pub loader_version: String,
    pub user_id: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SystemInfoResponse {
    pub version: String,
    pub latest_data: Option<DataImportInfo>,
    pub stats: SystemStats,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SystemStats {
    pub projects: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AccessKeyInfo {
    pub id: i64,
    pub name: String,
    pub user_id: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CreateAccessKeyResponse {
    pub key: AccessKeyBrief,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AccessKeyBrief {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AdminProjectInfo {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub visibility: ProjectVisibility,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ReportInfo {
    pub id: String,
    pub r#type: String,
    pub reason: String,
    pub body: String,
    pub status: ReportStatus,
    pub submitter_id: String,
    pub project_id: String,
    pub path: Option<String>,
    pub locale: Option<String>,
    pub version_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub platforms: HashMap<String, String>,
    pub is_community: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectDetails {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub platforms: HashMap<String, String>,
    pub is_community: bool,
    pub created_at: NaiveDateTime,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub visibility: ProjectVisibility,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Option::is_none")] // TODO empty by default
    pub flags: Option<Vec<ProjectFlag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProjectStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_active_deployment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_level: Option<ProjectMemberRole>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct UserProjectsResponse {
    pub profile: UserProfile,
    pub projects: Vec<ProjectDetails>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct UserProfile {
    pub username: String,
    pub name: String,
    pub role: String,
    pub modrinth_id: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectCreatedResponse {
    pub project: ProjectDetails,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DeploymentInfo {
    pub id: String,
    pub project_id: String,
    pub revision: Option<serde_json::Value>,
    pub status: String,
    pub active: bool,
    pub user_id: Option<String>,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub created_at: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issues: Option<Vec<ProjectIssueInfo>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectIssueInfo {
    pub id: String,
    pub level: String,
    pub deployment_id: String,
    pub r#type: String,
    pub subject: String,
    pub details: Option<String>,
    pub file: Option<String>,
    pub version_name: Option<String>,
    pub created_at: NaiveDateTime,
}
