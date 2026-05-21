use serde::Deserialize;

pub mod content;
pub mod lifecycle;
pub mod manage;
pub mod public;
pub mod realtime;

#[derive(Debug, Deserialize)]
pub struct ContentParams {
    pub version: Option<String>,
    pub locale: Option<String>,
    pub query: Option<String>,
    pub page: Option<u64>,
}
