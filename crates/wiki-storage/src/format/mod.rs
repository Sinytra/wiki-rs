use crate::error::{StorageError, StorageResult};
use crate::format::reader::RawPage;
use crate::ingestor::markdown::{read_first_h1, read_frontmatter};
use crate::ingestor::try_parse_json_path;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_content::content::ContentScan;
use wiki_content::links::PageLink;
use wiki_content::resolve::LinkData;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::{ProjectMetadata, ProjectMetadataStub};
use wiki_domain::pages::links::ResolvedLink;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

mod disk;
mod reader;
mod shared;

pub use disk::DiskFormat;
pub use reader::RuntimeReadError;
pub use wiki_content::layout::{DOCS_FILE_EXT, FOLDER_META_FILE, WIKI_META_FILE};


pub const JSON_EXT: &str = "json";

pub fn create_project_format(
    root: PathBuf,
    locale: Option<String>,
    relative_root: Option<&Path>,
) -> StorageResult<Arc<dyn ProjectFormat>> {
    let meta_path = root.join(WIKI_META_FILE);
    if !meta_path.exists() {
        let rel_path = match relative_root {
            Some(path) => meta_path.strip_prefix(path).unwrap_or(&meta_path),
            _ => Path::new(WIKI_META_FILE),
        };

        return Err(StorageError::project(
            ProjectError::NoPath,
            format!("Metadata file '{}' missing", rel_path.display()),
        ));
    }

    let meta: ProjectMetadataStub = try_parse_json_path("project metadata", &meta_path)
        .map_err(StorageError::to_invalid_meta)?
        .value;

    let format: Arc<dyn ProjectFormat> =
        Arc::new(DiskFormat::new(meta.schema, root).with_locale(locale));

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
    fn item_asset_from(&self, item_id: &ResourceLocation) -> ResourceLocation;
    fn item_asset_id(&self, location: &str) -> String;
    fn is_content_page(&self, path: &Path) -> bool;

    fn asset_paths(&self, location: &ResourceLocation) -> Vec<PathBuf>;
    fn scan_content_pages(&self, modid: &str) -> ContentScan;
    fn lang_key(&self, namespace: &str, lang: &str, key: &str) -> Option<String>;
    fn item_name(&self, id: &str, lang: &str, page_slug: Option<&str>) -> Option<String>;
    fn resolve_links(
        &self,
        links: &[PageLink],
        modid: Option<&str>,
        lang: &str,
        data: &LinkData,
    ) -> HashMap<String, ResolvedLink>;

    // File access
    async fn read_metadata_async(&self) -> StorageResult<ProjectMetadata>;
    fn read_metadata(&self) -> StorageResult<ProjectMetadata>;
    fn read_page(&self, path: &Path) -> Result<RawPage, RuntimeReadError>;
    fn try_read_frontmatter_at(&self, path: &Path) -> Option<Frontmatter>;

    // Trees
    fn docs_tree(&self) -> FileTree;
    async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree>;

    // Defaults

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
            ".mdx" => match read_frontmatter(path) {
                Err(e) => {
                    return Err(StorageError::project(
                        ProjectError::InvalidFrontmatter,
                        e.to_string(),
                    ));
                }
                Ok(fm) => {
                    if self.is_content_page(path) && fm.id.is_empty() {
                        return Err(StorageError::project(
                            ProjectError::InvalidFrontmatter,
                            "Missing required property 'id'",
                        ));
                    }
                }
            },
            // JSON: verify file is valid json
            ".json" => {
                try_parse_json_path::<serde_json::Value>("file", path)?;
            }
            _ => {}
        };

        Ok(())
    }
}
