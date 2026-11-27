use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

const GITHUB_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";
const GITHUB_OIDC_JWKS_URL: &str = "https://token.actions.githubusercontent.com/.well-known/jwks";
const JWKS_TTL: Duration = Duration::from_secs(60 * 60);
const JWKS_REFRESH: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Deserialize)]
pub struct GithubOidcClaims {
    pub repository: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub actor: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GithubOidcError {
    #[error("invalid token: {0}")]
    InvalidToken(#[from] jsonwebtoken::errors::Error),
    #[error("unknown signing key")]
    UnknownKey,
    #[error("failed to fetch signing keys: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Clone)]
struct CachedJwks {
    keys: Arc<JwkSet>,
    fetched_at: Instant,
}

pub struct GithubOidc {
    http: reqwest::Client,
    validation: Validation,
    jwks: Mutex<Option<CachedJwks>>,
}

impl GithubOidc {
    pub fn new(http: reqwest::Client, audience: &str) -> Self {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[audience]);
        validation.set_issuer(&[GITHUB_OIDC_ISSUER]);
        validation.set_required_spec_claims(&["exp", "aud", "iss"]);

        Self {
            http,
            validation,
            jwks: Mutex::new(None),
        }
    }

    pub async fn verify(&self, token: &str) -> Result<GithubOidcClaims, GithubOidcError> {
        let kid = decode_header(token)?
            .kid
            .ok_or(GithubOidcError::UnknownKey)?;
        let jwk = self.find_key(&kid).await?;
        let key = DecodingKey::from_jwk(&jwk)?;

        Ok(decode::<GithubOidcClaims>(token, &key, &self.validation)?.claims)
    }

    async fn find_key(&self, kid: &str) -> Result<Jwk, GithubOidcError> {
        let cached = self.jwks.lock().expect("jwks cache lock poisoned").clone();

        if let Some(cached) = cached {
            let age = cached.fetched_at.elapsed();
            if age < JWKS_TTL
                && let Some(jwk) = cached.keys.find(kid)
            {
                return Ok(jwk.clone());
            }
            if age < JWKS_REFRESH {
                return Err(GithubOidcError::UnknownKey);
            }
        }

        let keys = self.fetch_keys().await?;
        keys.find(kid).cloned().ok_or(GithubOidcError::UnknownKey)
    }

    async fn fetch_keys(&self) -> Result<Arc<JwkSet>, GithubOidcError> {
        let keys = self
            .http
            .get(GITHUB_OIDC_JWKS_URL)
            .send()
            .await?
            .error_for_status()?
            .json::<JwkSet>()
            .await?;
        let keys = Arc::new(keys);

        *self.jwks.lock().expect("jwks cache lock poisoned") = Some(CachedJwks {
            keys: Arc::clone(&keys),
            fetched_at: Instant::now(),
        });

        Ok(keys)
    }
}
