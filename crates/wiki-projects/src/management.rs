use std::collections::HashMap;
use std::sync::Arc;

use sea_orm::{DatabaseConnection, Set};
use tracing::{debug, error, warn};
use url::Url;
use uuid::Uuid;

use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::error::DomainError;
use wiki_domain::metadata::ProjectMetadata;
use wiki_external::curseforge;
use wiki_external::modrinth;
use wiki_external::platforms::{PlatformProject, Platforms};
use wiki_storage::deployment::DeploymentManager;

const ALLOWED_PROTOCOLS: &[&str] = &["http", "https"];
const DEFAULT_NAMESPACE: &str = "minecraft";
const COMMON_NAMESPACE: &str = "c";

pub use curseforge::PLATFORM as PLATFORM_CURSEFORGE;
pub use modrinth::PLATFORM as PLATFORM_MODRINTH;
use wiki_domain::project::ProjectType;

#[derive(Debug, Clone)]
pub struct RegistrationInput {
    pub repo: String,
    pub branch: String,
    pub path: String,
}

#[derive(Debug)]
pub struct ValidatedProjectData {
    pub project: project::ActiveModel,
    pub platforms: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ActorUser {
    pub id: String,
    pub modrinth_id: Option<String>,
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
    user: &ActorUser,
    local_env: bool,
) -> Result<PlatformProject, DomainError> {
    if platform == curseforge::PLATFORM && !platforms.curseforge.is_available() {
        return Err(DomainError::BadRequest("cf_unavailable".into()));
    }

    let platform_proj = platforms
        .get_project(platform, slug)
        .await
        .map_err(|e| DomainError::Internal(format!("platform lookup failed: {e}")))?
        .ok_or_else(|| DomainError::BadRequest(format!("no_project (Platform: {platform})")))?;

    if platform_proj.project_type == ProjectType::Unknown {
        return Err(DomainError::BadRequest(format!(
            "unsupported_type (Platform: {platform})"
        )));
    }

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
            // TODO this should return json
            return Err(DomainError::BadRequest(format!(
                "ownership (Platform: {platform}, can_verify_mr: {can_verify_mr})"
            )));
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
    user: &ActorUser,
    check_existing: bool,
    local_env: bool,
) -> Result<ValidatedProjectData, DomainError> {
    let parsed = Url::parse(&input.repo).map_err(|e| {
        error!(
            "Invalid repository URL provided: {} Error: {}",
            input.repo, e
        );
        DomainError::BadRequest("no_repository".into())
    })?;
    if !local_env && !ALLOWED_PROTOCOLS.contains(&parsed.scheme()) {
        return Err(DomainError::BadRequest("no_repository".into()));
    }

    let temp_id = format!("_temp-{}", Uuid::new_v4().simple());
    let temp_record = project::Model {
        id: temp_id,
        name: String::new(),
        source_path: input.path.clone(),
        source_repo: input.repo.clone(),
        source_branch: input.branch.clone(),
        is_community: false,
        r#type: ProjectType::Unknown,
        platforms: project::Platforms(HashMap::default()),
        search_vector: None,
        created_at: chrono::Utc::now().naive_utc(),
        is_public: false,
        modid: None,
        is_virtual: false,
        visibility: "public".into(),
        flags: None,
    };

    let resolved = deployments
        .validate_temp_project(&temp_record)
        .await
        .map_err(|e| match e {
            wiki_storage::error::StorageError::Project { error, message } => {
                DomainError::BadRequest(format!("{} ({message})", error.as_ref()))
            }
            other => DomainError::Internal(other.to_string()),
        })?;

    if !local_env
        && !check_existing
        && let Some(owners) = &resolved.owners
        && !is_owner(owners, &user.id)
    {
        return Err(DomainError::BadRequest("not_owner".into()));
    }

    let id = resolved.id.clone();
    if id == DEFAULT_NAMESPACE {
        return Err(DomainError::BadRequest("illegal_id".into()));
    }

    let modid = resolved.modid.clone().unwrap_or_default();
    if !modid.is_empty() && (modid == DEFAULT_NAMESPACE || modid == COMMON_NAMESPACE) {
        return Err(DomainError::BadRequest("illegal_modid".into()));
    }

    let platforms_map = process_platforms(&resolved, platforms);
    if platforms_map.is_empty() {
        return Err(DomainError::BadRequest("no_platforms".into()));
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
    user_id: String,
) {
    tokio::spawn(async move {
        debug!(
            "Deploying project '{}' from branch '{}'",
            record.id, record.source_branch
        );
        match deployments.deploy(&record, Some(&user_id)).await {
            Ok(()) => debug!("Project '{}' deployed successfully", record.id),
            Err(e) => error!(
                "Encountered error while deploying project '{}': {e}",
                record.id
            ),
        }
    });
}
