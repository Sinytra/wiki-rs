pub mod crowdin;
pub mod curseforge;
pub mod discord;
pub mod error;
pub mod frontend;
pub mod github;
pub mod modrinth;
pub mod platforms;

pub const USER_AGENT: &str = "Sinytra/modded-wiki-rs/1.0.0";

pub use error::{ExternalError, ExternalResult};
