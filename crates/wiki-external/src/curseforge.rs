use serde::Deserialize;

use crate::error::ExternalResult;
use crate::platforms::{PlatformProject, ProjectType};

const CURSEFORGE_API: &str = "https://api.curseforge.com";
const MC_GAME_ID: u32 = 432;
pub const PLATFORM: &str = "curseforge";

fn class_to_type(class_id: i64) -> ProjectType {
    match class_id {
        6 => ProjectType::Mod,
        12 => ProjectType::ResourcePack,
        6945 => ProjectType::DataPack,
        6552 => ProjectType::Shader,
        4471 => ProjectType::ModPack,
        5 => ProjectType::Plugin,
        _ => ProjectType::Unknown,
    }
}

#[derive(Debug, Deserialize)]
struct DataResp<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct SearchResp {
    pagination: Pagination,
    data: Vec<ProjectData>,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    #[serde(rename = "resultCount")]
    result_count: i64,
}

#[derive(Debug, Deserialize)]
struct ProjectData {
    slug: String,
    name: String,
    #[serde(rename = "classId")]
    class_id: i64,
    #[serde(default)]
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct Links {
    #[serde(rename = "sourceUrl", default)]
    source_url: Option<String>,
}

#[derive(Clone)]
pub struct CurseForge {
    http: reqwest::Client,
    api_key: String,
}

impl CurseForge {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }

    pub fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub async fn get_project(&self, slug: &str) -> ExternalResult<Option<PlatformProject>> {
        let data = self.get_project_data(slug).await?;
        Ok(data.map(|p| PlatformProject {
            slug: p.slug,
            name: p.name,
            source_url: p.links.and_then(|l| l.source_url).unwrap_or_default(),
            project_type: class_to_type(p.class_id),
            platform: PLATFORM,
        }))
    }

    async fn get_project_data(&self, slug: &str) -> ExternalResult<Option<ProjectData>> {
        if slug.chars().all(|c| c.is_ascii_digit()) && !slug.is_empty() {
            let url = format!("{CURSEFORGE_API}/v1/mods/{slug}");
            let resp = self
                .http
                .get(&url)
                .header("x-api-key", &self.api_key)
                .send()
                .await?
                .error_for_status();
            if let Ok(resp) = resp {
                let body: DataResp<ProjectData> = resp.json().await?;
                return Ok(Some(body.data));
            }
        }

        let url = format!("{CURSEFORGE_API}/v1/mods/search?gameId={MC_GAME_ID}&slug={slug}");
        let body = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?
            .error_for_status()?
            .json::<SearchResp>()
            .await?;
        if body.pagination.result_count == 1 && body.data.len() == 1 {
            return Ok(body.data.into_iter().next());
        }
        Ok(None)
    }
}
