use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};

const GITHUB_USER_URL: &str = "https://api.github.com/user";

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubProfile {
    pub login: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone)]
pub struct GitHub {
    http: reqwest::Client,
}

impl GitHub {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn get_user_profile(&self, access_token: &str) -> reqwest::Result<GithubProfile> {
        self.http
            .get(GITHUB_USER_URL)
            .header(USER_AGENT.as_str(), USER_AGENT)
            .bearer_auth(access_token)
            .send()
            .await?
            .error_for_status()?
            .json::<GithubProfile>()
            .await
    }
}
