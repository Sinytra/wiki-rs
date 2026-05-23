use std::collections::HashMap;

use crate::access::ProjectMemberRole;
use crate::content::{GameRecipeType, ResolvedItem};
use crate::error::{ProjectError, ProjectIssueLevel, ProjectIssueStats, ProjectIssueType};
use crate::project::{FileTree, ProjectType};
use crate::visibility::{ProjectFlag, ProjectStatus, ProjectVisibility, ReportReason, ReportStatus, ReportType};
use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::prelude::StringLen;
use sea_orm::{DeriveActiveEnum, EnumIter, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumString,
    AsRefStr,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum UserRole {
    User,
    Admin,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct BrowseProject {
    pub id: String,
    pub name: String,
    pub r#type: ProjectType,
    pub platforms: HashMap<String, String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectLicense {
    pub id: Option<String>,
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectLicenses {
    pub project: Option<ProjectLicense>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectInfo {
    pub page_count: u64,
    pub content_count: u64,
    pub licenses: ProjectLicenses,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectData {
    // Base
    pub id: String,
    pub name: String,
    pub r#type: ProjectType,
    pub platforms: HashMap<String, String>,
    pub is_community: bool,
    pub created_at: NaiveDateTime,
    pub source_repo: Option<String>,
    // Docs
    pub versions: Vec<String>,
    pub locales: Vec<String>,
    pub local: bool,
    pub info: ProjectInfo,
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
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ContentItemNameResponse {
    pub source: String,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RecipeTypeResponse {
    pub r#type: GameRecipeType,
    pub workbenches: Vec<ResolvedItem>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LocaleInfo {
    pub id: String,
    pub name: String,
    pub code: String,
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
    pub users: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AccessKeyInfo {
    pub id: i64,
    pub name: String,
    pub user_id: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub expired: bool
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
    pub expires_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AdminProjectInfo {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub visibility: ProjectVisibility,
    pub mod_id: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ReportInfo {
    pub id: String,
    pub r#type: ReportType,
    pub reason: ReportReason,
    pub body: String,
    pub status: ReportStatus,
    pub submitter_id: String,
    pub project_id: String,
    pub path: Option<String>,
    pub locale: Option<String>,
    pub version: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub r#type: ProjectType,
    pub platforms: HashMap<String, String>,
    pub is_community: bool,
    pub source_repo: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DevProjectData {
    pub id: String,
    pub name: String,
    pub r#type: ProjectType,
    pub platforms: HashMap<String, String>,
    pub is_community: bool,
    pub mod_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub visibility: ProjectVisibility,
    pub is_public: bool,
    pub flags: Vec<ProjectFlag>,
    pub status: ProjectStatus,
    pub has_active_deployment: bool,
    pub access_level: ProjectMemberRole,
    pub revision: Option<GitRevision>,
    pub issue_stats: ProjectIssueStats,
    pub has_failing_deployment: bool,
    pub version_names: Vec<String>
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct UserProjectsResponse {
    pub profile: UserProfile,
    pub projects: Vec<DevProjectData>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct UserProfile {
    pub username: String,
    pub name: String,
    pub role: UserRole,
    pub modrinth_id: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectCreatedResponse {
    pub project: DevProjectData,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[serde(rename_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "snake_case"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DeploymentStatus {
    Created,
    Loading,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DeploymentEvent {
    Created {
        deployment_id: String,
    },
    Loading {
        deployment_id: String,
    },
    Revision {
        deployment_id: String,
        revision: GitRevision,
    },
    Success {
        deployment_id: String,
    },
    Error {
        deployment_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, FromJsonQueryResult)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GitRevision {
    pub hash: String,
    pub full_hash: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DeploymentInfo {
    pub id: String,
    pub project_id: String,
    pub revision: Option<GitRevision>,
    pub status: DeploymentStatus,
    pub active: bool,
    pub user_id: Option<String>,
    pub source_repo: String,
    pub source_branch: String,
    pub source_path: String,
    pub created_at: NaiveDateTime,
    pub issues: Vec<ProjectIssueInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectIssueInfo {
    pub id: String,
    pub level: ProjectIssueLevel,
    pub deployment_id: String,
    pub r#type: ProjectIssueType,
    pub subject: ProjectError,
    pub details: Option<String>,
    pub file: Option<String>,
    pub version_name: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectVersionData {
    pub name: Option<String>,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DataMigration {
    pub id: String,
    pub title: String,
    pub desc: String,
}