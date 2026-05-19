use oauth2::basic::{BasicClient, BasicRequestTokenError};
use oauth2::http::{HeaderValue, header::AUTHORIZATION};
use oauth2::url::Url;
use oauth2::{
    AsyncHttpClient, AuthType, AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet,
    EndpointSet, HttpClientError, HttpRequest, RedirectUrl, Scope, TokenResponse, TokenUrl,
};

const MR_AUTH_URL: &str = "https://modrinth.com/auth/authorize";
const MR_TOKEN_URL: &str = "https://api.modrinth.com/_internal/oauth/token";
const MR_SCOPE: &str = "USER_READ";

pub type ModrinthClientSet =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn build_modrinth_oauth_client(
    client_id: String,
    redirect_url: String,
) -> Result<ModrinthClientSet, oauth2::url::ParseError> {
    let auth_url = AuthUrl::new(MR_AUTH_URL.to_owned())?;
    let token_url = TokenUrl::new(MR_TOKEN_URL.to_owned())?;
    let redirect_url = RedirectUrl::new(redirect_url)?;

    Ok(BasicClient::new(ClientId::new(client_id))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url)
        .set_auth_type(AuthType::RequestBody))
}

#[derive(Debug, thiserror::Error)]
pub enum ModrinthOAuthError {
    #[error(transparent)]
    OAuth2(BasicRequestTokenError<HttpClientError<reqwest::Error>>),
}

// Modrinth's /_internal/oauth/token expects the client_secret as a raw
// Authorization header, so we wrap reqwest to inject it
#[derive(Clone)]
pub struct ModrinthHttpClient {
    inner: reqwest::Client,
    secret: HeaderValue,
}

impl ModrinthHttpClient {
    pub fn new(inner: reqwest::Client, client_secret: &str) -> Self {
        Self {
            inner,
            secret: HeaderValue::from_str(client_secret)
                .expect("modrinth client secret must be a valid header value"),
        }
    }
}

impl<'c> AsyncHttpClient<'c> for ModrinthHttpClient {
    type Error = <reqwest::Client as AsyncHttpClient<'c>>::Error;
    type Future = <reqwest::Client as AsyncHttpClient<'c>>::Future;

    fn call(&'c self, mut request: HttpRequest) -> Self::Future {
        request
            .headers_mut()
            .insert(AUTHORIZATION, self.secret.clone());
        self.inner.call(request)
    }
}

#[derive(Clone)]
pub struct ModrinthOAuth {
    client: ModrinthClientSet,
    http: ModrinthHttpClient,
}

impl ModrinthOAuth {
    pub fn new(client: ModrinthClientSet, client_secret: &str) -> Self {
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client should build");
        let wrapped = ModrinthHttpClient::new(http, client_secret);
        Self {
            client,
            http: wrapped,
        }
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        self.client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(MR_SCOPE.to_owned()))
            .url()
    }

    pub async fn exchange_code(&self, code: String) -> Result<String, ModrinthOAuthError> {
        let token_res = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(&self.http)
            .await
            .map_err(ModrinthOAuthError::OAuth2)?;
        Ok(token_res.access_token().secret().clone())
    }
}
