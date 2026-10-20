use markdown::mdast::Node;
use wiki_domain::error::{DomainError, ProjectError};
use wiki_domain::pages::metadata::Frontmatter;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeReadError {
    #[error("not found")]
    NotFound,
    #[error("io error")]
    Io,
    #[error("malformed markdown")]
    MalformedMarkdown { message: String },
    #[error("malformed frontmatter")]
    MalformedFrontmatter { message: String },
}

impl From<RuntimeReadError> for DomainError {
    fn from(e: RuntimeReadError) -> Self {
        match e {
            RuntimeReadError::NotFound => DomainError::NotFound,
            RuntimeReadError::Io => DomainError::Project {
                error: ProjectError::NoPath,
                message: String::new(),
            },
            RuntimeReadError::MalformedMarkdown { message } => DomainError::Project {
                error: ProjectError::InvalidFormat,
                message,
            },
            RuntimeReadError::MalformedFrontmatter { message } => DomainError::Project {
                error: ProjectError::InvalidFrontmatter,
                message,
            },
        }
    }
}

pub struct RawPage {
    pub content: String,
    pub tree: Node,
    pub frontmatter: Frontmatter,
}
