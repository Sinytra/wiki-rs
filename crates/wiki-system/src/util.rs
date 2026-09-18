use crate::{SystemError, SystemResult};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

pub async fn merge_json_file(path: &Path, downloaded: &[u8]) -> SystemResult<()> {
    let existing = match tokio::fs::read(path).await {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warn!("failed to read lang file {}: {e}", path.display());
            None
        }
    };

    let bytes = merge_json_bytes(path, existing.as_deref(), downloaded);

    tokio::fs::write(path, &bytes)
        .await
        .map_err(|e| SystemError::Internal(format!("failed to write {}: {e}", path.display())))
}

pub fn merge_json_bytes<'a>(
    path: &Path,
    existing: Option<&[u8]>,
    incoming: &'a [u8],
) -> Cow<'a, [u8]> {
    let Some(existing) = existing else {
        return Cow::Borrowed(incoming);
    };

    match merge_json(existing, incoming) {
        Ok(merged) => Cow::Owned(merged),
        Err(e) => {
            warn!("failed to merge lang file {}: {e}", path.display());
            Cow::Borrowed(incoming)
        }
    }
}

fn merge_json(old: &[u8], new: &[u8]) -> serde_json::Result<Vec<u8>> {
    let mut merged: HashMap<String, String> = serde_json::from_slice(old)?;
    let incoming: HashMap<String, String> = serde_json::from_slice(new)?;
    merged.extend(incoming);

    serde_json::to_vec(&merged)
}

pub fn clean_dir_filtered(root: &Path, exclude: &[&str]) -> std::io::Result<()> {
    let keep: Vec<PathBuf> = exclude.iter().map(PathBuf::from).collect();
    clean_dir_inner(root, Path::new(""), &keep)
}

pub fn clean_dir_inner(dir: &Path, rel: &Path, keep: &[PathBuf]) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let entry_rel = rel.join(entry.file_name());

        if keep.contains(&entry_rel) {
            continue;
        }

        let is_dir = entry.file_type()?.is_dir();

        if is_dir && keep.iter().any(|k| k.starts_with(&entry_rel)) {
            clean_dir_inner(&entry.path(), &entry_rel, keep)?;
        } else if is_dir {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}
