use crate::error::StorageResult;
use crate::format::reader::{RawPage, RuntimeReadError};
use crate::format::shared::ProjectFormatInternal;
use crate::format::{ProjectFormat, WIKI_META_FILE, read_frontmatter_at, read_title_at};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_db::repo::ProjectRepo;
use wiki_domain::metadata::ProjectMetadata;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

const ASSETS_DIR: &str = ".assets";
const DATA_DIR: &str = ".data";
const RECIPES_DIR: &str = "recipe";
const RECIPE_TYPES_DIR: &str = "recipe_type";
const ASSETS_LANG_DIR: &str = "lang";
const I18N_DIR: &str = ".translated";
const CONTENT_DIR: &str = ".content";
const PROPERTIES_FILE: &str = ".data/properties.json";
const WORKBENCHES_FILE: &str = "workbenches.json";

#[derive(Debug, Clone)]
pub struct LegacyProjectFormat {
    root: PathBuf,
    locale: Option<String>,
    data_root_override: Option<PathBuf>,
}

impl LegacyProjectFormat {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            locale: None,
            data_root_override: None,
        }
    }

    pub fn with_locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale.filter(|s| !s.is_empty());
        self
    }

    pub fn with_data_root(mut self, data_root: PathBuf) -> Self {
        self.data_root_override = Some(data_root);
        self
    }
}

#[async_trait::async_trait]
impl ProjectFormat for LegacyProjectFormat {
    fn clone_with_root(&self, root: PathBuf) -> Arc<dyn ProjectFormat> {
        Arc::new(Self {
            root,
            locale: self.locale.clone(),
            data_root_override: None,
        })
    }

    // Layout

    fn root(&self) -> &Path {
        &self.root
    }

    fn assets_root(&self) -> PathBuf {
        self.root.join(ASSETS_DIR)
    }

    fn data_root(&self) -> PathBuf {
        self.data_root_override
            .clone()
            .unwrap_or_else(|| self.root.join(DATA_DIR))
    }

    fn recipes_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(modid).join(RECIPES_DIR)
    }

    fn recipe_types_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(modid).join(RECIPE_TYPES_DIR)
    }

    fn contents_root(&self) -> PathBuf {
        self.root.join(CONTENT_DIR)
    }

    fn workbenches_path(&self) -> PathBuf {
        self.data_root().join(WORKBENCHES_FILE)
    }

    fn wiki_metadata_path(&self) -> PathBuf {
        self.root.join(WIKI_META_FILE)
    }

    fn translated_root(&self) -> PathBuf {
        self.root.join(I18N_DIR)
    }

    fn item_properties_path(&self) -> PathBuf {
        self.localized_file_path(PROPERTIES_FILE)
    }

    fn language_file_path(&self, namespace: &str, locale: &str) -> PathBuf {
        super::shared::language_file_path(
            &self.root,
            ASSETS_DIR,
            ASSETS_LANG_DIR,
            namespace,
            locale,
        )
    }

    // Paths

    fn docs_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&super::shared::append_doc_ext(slug))
    }

    fn content_page_path(&self, slug: &str) -> PathBuf {
        let prefixed = format!("{}/{}", CONTENT_DIR, slug);
        self.localized_file_path(&super::shared::append_doc_ext(&prefixed))
    }

    // File access

    async fn read_metadata_async(&self) -> StorageResult<ProjectMetadata> {
        super::shared::read_metadata_async(self.clone()).await
    }

    fn read_metadata(&self) -> StorageResult<ProjectMetadata> {
        super::shared::read_metadata(&self.wiki_metadata_path())
    }

    fn read_page(&self, path: &Path) -> Result<RawPage, RuntimeReadError> {
        super::shared::read_page_at(path)
    }

    fn try_read_frontmatter_at(&self, path: &Path) -> Option<Frontmatter> {
        read_frontmatter_at(path)
    }

    fn read_page_title(&self, path: &Path) -> Option<String> {
        read_title_at(path)
    }

    fn docs_tree(&self) -> FileTree {
        self.build_directory_tree(self.root())
    }

    async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        self.build_content_tree(repo).await
    }
}

impl ProjectFormatInternal for LegacyProjectFormat {
    fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
}
