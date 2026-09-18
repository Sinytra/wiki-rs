use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::error::{StorageError, StorageResult};

pub struct TempArchive {
    path: PathBuf,
}

impl TempArchive {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn open(&self) -> StorageResult<tokio::fs::File> {
        Ok(tokio::fs::File::open(&self.path).await?)
    }

    pub async fn size(&self) -> StorageResult<u64> {
        Ok(tokio::fs::metadata(&self.path).await?.len())
    }
}

impl Drop for TempArchive {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::warn!(path = %self.path.display(), "failed to remove temp archive: {err}");
        }
    }
}

#[tracing::instrument(name = "Creating directory archive", skip_all, fields(source = %source.display()))]
pub async fn archive_directory(source: PathBuf, dest: PathBuf) -> StorageResult<TempArchive> {
    if !source.is_dir() {
        return Err(StorageError::Internal(format!(
            "cannot archive nonexistent directory {}",
            source.display()
        )));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let archive = TempArchive { path: dest };
    let target = archive.path.clone();

    tokio::task::spawn_blocking(move || write_archive(&source, &target))
        .await
        .map_err(|e| StorageError::TaskPanic(e.to_string()))??;

    Ok(archive)
}

fn write_archive(source: &Path, dest: &Path) -> StorageResult<()> {
    let file = File::create(dest)?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for entry in WalkDir::new(source).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(source) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }

        let name = relative.to_string_lossy().replace('\\', "/");
        let result = if entry.file_type().is_dir() {
            zip
                .add_directory(format!("{name}/"), options)
                .map(|_| ())
        } else if entry.file_type().is_file() {
            zip.start_file(name, options).and_then(|_| {
                let mut source_file = File::open(path)?;
                std::io::copy(&mut source_file, &mut zip)?;
                Ok(())
            })
        } else {
            continue;
        };

        result.map_err(|e| StorageError::Internal(format!("failed to archive entry: {e}")))?;
    }

    let mut inner = zip
        .finish()
        .map_err(|e| StorageError::Internal(format!("failed to finish archive: {e}")))?;
    inner.flush()?;

    Ok(())
}
