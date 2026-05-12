use std::path::{Path, PathBuf};

pub const DOCS_FILE_EXT: &str = "mdx";
pub const JSON_EXT: &str = "json";
pub const WIKI_META_FILE: &str = "sinytra-wiki.json";

const DATA_DIR: &str = ".data";
const CONTENT_DIR: &str = ".content";
const WORKBENCHES_FILE: &str = ".data/workbenches.json";

#[derive(Debug, Clone)]
pub struct ProjectFormat {
    root: PathBuf,
}

impl ProjectFormat {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn data_root(&self) -> PathBuf {
        self.root.join(DATA_DIR)
    }

    pub fn content_dir(&self) -> PathBuf {
        self.root.join(CONTENT_DIR)
    }

    pub fn workbenches_path(&self) -> PathBuf {
        self.root.join(WORKBENCHES_FILE)
    }

    pub fn wiki_metadata_path(&self) -> PathBuf {
        self.root.join(WIKI_META_FILE)
    }
}

pub fn is_subpath(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}
