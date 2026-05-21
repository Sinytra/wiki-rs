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
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
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
    strum::IntoStaticStr,
    EnumIter,
    DeriveActiveEnum,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectError {
    Ok,
    RequiresAuth,
    NoRepository,
    RepoTooLarge,
    NoBranch,
    NoPath,
    InvalidMeta,
    PageRender,
    DuplicatePage,
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
    rename_all = "SCREAMING_SNAKE_CASE"
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
    rename_all = "SCREAMING_SNAKE_CASE"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectIssueType {
    Meta,
    File,
    GitClone,
    GitInfo,
    Page,
    Ingestor,
    Internal,
}

pub type ProjectIssueStats = HashMap<ProjectIssueLevel, u64>;
