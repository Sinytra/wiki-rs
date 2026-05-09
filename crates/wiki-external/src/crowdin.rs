use serde::{Deserialize, Serialize};

use crate::error::ExternalResult;

const CROWDIN_API: &str = "https://api.crowdin.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locale {
    pub id: String,
    pub name: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
struct ProjectResp {
    data: ProjectData,
}

#[derive(Debug, Deserialize)]
struct ProjectData {
    #[serde(rename = "targetLanguages")]
    target_languages: Vec<TargetLanguage>,
}

#[derive(Debug, Deserialize)]
struct TargetLanguage {
    id: String,
    name: String,
    locale: String,
}

impl From<TargetLanguage> for Locale {
    fn from(t: TargetLanguage) -> Self {
        Self {
            id: t.id,
            name: t.name,
            code: t.locale.to_lowercase().replace('-', "_"),
        }
    }
}

#[derive(Clone)]
pub struct Crowdin {
    http: reqwest::Client,
    project_id: String,
    token: String,
}

impl Crowdin {
    pub fn new(http: reqwest::Client, project_id: String, token: String) -> Self {
        Self {
            http,
            project_id,
            token,
        }
    }

    pub async fn available_locales(&self) -> ExternalResult<Vec<Locale>> {
        tracing::info!("Loading available languages from Crowdin");

        let url = format!("{CROWDIN_API}/api/v2/projects/{}", self.project_id);
        let body = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?
            .json::<ProjectResp>()
            .await?;

        let locales: Vec<Locale> = body
            .data
            .target_languages
            .into_iter()
            .map(Locale::from)
            .collect();

        tracing::info!("Loaded {} languages", locales.len());
        Ok(locales)
    }
}
