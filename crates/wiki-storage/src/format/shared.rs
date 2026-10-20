use crate::error::{StorageError, StorageResult};
use crate::format::ProjectFormat;
use crate::format::disk::DiskFs;
use crate::format::reader::{RawPage, RuntimeReadError};
use crate::ingestor::markdown::{parse_frontmatter, parse_mdast, read_first_h1, read_frontmatter};

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use tracing::warn;
use wiki_content::content::{content_tree, tree_file_paths};
use wiki_content::layout::{Layout, Section};
use wiki_content::metadata::{ProjectMetadataError, parse_project_metadata};
use wiki_content::tree::build_tree;
use wiki_db::repo::ProjectRepo;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::ProjectMetadata;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

pub trait ProjectFormatInternal: ProjectFormat {
    fn locale(&self) -> Option<&str>;

    fn layout(&self) -> Layout;

    fn content_dir_name(&self) -> &str;

    fn is_content_page(&self, path: &Path) -> bool {
        if path.starts_with(self.contents_root()) {
            return true;
        }

        let Ok(rel) = path.strip_prefix(self.translated_root()) else {
            return false;
        };

        let mut comps = rel.components();

        if !matches!(comps.next(), Some(Component::Normal(_))) {
            return false;
        }

        if !matches!(comps.next(), Some(Component::Normal(s)) if s == self.content_dir_name()) {
            return false;
        }

        !comps.as_path().as_os_str().is_empty()
    }

    fn localized_file_path(&self, path: &str) -> PathBuf {
        let trimmed = path.trim_start_matches('/');
        if let Some(loc) = self.locale() {
            let candidate = self.translated_root().join(loc).join(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root().join(trimmed)
    }

    fn build_directory_tree(&self, section: Section) -> FileTree {
        let result = build_tree(
            &DiskFs::new(self.root()),
            self.layout(),
            section,
            self.locale(),
        );
        for issue in &result.issues {
            warn!(path = %issue.path, "{}", issue.message);
        }
        result.tree
    }

    async fn build_content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        let tree = self.build_directory_tree(Section::Content);
        let refs = repo.get_page_refs(&tree_file_paths(&tree)).await?;
        Ok(content_tree(
            &DiskFs::new(self.root()),
            self.layout(),
            self.locale(),
            tree,
            &refs,
        ))
    }
}

pub async fn read_metadata_async<F>(format: F) -> StorageResult<ProjectMetadata>
where
    F: ProjectFormat + Send + Clone + 'static,
{
    tokio::task::spawn_blocking({
        let format = format.clone();
        move || format.read_metadata()
    })
    .await
    .map_err(|e| StorageError::Internal(e.to_string()))?
}

pub fn read_metadata(path: &Path) -> StorageResult<ProjectMetadata> {
    if !path.exists() {
        return Err(StorageError::project(
            ProjectError::NoPath,
            format!("Metadata file '{}' missing", path.display()),
        ));
    }

    let text = fs::read_to_string(path)
        .map_err(|e| StorageError::project(ProjectError::InvalidMeta, e.to_string()))?;
    let metadata = parse_project_metadata(&text).map_err(|e| match e {
        ProjectMetadataError::Json(e) => {
            StorageError::project(ProjectError::InvalidMeta, e.to_string())
        }
        ProjectMetadataError::Validate(_) => StorageError::project(
            ProjectError::InvalidMeta,
            format!("Failed to parse metadata file: {e}"),
        ),
    })?;

    Ok(metadata)
}

#[tracing::instrument(name = "Reading page at path")]
pub fn read_page_at(path: &Path) -> Result<RawPage, RuntimeReadError> {
    let content = fs::read_to_string(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => RuntimeReadError::NotFound,
        _ => RuntimeReadError::Io,
    })?;
    let tree = parse_mdast(&content).map_err(|e| RuntimeReadError::MalformedMarkdown {
        message: e.to_string(),
    })?;
    let frontmatter =
        parse_frontmatter(&tree).map_err(|e| RuntimeReadError::MalformedFrontmatter {
            message: e.to_string(),
        })?;
    Ok(RawPage {
        content,
        tree,
        frontmatter,
    })
}

pub fn try_read_frontmatter(path: &Path) -> Option<Frontmatter> {
    match read_frontmatter(path) {
        Ok(fm) => Some(fm),
        Err(e) => {
            warn!(path = %path.display(), "failed to read frontmatter: {e}");
            None
        }
    }
}
