use serde::Deserialize;
use std::str::FromStr;

use crate::USER_AGENT;
use crate::error::ExternalResult;
use crate::platforms::{PlatformProject, ProjectType};

const MODRINTH_API: &str = "https://api.modrinth.com";
pub const PLATFORM: &str = "modrinth";

#[derive(Debug, Deserialize)]
struct ProjectResp {
    slug: String,
    name: String,
    #[serde(default)]
    link_urls: Option<LinkUrls>,
    #[serde(default)]
    project_types: Vec<String>,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinkUrls {
    #[serde(default)]
    source: Option<LinkUrl>,
}

#[derive(Debug, Deserialize)]
struct LinkUrl {
    url: String,
}

#[derive(Debug, Deserialize)]
struct UserResp {
    id: String,
}

#[derive(Debug, Deserialize)]
struct MemberResp {
    user: MemberUser,
}

#[derive(Debug, Deserialize)]
struct MemberUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OrgResp {
    members: Vec<MemberResp>,
}

#[derive(Clone)]
pub struct Modrinth {
    http: reqwest::Client,
}

impl Modrinth {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn get_project(&self, slug: &str) -> ExternalResult<Option<PlatformProject>> {
        let url = format!("{MODRINTH_API}/v3/project/{slug}");
        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let project: ProjectResp = resp.error_for_status()?.json().await?;

        let source_url = project
            .link_urls
            .and_then(|l| l.source)
            .map(|s| s.url)
            .unwrap_or_default();
        let maybe_project_type = project
            .project_types
            .first()
            .and_then(|s| ProjectType::from_str(s).ok());
        let Some(project_type) = maybe_project_type else {
            return Ok(None);
        };

        Ok(Some(PlatformProject {
            slug: project.slug,
            name: project.name,
            source_url,
            project_type,
            icon_url: project.icon_url,
            platform: PLATFORM.to_owned(),
        }))
    }

    pub async fn authenticated_user_id(&self, token: &str) -> ExternalResult<Option<String>> {
        let url = format!("{MODRINTH_API}/v3/user");
        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header(reqwest::header::AUTHORIZATION, token)
            .send()
            .await?
            .error_for_status();
        match resp {
            Ok(resp) => {
                let user: UserResp = resp.json().await?;
                Ok(Some(user.id))
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn is_project_member(&self, slug: &str, user_id: &str) -> ExternalResult<bool> {
        let members_url = format!("{MODRINTH_API}/v3/project/{slug}/members");
        let resp = self
            .http
            .get(&members_url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status();
        if let Ok(resp) = resp {
            let members: Vec<MemberResp> = resp.json().await?;
            if members.iter().any(|m| m.user.id == user_id) {
                return Ok(true);
            }
        }

        let org_url = format!("{MODRINTH_API}/v3/project/{slug}/organization");
        let resp = self
            .http
            .get(&org_url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?
            .error_for_status();

        match resp {
            Ok(resp) => {
                let org: OrgResp = resp.json().await?;
                Ok(org.members.iter().any(|m| m.user.id == user_id))
            }
            Err(_) => Ok(false),
        }
    }

    pub async fn verify_project_access(
        &self,
        project: &PlatformProject,
        modrinth_id: Option<&str>,
        repo_url: &str,
    ) -> ExternalResult<bool> {
        if !project.source_url.is_empty() && project.source_url.starts_with(repo_url) {
            return Ok(true);
        }
        if let Some(id) = modrinth_id {
            return self.is_project_member(&project.slug, id).await;
        }
        Ok(false)
    }
}
