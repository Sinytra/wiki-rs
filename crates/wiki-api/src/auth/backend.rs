use axum_login::{AuthUser as AxumAuthUser, AuthnBackend, UserId};
use oauth2::basic::{BasicClient, BasicRequestTokenError};
use oauth2::reqwest;
use oauth2::url::Url;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use wiki_db::entity::user;
use wiki_db::query;
use wiki_external::github::GitHub;

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

pub type BasicClientSet =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn build_oauth_client(
    client_id: String,
    client_secret: String,
    redirect_url: String,
) -> Result<BasicClientSet, oauth2::url::ParseError> {
    let auth_url = AuthUrl::new(GITHUB_AUTH_URL.to_owned())?;
    let token_url = TokenUrl::new(GITHUB_TOKEN_URL.to_owned())?;
    let redirect_url = RedirectUrl::new(redirect_url)?;

    Ok(BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    #[serde(skip)]
    pub access_token: Option<String>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("avatar_url", &self.avatar_url)
            .field("access_token", &"[redacted]")
            .finish()
    }
}

impl AxumAuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.id.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.id.as_bytes()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub code: String,
    pub old_state: CsrfToken,
    pub new_state: CsrfToken,
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error(transparent)]
    Db(#[from] wiki_db::error::DbError),
    #[error(transparent)]
    Sea(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    OAuth2(BasicRequestTokenError<<reqwest::Client as oauth2::AsyncHttpClient<'static>>::Error>),
    #[error("decode error: {0}")]
    Decode(String),
}

#[derive(Clone)]
pub struct AuthBackend {
    db: DatabaseConnection,
    client: BasicClientSet,
    http_client: reqwest::Client,
    github: GitHub,
}

impl AuthBackend {
    pub fn new(db: DatabaseConnection, client: BasicClientSet) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client should build");
        Self {
            db,
            client,
            github: GitHub::new(http_client.clone()),
            http_client,
        }
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        self.client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("read:user".to_owned()))
            .add_scope(Scope::new("read:org".to_owned()))
            .url()
    }
}

impl AuthnBackend for AuthBackend {
    type User = User;
    type Credentials = Credentials;
    type Error = BackendError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        if creds.old_state.secret() != creds.new_state.secret() {
            return Ok(None);
        }

        let token_res = self
            .client
            .exchange_code(AuthorizationCode::new(creds.code))
            .request_async(&self.http_client)
            .await
            .map_err(BackendError::OAuth2)?;

        let access_token = token_res.access_token().secret().clone();
        let profile = self.github.get_user_profile(access_token.as_str()).await?;

        let id = profile.login.to_lowercase();
        query::user::create_if_not_exists(&self.db, &id).await?;

        Ok(Some(User {
            id,
            username: profile.login,
            avatar_url: profile.avatar_url,
            access_token: Some(access_token),
        }))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let model = user::Entity::find_by_id(user_id.clone())
            .one(&self.db)
            .await?;
        Ok(model.map(|m| User {
            id: m.id.clone(),
            username: m.id,
            avatar_url: None,
            access_token: None,
        }))
    }
}

pub type AuthSession = axum_login::AuthSession<AuthBackend>;
