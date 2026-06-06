use crate::error::{StorageError, StorageResult};
use crate::format::reader::{RawPage, RuntimeReadError};
use crate::format::v1_format::V1ProjectFormat;
use crate::ingestor::markdown::{read_first_h1, read_frontmatter};
use crate::ingestor::try_parse_json_path;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::{ProjectMetadata, ProjectMetadataStub, ProjectSchemaVersion};
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

mod legacy;
mod reader;
mod shared;
pub mod v1_format;

pub use legacy::LegacyProjectFormat;

pub const DOCS_FILE_EXT: &str = "mdx";
pub const JSON_EXT: &str = "json";
pub const WIKI_META_FILE: &str = "sinytra-wiki.json";
pub const FOLDER_META_FILE: &str = "_meta.json";

const NO_ICON: &str = "_none";
const DOCS_FILE_DOT_EXT: &str = ".mdx";

pub fn create_project_format(
    root: PathBuf,
    locale: Option<String>,
) -> StorageResult<Arc<dyn ProjectFormat>> {
    let meta_path = root.join(WIKI_META_FILE);

    let meta: ProjectMetadataStub = try_parse_json_path("project metadata", &meta_path)
        .map_err(StorageError::to_invalid_meta)?
        .value;

    let format: Arc<dyn ProjectFormat> = match meta.schema {
        ProjectSchemaVersion::Legacy => {
            Arc::new(LegacyProjectFormat::new(root).with_locale(locale))
        }
        ProjectSchemaVersion::V1 => Arc::new(V1ProjectFormat::new(root).with_locale(locale)),
    };

    Ok(format)
}

#[async_trait::async_trait]
pub trait ProjectFormat: Send + Sync {
    fn clone_with_root(&self, root: PathBuf) -> Arc<dyn ProjectFormat>;

    // Layout
    fn root(&self) -> &Path;
    fn assets_root(&self) -> PathBuf;
    fn data_root(&self) -> PathBuf;
    fn recipes_root(&self, modid: &str) -> PathBuf;
    fn recipe_types_root(&self, modid: &str) -> PathBuf;
    fn contents_root(&self) -> PathBuf;
    fn workbenches_path(&self, modid: &str) -> PathBuf;
    fn wiki_metadata_path(&self) -> PathBuf;
    fn translated_root(&self) -> PathBuf;
    fn item_properties_path(&self, modid: &str) -> PathBuf;
    fn language_file_path(&self, namespace: &str, locale: &str) -> PathBuf;

    // Paths
    fn docs_index_page_path(&self) -> PathBuf;
    fn docs_page_path(&self, slug: &str) -> PathBuf;
    fn content_page_path(&self, slug: &str) -> PathBuf;
    /// Returns the inventory icon asset for an item
    fn item_asset_from(&self, item_id: &ResourceLocation) -> ResourceLocation;

    // File access
    async fn read_metadata_async(&self) -> StorageResult<ProjectMetadata>;
    fn read_metadata(&self) -> StorageResult<ProjectMetadata>;
    fn read_page(&self, path: &Path) -> Result<RawPage, RuntimeReadError>;
    fn try_read_frontmatter_at(&self, path: &Path) -> Option<Frontmatter>;
    fn read_page_title(&self, path: &Path) -> Option<String>;

    // Trees
    fn docs_tree(&self) -> FileTree;
    async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree>;

    // Defaults
    fn item_asset_id(&self, location: &str) -> String {
        ResourceLocation::parse(location)
            .map(|loc| self.item_asset_from(&loc).to_string())
            .unwrap_or_else(|| location.to_owned())
    }

    fn asset_path(&self, location: &ResourceLocation) -> PathBuf {
        let ext = if location.path.contains('.') {
            ""
        } else {
            ".png"
        };
        self.assets_root()
            .join(&location.namespace)
            .join(format!("{}{ext}", location.path))
    }

    fn content_slug(&self, path: &Path) -> String {
        self.slug_from_path(&self.contents_root(), path)
    }

    fn slug_from_path(&self, root: &Path, path: &Path) -> String {
        let str = path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy();
        shared::strip_doc_ext(&str).to_string()
    }

    fn rel_path_with_ext(&self, path: &Path) -> String {
        path.strip_prefix(self.root())
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }

    fn read_page_title_at(&self, frontmatter: &Frontmatter, path: &Path) -> Option<String> {
        if let Some(ref title) = frontmatter.title {
            return Some(title.clone());
        }
        read_first_h1(path)
    }

    fn validate_file(&self, path: &Path, ext: &str) -> StorageResult<()> {
        match ext {
            // Markdown: validate frontmatter only
            ".mdx" => {
                if let Err(e) = read_frontmatter(path) {
                    return Err(StorageError::project(
                        ProjectError::InvalidFrontmatter,
                        e.to_string(),
                    ));
                }
            }
            // JSON: verify file is valid json
            ".json" => {
                try_parse_json_path::<serde_json::Value>("file", path)?;
            }
            _ => {}
        };

        Ok(())
    }
}

