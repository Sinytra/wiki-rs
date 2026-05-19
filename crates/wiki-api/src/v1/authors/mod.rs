use serde::Deserialize;

pub mod public;
pub mod lifecycle;
pub mod manage;
pub mod content;

#[derive(Debug, Deserialize)]
pub struct ContentParams {
    pub version: Option<String>,
    pub locale: Option<String>,
    pub query: Option<String>,
    pub page: Option<u64>,
}
