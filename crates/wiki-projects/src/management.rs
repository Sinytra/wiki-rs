use garde::Validate;
use sea_orm::{DatabaseConnection, Set};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, warn};
use url::Url;
use uuid::Uuid;

use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::error::{DomainError, DomainResult, ProjectError};
use wiki_domain::metadata::ProjectMetadata;
use wiki_external::curseforge;
use wiki_external::modrinth;
use wiki_external::platforms::{PlatformProject, Platforms};
use wiki_storage::deployment::DeploymentManager;

const ALLOWED_PROTOCOLS: &[&str] = &["http", "https"];

use crate::access::Actor;
pub use curseforge::PLATFORM as PLATFORM_CURSEFORGE;
pub use modrinth::PLATFORM as PLATFORM_MODRINTH;
use wiki_domain::content::ResourceLocation;
use wiki_domain::project::ProjectType;
use wiki_domain::visibility::{ProjectFlags, ProjectVisibility};

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RegistrationInput {
    #[garde(length(min = 1))]
    pub repo: String,

    #[garde(length(min = 1))]
    pub branch: String,

    #[garde(length(min = 1))]
    pub path: String,
}

#[derive(Debug)]
pub struct ValidatedProjectData {
    pub project: project::ActiveModel,
    pub platforms: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ProjectCoords<'a> {
    id: &'a str,
    repo: &'a str,
    platform: &'a str,
    slug: &'a str,
}

pub fn process_platforms(
    metadata: &ProjectMetadata,
    platforms: &Platforms,
) -> HashMap<String, String> {
    let valid = platforms.available_platforms();
    let mut result = HashMap::new();

    if let Some(declared) = &metadata.platforms {
        for platform in &valid {
            if let Some(slug) = declared.get(*platform) {
                result.insert((*platform).to_owned(), slug.clone());
            }
        }
    }

    if let (Some(platform), Some(slug)) = (metadata.platform.as_deref(), metadata.slug.as_deref())
        && !result.contains_key(platform)
    {
        result.insert(platform.to_owned(), slug.to_owned());
    }

    result
}

async fn is_project_publicly_browsable(client: &reqwest::Client, repo: &str) -> bool {
    if !repo.starts_with("https://") {
        return false;
    }
    match client.get(repo).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(e) => {
            warn!("Error checking public status of {repo}: {e}");
            false
        }
    }
}

fn is_owner(owners: &[String], username: &str) -> bool {
    let lower = username.to_lowercase();
    owners.iter().any(|o| o.to_lowercase() == lower)
}

pub async fn validate_platform(
    db: &DatabaseConnection,
    platforms: &Platforms,
    ProjectCoords {
        id,
        repo,
        platform,
        slug,
    }: ProjectCoords<'_>,
    check_existing: bool,
    user: &Actor,
    local_env: bool,
) -> DomainResult<PlatformProject> {
    if platform == curseforge::PLATFORM && !platforms.curseforge.is_available() {
        // TODO Use ProjectError
        return Err(DomainError::BadRequest("cf_unavailable".into()));
    }

    let platform_proj = platforms
        .get_project(platform, slug)
        .await
        .map_err(|e| DomainError::Internal(format!("platform lookup failed: {e}")))?
        .ok_or_else(|| DomainError::BadRequest("no_project".into()))?; // TODO ProjectError

    let mut skip_check = local_env;
    if !skip_check
        && check_existing
        && let Ok(existing) = query::project::find_by_id(db, id).await
        && existing.platforms.0.get(platform).map(String::as_str) == Some(slug)
    {
        skip_check = true;
    }

    if !skip_check {
        let verified = verify_project_access(
            platforms,
            platform,
            &platform_proj,
            user.modrinth_id.as_deref(),
            repo,
        )
        .await
        .map_err(|e| DomainError::Internal(format!("verify access failed: {e}")))?;
        if !verified {
            let can_verify_mr = platform == modrinth::PLATFORM && user.modrinth_id.is_none();
            return Err(DomainError::OwnershipUnverified {
                platform: platform.to_owned(),
                can_verify_mr,
            });
        }
    }

    Ok(platform_proj)
}

async fn verify_project_access(
    platforms: &Platforms,
    platform: &str,
    project: &PlatformProject,
    modrinth_user_id: Option<&str>,
    repo_url: &str,
) -> Result<bool, wiki_external::error::ExternalError> {
    match platform {
        modrinth::PLATFORM => {
            platforms
                .modrinth
                .verify_project_access(project, modrinth_user_id, repo_url)
                .await
        }
        _ => Ok(!project.source_url.is_empty() && project.source_url.starts_with(repo_url)),
    }
}

pub async fn validate_project_data(
    db: &DatabaseConnection,
    deployments: &DeploymentManager,
    platforms: &Platforms,
    http: &reqwest::Client,
    input: RegistrationInput,
    user: &Actor,
    check_existing: bool,
    local_env: bool,
) -> DomainResult<ValidatedProjectData> {
    let parsed = Url::parse(&input.repo).map_err(|e| {
        error!(
            "Invalid repository URL provided: {} Error: {}",
            input.repo, e
        );
        DomainError::Project {
            error: ProjectError::NoRepository,
            message: "Invalid repository URL".into(),
        }
    })?;
    if !local_env && !ALLOWED_PROTOCOLS.contains(&parsed.scheme()) {
        return Err(DomainError::Project {
            error: ProjectError::NoRepository,
            message: "Unsupported repository URL".into(),
        });
    }

    let temp_id = format!("_temp-{}", Uuid::new_v4().simple());
    let temp_record = project::Model {
        id: temp_id,
        name: String::new(),
        source_path: input.path.clone(),
        source_repo: input.repo.clone(),
        source_branch: input.branch.clone(),
        is_community: false,
        r#type: ProjectType::Mod,
        platforms: project::Platforms(HashMap::default()),
        search_vector: None,
        created_at: chrono::Utc::now().naive_utc(),
        is_public: false,
        modid: None,
        is_virtual: false,
        visibility: ProjectVisibility::Public,
        flags: ProjectFlags::empty().bits(),
    };

    let resolved = deployments
        .validate_temp_project(&temp_record)
        .await?;

    if !local_env
        && !check_existing
        && let Some(owners) = &resolved.owners
        && !is_owner(owners, &user.username)
    {
        return Err(DomainError::Project {
            error: ProjectError::NotOwner,
            message: "User is missing from project owners".into(),
        });
    }

    let id = resolved.id.clone();
    if ResourceLocation::BUILTIN_NAMESPACES.contains(&id.as_str()) {
        return Err(DomainError::Project {
            error: ProjectError::IllegalId,
            message: "Project ID is unavailable".into(),
        });
    }

    let modid = resolved.modid.clone().unwrap_or_default();
    if !modid.is_empty() && (ResourceLocation::BUILTIN_NAMESPACES.contains(&modid.as_str())) {
        return Err(DomainError::Project {
            error: ProjectError::IllegalId,
            message: "Project mod ID is unavailable".into(),
        });
    }

    let platforms_map = process_platforms(&resolved, platforms);
    if platforms_map.is_empty() {
        return Err(DomainError::Project {
            error: ProjectError::NoPlatforms,
            message: "No hosting platforms specified".into(),
        });
    }

    let mut platform_projects: HashMap<String, PlatformProject> = HashMap::new();
    for (platform, slug) in &platforms_map {
        let coords: ProjectCoords = ProjectCoords {
            id: &id,
            repo: &input.repo,
            platform,
            slug,
        };

        let pp = validate_platform(db, platforms, coords, check_existing, user, local_env).await?;
        platform_projects.insert(platform.clone(), pp);
    }

    let preferred = platform_projects
        .get(modrinth::PLATFORM)
        .or_else(|| platform_projects.values().next())
        .expect("non-empty platform_projects");

    let is_public = is_project_publicly_browsable(http, &input.repo).await;

    let mut active = project::ActiveModel {
        id: Set(id),
        name: Set(preferred.name.clone()),
        source_repo: Set(input.repo),
        source_branch: Set(input.branch),
        source_path: Set(input.path),
        r#type: Set(preferred.project_type),
        platforms: Set(project::Platforms(platforms_map.clone())),
        is_public: Set(is_public),
        ..Default::default()
    };
    if !modid.is_empty() {
        active.modid = Set(Some(modid));
    }

    Ok(ValidatedProjectData {
        project: active,
        platforms: platforms_map,
    })
}

pub fn enqueue_deploy(
    deployments: Arc<DeploymentManager>,
    record: project::Model,
    user_id: Option<String>,
) {
    tokio::spawn(async move {
        debug!(
            "Deploying project '{}' from branch '{}'",
            record.id, record.source_branch
        );
        match deployments.deploy(&record, user_id.as_deref()).await {
            Ok(()) => debug!("Project '{}' deployed successfully", record.id),
            Err(e) => error!(
                "Encountered error while deploying project '{}': {e}",
                record.id
            ),
        }
    });
}
