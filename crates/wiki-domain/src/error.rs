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
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
    strum::Display, strum::AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    Unknown
}

impl ProjectError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::RequiresAuth => "requires_auth",
            Self::NoRepository => "no_repository",
            Self::RepoTooLarge => "repo_too_large",
            Self::NoBranch => "no_branch",
            Self::NoPath => "no_path",
            Self::InvalidMeta => "invalid_meta",
            Self::PageRender => "page_render",
            Self::DuplicatePage => "duplicate_page",
            Self::UnknownRecipeType => "unknown_recipe_type",
            Self::InvalidIngredient => "invalid_ingredient",
            Self::InvalidFile => "invalid_file",
            Self::InvalidFormat => "invalid_format",
            Self::InvalidResloc => "invalid_resloc",
            Self::InvalidVersionBranch => "invalid_version_branch",
            Self::InvalidFrontmatter => "invalid_frontmatter",
            Self::MissingPlatformProject => "missing_platform_project",
            Self::NoPageTitle => "no_page_title",
            Self::MissingRequiredAttribute => "missing_required_attribute",
            Self::Unknown => "unknown"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectIssueLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectIssueType {
    Meta,
    File,
    GitClone,
    GitInfo,
    Page,
    Ingestor,
    Internal,
}
