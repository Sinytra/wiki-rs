use crate::error::StorageResult;
use crate::format::reader::{RawPage, RuntimeReadError};
use crate::format::shared::ProjectFormatInternal;
use crate::format::{ProjectFormat, WIKI_META_FILE};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::metadata::ProjectMetadata;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

const ASSETS_DIR: &str = "assets";
const ASSETS_LANG_DIR: &str = "lang";

const DATA_DIR: &str = "data";
const RECIPES_DIR: &str = "recipe";
const RECIPE_TYPES_DIR: &str = "recipe_type";

const I18N_DIR: &str = "translated";
const DOCS_DIR: &str = "docs";
const CONTENT_DIR: &str = "content";

const PROPERTIES_FILE: &str = "properties/item.json";
const WORKBENCHES_FILE: &str = "workbenches.json";

const DOCS_INDEX_PAGE_SLUG: &str = "_index";

#[derive(Debug, Clone)]
pub struct V1ProjectFormat {
    root: PathBuf,
    locale: Option<String>,
}

impl V1ProjectFormat {
    pub fn new(root: PathBuf) -> Self {
        Self { root, locale: None }
    }

    pub fn with_locale(mut self, locale: Option<String>) -> Self {
        self.locale = locale.filter(|s| !s.is_empty());
        self
    }
}

#[async_trait::async_trait]
impl ProjectFormat for V1ProjectFormat {
    fn clone_with_root(&self, root: PathBuf) -> Arc<dyn ProjectFormat> {
        Arc::new(Self {
            root,
            locale: self.locale.clone(),
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
        self.root.join(DATA_DIR)
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

    fn workbenches_path(&self, modid: &str) -> PathBuf {
        self.data_root().join(modid).join(WORKBENCHES_FILE)
    }

    fn wiki_metadata_path(&self) -> PathBuf {
        self.root.join(WIKI_META_FILE)
    }

    fn translated_root(&self) -> PathBuf {
        self.root.join(I18N_DIR)
    }

    fn item_properties_path(&self, modid: &str) -> PathBuf {
        let rel_path = format!("{DATA_DIR}/{modid}/{PROPERTIES_FILE}");
        self.localized_file_path(&rel_path)
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

    fn docs_index_page_path(&self) -> PathBuf {
        self.docs_page_path(DOCS_INDEX_PAGE_SLUG)
    }

    fn docs_page_path(&self, slug: &str) -> PathBuf {
        let prefixed = format!("{DOCS_DIR}/{slug}");
        self.localized_file_path(&super::shared::append_doc_ext(&prefixed))
    }

    fn content_page_path(&self, slug: &str) -> PathBuf {
        let prefixed = format!("{CONTENT_DIR}/{slug}");
        self.localized_file_path(&super::shared::append_doc_ext(&prefixed))
    }

    fn item_asset_from(&self, item_id: &ResourceLocation) -> ResourceLocation {
        item_id.with_path_prefix("item/")
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
        super::shared::read_frontmatter_at(path)
    }

    fn read_page_title(&self, path: &Path) -> Option<String> {
        super::shared::read_title_at(path)
    }

    fn docs_tree(&self) -> FileTree {
        self.build_directory_tree(&self.docs_root())
    }

    async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        self.build_content_tree(repo).await
    }
}

impl ProjectFormatInternal for V1ProjectFormat {
    fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
}

impl V1ProjectFormat {
    fn docs_root(&self) -> PathBuf {
        self.root.join(DOCS_DIR)
    }
}
