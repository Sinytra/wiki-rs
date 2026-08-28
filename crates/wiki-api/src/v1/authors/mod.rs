use serde::Deserialize;
use wiki_domain::project::ProjectOptions;

pub mod content;
pub mod lifecycle;
pub mod manage;
pub mod public;
pub mod realtime;

#[derive(Debug, Deserialize)]
pub struct ContentParams {
    #[serde(flatten)]
    pub options: ProjectOptions,
    pub query: Option<String>,
    pub page: Option<u64>,
}
