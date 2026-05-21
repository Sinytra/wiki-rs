use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};
use crate::error::{StorageError, StorageResult};
use crate::format::ProjectFormat;
use crate::ingestor::issues::{IssueSink, ProjectIssue};

const ALLOWED_EXTENSIONS: &[&str] = &[".mdx", ".json", ".png", ".jpg", ".jpeg", ".webp", ".gif"];

const MIME_TYPE_TEXT: &str = "text/plain";

pub fn is_valid_mime_type_for(ext: &str, mime_type: &str) -> bool {
    let valid_types = match ext {
        ".mdx" | ".json" => &[MIME_TYPE_TEXT],
        ".png" => &["image/png"],
        ".jpeg" | ".jpg" => &["image/jpeg"],
        ".webp" => &["image/webp"],
        ".gif" => &["image/gif"],
        _ => &[] as &'static [&'static str],
    };
    valid_types.contains(&mime_type)
}

pub struct FileCopier {
    allowed: HashSet<&'static str>,
    format: ProjectFormat,
    issues: Arc<dyn IssueSink>
}

impl FileCopier {
    pub fn new(format: ProjectFormat, issues: Arc<dyn IssueSink>) -> Self {
        Self {
            format,
            issues,
            allowed: ALLOWED_EXTENSIONS.iter().copied().collect()
        }
    }

    pub fn copy_project_files(&self, src: &Path, dest: &Path) -> StorageResult<()> {
        info!(dest = %dest.display(), "Copying project files");

        std::fs::create_dir_all(dest)?;

        self.copy_dir_recursive(src, src, dest)?;

        debug!("Done copying files");
        Ok(())
    }

    fn copy_dir_recursive(
        &self,
        root: &Path,
        current: &Path,
        dest_root: &Path,
    ) -> StorageResult<()> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                self.copy_dir_recursive(root, &entry.path(), dest_root)?;
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let maybe_ext = entry.path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{e}"));
            let Some(ext) = maybe_ext else {
                continue;
            };

            if !self.is_allowed_file(&entry.path(), &ext).unwrap_or(false) {
                warn!(file = %entry.path().display(), "Skipping non-allowed file");
                continue;
            }

            if let Err(err) = self.format.validate_file(&entry.path(), &ext) {
                let (error, message) = match err {
                    StorageError::Project { error, message } => (error, message),
                    _ => (ProjectError::InvalidFile, "Unexpected error validating file".to_owned())
                };
                self.issues.add(ProjectIssue {
                    level: ProjectIssueLevel::Error,
                    kind: ProjectIssueType::File,
                    subject: error,
                    details: Some(message),
                    file: Some(entry.path()),
                });
                continue;
            }

            let relative = entry.path().strip_prefix(root).unwrap().to_owned();
            let dest_path = dest_root.join(&relative);

            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::copy(entry.path(), &dest_path)?;
        }
        Ok(())
    }

    #[tracing::instrument(err, skip(self))]
    fn is_allowed_file(&self, path: &Path, ext: &str) -> StorageResult<bool> {
        if !self.allowed.contains(ext) {
            return Ok(false);
        }

        let mut file = File::open(path)?;

        let mut buf = [0u8; 8192];
        let n = file.read(&mut buf)?;
        let bytes = &buf[..n];

        let mime_type = match infer::get(bytes) {
            Some(kind) => Some(kind.mime_type()),
            None => {
                if std::str::from_utf8(bytes).is_ok() {
                    Some(MIME_TYPE_TEXT)
                } else {
                    None
                }
            }
        };

        Ok(match mime_type {
            Some(mime_type) => is_valid_mime_type_for(ext, mime_type),
            None => false,
        })
    }
}