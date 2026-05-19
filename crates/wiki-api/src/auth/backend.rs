use crate::auth::github::GitHubOAuth;
use axum_login::{AuthUser as AxumAuthUser, AuthnBackend, UserId};
use chrono::{DateTime, Utc};
use oauth2::basic::BasicRequestTokenError;
use oauth2::reqwest;
use oauth2::CsrfToken;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use oauth2::url::Url;
use wiki_db::entity::user;
use wiki_db::query;
use wiki_external::github::{GitHub, GithubProfile};
use wiki_system::MemoryCache;

const DURATION_ONE_WEEK: Duration = Duration::from_secs(60 * 60 * 24 * 7);

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub role: String,
    pub modrinth_id: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>
}

impl User {
    fn new (model: user::Model, profile: GithubProfile) -> Self {
        Self {
            id: model.id,
            name: profile.name,
            role: model.role,
            modrinth_id: model.modrinth_id,
            avatar_url: profile.avatar_url,
            created_at: model.created_at.and_utc()
        }
    }
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
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
    #[error(transparent)]
    System(#[from] wiki_system::SystemError),
}

#[derive(Clone)]
pub struct AuthBackend {
    db: DatabaseConnection,
    cache: Arc<MemoryCache>,
    client: GitHubOAuth,
    http_client: reqwest::Client,
    github: GitHub,
}

impl AuthBackend {
    pub fn new(db: DatabaseConnection, cache: Arc<MemoryCache>, client: GitHubOAuth) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("reqwest client should build");
        Self {
            db,
            cache,
            client,
            github: GitHub::new(http_client.clone()),
            http_client,
        }
    }

    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        self.client.authorize_url()
    }

    fn user_profile_key(user_id: &str) -> String {
        format!("github:profile:{}", user_id)
    }

    async fn get_user_profile(&self, user_id: &str) -> Result<Option<GithubProfile>, BackendError> {
        let key = Self::user_profile_key(user_id);
        let profile = self.cache.get_json::<GithubProfile>(&key).await?;
        Ok(profile)
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

        let access_token = self.client.exchange_code(creds.code).await?;
        let profile = self.github.get_user_profile(&access_token).await?;

        let user_id = profile.login.to_lowercase();
        let user = query::user::create_if_not_exists(&self.db, &user_id).await?;

        // Cache profile
        let profile_key = Self::user_profile_key(&user_id);
        let fresh_key = profile_key.clone() + ":fresh";
        if !self.cache.exists(&fresh_key).await? {
            self
                .cache
                .set_json(&profile_key, &profile, Duration::from_secs(0))
                .await?;
            // Revalidate after 1 week
            self.cache.set(&fresh_key, "1", DURATION_ONE_WEEK).await?;
        }

        Ok(Some(User::new(user, profile)))
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let model = user::Entity::find_by_id(user_id.clone())
            .one(&self.db)
            .await?;
        let profile = self.get_user_profile(user_id).await?.unwrap_or_else(|| {
            GithubProfile {
                login: user_id.clone(),
                name: user_id.clone(),
                avatar_url: None
            }
        });
        Ok(model.map(|m| User::new(m, profile)))
    }
}

pub type AuthSession = axum_login::AuthSession<AuthBackend>;
