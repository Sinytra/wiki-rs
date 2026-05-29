use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BuiltinProjectFormat {
    root: PathBuf,
}

impl BuiltinProjectFormat {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets_root(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn data_root(&self) -> PathBuf {
        self.root.join("data")
    }
}

pub fn is_subpath(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}