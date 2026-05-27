use sea_orm::prelude::StringLen;
use sea_orm::{DeriveActiveEnum, EnumIter};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,
    #[error("version not found")]
    VersionNotFound,
    #[error("no active deployment")]
    NoActiveDeployment,
    #[error("checkout missing on disk")]
    CheckoutMissing,
    #[error("project error")]
    Project {
        error: ProjectError,
        message: String
    },
    #[error("failed to verify ownership")]
    OwnershipUnverified {
        platform: String,
        can_verify_mr: bool,
    },
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::AsRefStr,
    strum::EnumString,
    strum::IntoStaticStr,
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
pub enum ProjectError {
    RequiresAuth,
    NoRepository,
    RepoTooLarge,
    NoBranch,
    NoPath,
    InvalidMeta,
    PageRender,
    UnknownRecipeType,
    InvalidIngredient,
    InvalidFile,
    InvalidFormat,
    InvalidResloc,
    InvalidVersionBranch,
    InvalidFrontmatter,
    MissingPlatformProject,
    NoPageTitle,
    MissingRequiredAttribute,
    IllegalId,
    IllegalModId,
    NotOwner,
    NoPlatforms,
    Unknown,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::AsRefStr,
    strum::EnumString,
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
pub enum ProjectIssueLevel {
    Warning,
    Error,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::AsRefStr,
    strum::EnumString,
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
pub enum ProjectIssueType {
    Meta,
    File,
    GitClone,
    GitInfo,
    Page,
    Ingestor,
    VersionSetup,
    Internal,
}

pub type ProjectIssueStats = HashMap<ProjectIssueLevel, u64>;
