use crate::auth::BackendError;
use oauth2::basic::BasicClient;
use oauth2::url::Url;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

pub type GitHubClientSet =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn build_github_oauth_client(
    client_id: String,
    client_secret: String,
    redirect_url: String,
) -> Result<GitHubClientSet, oauth2::url::ParseError> {
    let auth_url = AuthUrl::new(GITHUB_AUTH_URL.to_owned())?;
    let token_url = TokenUrl::new(GITHUB_TOKEN_URL.to_owned())?;
    let redirect_url = RedirectUrl::new(redirect_url)?;

    Ok(BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url))
}

#[derive(Clone)]
pub struct GitHubOAuth {
    client: GitHubClientSet,
    http: reqwest::Client,
}

impl GitHubOAuth {
    pub fn new(client: GitHubClientSet) -> Self {
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client should build");
        Self { client, http }
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        self.client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("read:user".to_owned()))
            .add_scope(Scope::new("read:org".to_owned()))
            .url()
    }

    pub async fn exchange_code(&self, code: String) -> Result<String, BackendError> {
        let token_res = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(&self.http)
            .await
            .map_err(BackendError::OAuth2)?;
        Ok(token_res.access_token().secret().clone())
    }
}
