use std::fs;
use std::path::{Path, PathBuf};
use wiki_content::fs::{ContentFs, DirEntry};

pub struct DiskFs<'a> {
    root: &'a Path,
}

impl<'a> DiskFs<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.to_path_buf()
        } else {
            self.root.join(rel)
        }
    }
}

impl ContentFs for DiskFs<'_> {
    fn read_to_string(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.path(path)).ok()
    }

    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let entries = fs::read_dir(self.path(path)).ok()?;
        Some(
            entries
                .flatten()
                .filter_map(|e| {
                    let file_type = e.file_type().ok()?;
                    (file_type.is_dir() || file_type.is_file()).then(|| DirEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        is_dir: file_type.is_dir(),
                    })
                })
                .collect(),
        )
    }

    fn exists(&self, path: &str) -> bool {
        self.path(path).exists()
    }
}
