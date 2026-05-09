use crate::error::ExternalResult;

#[derive(Clone)]
pub struct Frontend {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl Frontend {
    pub fn new(http: reqwest::Client, base_url: String, api_key: String) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
        }
    }

    pub async fn revalidate_project(&self, id: &str) -> ExternalResult<()> {
        tracing::debug!("Revalidating frontend project '{id}'");

        let url = format!("{}/api/docs/{id}/revalidate", self.base_url);
        self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
