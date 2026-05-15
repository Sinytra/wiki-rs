use std::path::{Path, PathBuf};

use wiki_domain::content::ResourceLocation;

pub const DOCS_FILE_EXT: &str = "mdx";
pub const JSON_EXT: &str = "json";
pub const WIKI_META_FILE: &str = "sinytra-wiki.json";

const ASSETS_DIR: &str = ".assets";
const DATA_DIR: &str = ".data";
const ASSETS_LANG_DIR: &str = "lang";
const I18N_DIR: &str = ".translated";
const CONTENT_DIR: &str = ".content";
const FOLDER_META_FILE: &str = "_meta.json";
const PROPERTIES_FILE: &str = ".data/properties.json";
const WORKBENCHES_FILE: &str = ".data/workbenches.json";

#[derive(Debug, Clone)]
pub struct ProjectFormat {
    root: PathBuf,
    locale: Option<String>,
}

impl ProjectFormat {
    pub fn new(root: PathBuf) -> Self {
        Self { root, locale: None }
    }

    pub fn with_locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale.filter(|s| !s.is_empty());
        self
    }

    pub fn set_locale(&mut self, locale: Option<String>) {
        self.locale = locale.filter(|s| !s.is_empty());
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets_root(&self) -> PathBuf {
        self.root.join(ASSETS_DIR)
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

    pub fn locales_path(&self) -> PathBuf {
        self.root.join(I18N_DIR)
    }

    pub fn item_properties_path(&self) -> PathBuf {
        self.localized_file_path(PROPERTIES_FILE)
    }

    pub fn assets_path(&self, location: &ResourceLocation) -> PathBuf {
        let ext = if location.path.contains('.') { "" } else { ".png" };
        self.root
            .join(ASSETS_DIR)
            .join(&location.namespace)
            .join(format!("{}{ext}", location.path))
    }

    pub fn language_file_path(&self, namespace: &str, locale: &str) -> PathBuf {
        self.root
            .join(ASSETS_DIR)
            .join(namespace)
            .join(ASSETS_LANG_DIR)
            .join(format!("{locale}.json"))
    }

    pub fn localized_file_path(&self, path: &str) -> PathBuf {
        let trimmed = path.trim_start_matches('/');
        if let Some(loc) = &self.locale {
            let candidate = self.root.join(I18N_DIR).join(loc).join(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root.join(trimmed)
    }

    pub fn folder_meta_file_path(&self, dir: &Path) -> PathBuf {
        if let Some(loc) = &self.locale
            && let Ok(rel) = dir.strip_prefix(&self.root)
        {
            let candidate = self.root.join(I18N_DIR).join(loc).join(rel).join(FOLDER_META_FILE);
            if candidate.exists() {
                return candidate;
            }
        }
        dir.join(FOLDER_META_FILE)
    }
}

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
