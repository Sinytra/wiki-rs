pub mod crowdin;
pub mod curseforge;
pub mod error;
pub mod frontend;
pub mod modrinth;
pub mod platforms;
pub mod github;

pub const USER_AGENT: &str = "Sinytra/modded-wiki-rs/1.0.0";

pub use error::{ExternalError, ExternalResult};
