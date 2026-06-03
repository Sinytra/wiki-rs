use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use garde::Validate;
use serde::Deserialize;

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct Config {
    #[garde(dive)]
    pub server: ServerConfig,
    #[garde(dive)]
    pub database: DatabaseConfig,
    #[garde(dive)]
    pub redis: RedisConfig,
    #[garde(dive)]
    pub storage: StorageConfig,
    #[garde(dive)]
    pub auth: AuthConfig,
    #[garde(dive)]
    pub sentry: SentryConfig,
    #[garde(dive)]
    pub logging: LoggingConfig,
    #[garde(dive)]
    pub github: GithubConfig,
    #[garde(dive)]
    pub modrinth: ModrinthConfig,
    #[garde(dive)]
    pub crowdin: CrowdinConfig,
    #[garde(dive)]
    pub curseforge: CurseForgeConfig,
    #[garde(dive)]
    #[serde(default)]
    pub discord: DiscordConfig,
    #[garde(dive)]
    #[serde(default)]
    pub search: Option<SearchConfig>,
    #[garde(skip)]
    pub app_url: String,
    #[garde(skip)]
    #[serde(default)]
    pub local: bool,
    #[garde(skip)]
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct ServerConfig {
    #[garde(skip)]
    #[serde(default = "default_host")]
    pub host: String,
    #[garde(range(min = 1, max = 65535))]
    #[serde(default = "default_port")]
    pub port: u16,
    #[garde(skip)]
    #[serde(default)]
    pub allow_origins: Vec<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> u16 {
    8080
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct DatabaseConfig {
    #[garde(length(min = 1))]
    pub url: String,
    #[garde(range(min = 1, max = 100))]
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[garde(range(min = 1))]
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout_secs: u64,
}

fn default_max_connections() -> u32 {
    10
}

fn default_acquire_timeout() -> u64 {
    5
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct RedisConfig {
    #[garde(length(min = 1))]
    pub url: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct StorageConfig {
    #[garde(length(min = 1))]
    pub path: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct AuthConfig {
    #[garde(url)]
    pub frontend_url: String,
    #[garde(url)]
    pub callback_url: String,
    #[garde(url)]
    pub settings_callback_url: String,
    #[garde(url)]
    pub error_callback_url: String,
    #[garde(skip)]
    #[serde(default)]
    pub frontend_api_key: String,
    #[garde(skip)]
    #[serde(default = "default_session_cookie")]
    pub session_cookie_name: String,
}

fn default_session_cookie() -> String {
    "sessionid".into()
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct SentryConfig {
    #[garde(skip)]
    pub environment: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub dsn: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct LoggingConfig {
    #[garde(skip)]
    #[serde(default = "default_log_path")]
    pub path: String,
    #[garde(skip)]
    #[serde(default = "default_log_filter")]
    pub filter: String,
    #[garde(range(min = 1))]
    #[serde(default = "default_log_max_files")]
    pub max_files: u32,
}

fn default_log_path() -> String {
    "./logs".to_owned()
}

fn default_log_filter() -> String {
    "info,tower_http=debug".to_owned()
}

fn default_log_max_files() -> u32 {
    14
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct GithubConfig {
    #[garde(length(min = 1))]
    pub client_id: String,
    #[garde(length(min = 1))]
    pub client_secret: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct ModrinthConfig {
    #[garde(length(min = 1))]
    pub client_id: String,
    #[garde(length(min = 1))]
    pub client_secret: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct CrowdinConfig {
    #[garde(length(min = 1))]
    pub token: String,
    #[garde(length(min = 1))]
    pub project_id: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct CurseForgeConfig {
    #[garde(length(min = 1))]
    pub api_key: String,
}

#[derive(Debug, Deserialize, Validate, Clone)]
pub struct SearchConfig {
    #[garde(url)]
    pub url: String,
    #[garde(length(min = 1))]
    pub api_key: String,
    #[garde(length(min = 1))]
    pub collection: String,
}

#[derive(Debug, Default, Deserialize, Validate, Clone)]
pub struct DiscordConfig {
    #[garde(inner(url))]
    #[serde(default)]
    pub webhook_url: Option<String>,
}

pub fn load() -> anyhow::Result<Config> {
    let config: Config = Figment::new()
        .merge(Toml::file("config.toml"))
        .merge(Env::prefixed("WIKI_").split("__"))
        .extract()?;
    config.validate()?;
    Ok(config)
}
