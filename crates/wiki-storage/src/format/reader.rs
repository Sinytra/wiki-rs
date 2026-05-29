use std::collections::BTreeMap;

use markdown::mdast::Node;
use serde::Deserialize;
use wiki_domain::error::{DomainError, ProjectError};
use wiki_domain::pages::metadata::Frontmatter;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeReadError {
    #[error("not found")]
    NotFound,
    #[error("io error")]
    Io,
    #[error("malformed markdown")]
    MalformedMarkdown,
    #[error("malformed frontmatter")]
    MalformedFrontmatter,
}

impl From<RuntimeReadError> for DomainError {
    fn from(e: RuntimeReadError) -> Self {
        match e {
            RuntimeReadError::NotFound => DomainError::NotFound,
            RuntimeReadError::Io => DomainError::Project {
                error: ProjectError::NoPath,
                message: String::new(),
            },
            RuntimeReadError::MalformedMarkdown => DomainError::Project {
                error: ProjectError::InvalidFormat,
                message: String::new(),
            },
            RuntimeReadError::MalformedFrontmatter => DomainError::Project {
                error: ProjectError::InvalidFrontmatter,
                message: String::new(),
            },
        }
    }
}

pub struct RawPage {
    pub content: String,
    pub tree: Node,
    pub frontmatter: Frontmatter,
}

#[derive(Debug, Default)]
pub struct FolderMetadata {
    pub keys: Vec<String>,
    pub entries: BTreeMap<String, FolderMetadataEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct FolderMetadataEntry {
    pub name: String,
    pub icon: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RawMetaValue {
    String(String),
    Object {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        icon: Option<serde_json::Value>,
    },
}


