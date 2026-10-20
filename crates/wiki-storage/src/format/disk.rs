use crate::error::StorageResult;
use crate::format::ProjectFormat;
use crate::format::reader::{RawPage, RuntimeReadError};
use crate::format::shared::ProjectFormatInternal;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_content::content::{ContentScan, scan_content_pages};
use wiki_content::fs::{ContentFs, DirEntry};
use wiki_content::layout::{Layout, Section, WIKI_META_FILE};
use wiki_content::links::PageLink;
use wiki_content::resolve::{LinkData, ProjectView};
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::metadata::{ProjectMetadata, ProjectSchemaVersion};
use wiki_domain::pages::links::ResolvedLink;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

#[derive(Debug, Clone)]
pub struct DiskFormat {
    layout: Layout,
    root: PathBuf,
    locale: Option<String>,
    data_root_override: Option<PathBuf>,
}

impl DiskFormat {
    pub fn new(schema: ProjectSchemaVersion, root: PathBuf) -> Self {
        Self {
            layout: Layout::new(schema),
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

    fn path(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.root.clone()
        } else {
            self.root.join(rel)
        }
    }

    fn view<'a>(&'a self, fs: &'a DiskFs<'a>, lang: &'a str) -> ProjectView<'a, DiskFs<'a>> {
        ProjectView {
            fs,
            layout: self.layout,
            locale: self.locale.as_deref(),
            lang,
        }
    }
}

#[async_trait::async_trait]
impl ProjectFormat for DiskFormat {
    fn clone_with_root(&self, root: PathBuf) -> Arc<dyn ProjectFormat> {
        Arc::new(Self {
            layout: self.layout,
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
        self.path(self.layout.assets_dir())
    }

    fn data_root(&self) -> PathBuf {
        self.data_root_override
            .clone()
            .unwrap_or_else(|| self.path(self.layout.data_dir()))
    }

    fn recipes_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(self.layout.data_recipes_path(modid))
    }

    fn recipe_types_root(&self, modid: &str) -> PathBuf {
        self.data_root()
            .join(self.layout.data_recipe_types_path(modid))
    }

    fn contents_root(&self) -> PathBuf {
        self.path(self.layout.section_dir(Section::Content))
    }

    fn workbenches_path(&self, modid: &str) -> PathBuf {
        self.data_root()
            .join(self.layout.data_workbenches_path(modid))
    }

    fn wiki_metadata_path(&self) -> PathBuf {
        self.path(WIKI_META_FILE)
    }

    fn translated_root(&self) -> PathBuf {
        self.path(self.layout.translations_dir())
    }

    fn item_properties_path(&self, modid: &str) -> PathBuf {
        self.localized_file_path(&self.layout.item_properties_path(modid))
    }

    fn language_file_path(&self, namespace: &str, locale: &str) -> PathBuf {
        self.path(&self.layout.language_file_path(namespace, locale))
    }

    fn is_content_page(&self, path: &Path) -> bool {
        ProjectFormatInternal::is_content_page(self, path)
    }

    // Paths

    fn docs_index_page_path(&self) -> PathBuf {
        self.localized_file_path(&self.layout.docs_index_page_path())
    }

    fn docs_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&self.layout.page_path(Section::Docs, slug))
    }

    fn content_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&self.layout.page_path(Section::Content, slug))
    }

    fn item_asset_id(&self, location: &str) -> String {
        self.layout.item_asset_id(location)
    }

    fn item_asset_from(&self, item_id: &ResourceLocation) -> ResourceLocation {
        self.layout.item_asset_location(item_id)
    }

    fn asset_paths(&self, location: &ResourceLocation) -> Vec<PathBuf> {
        self.layout
            .asset_paths(location)
            .iter()
            .map(|p| self.path(p))
            .collect()
    }

    fn scan_content_pages(&self, modid: &str) -> ContentScan {
        scan_content_pages(&DiskFs::new(&self.root), self.layout, modid)
    }

    fn lang_key(&self, namespace: &str, lang: &str, key: &str) -> Option<String> {
        self.view(&DiskFs::new(&self.root), lang)
            .lang_key(namespace, key)
    }

    fn item_name(&self, id: &str, lang: &str, page_slug: Option<&str>) -> Option<String> {
        self.view(&DiskFs::new(&self.root), lang)
            .item_name(id, page_slug)
    }

    fn resolve_links(
        &self,
        links: &[PageLink],
        modid: Option<&str>,
        lang: &str,
        data: &LinkData,
    ) -> HashMap<String, ResolvedLink> {
        self.view(&DiskFs::new(&self.root), lang)
            .resolve_links(links, modid, data)
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
        super::shared::try_read_frontmatter(path)
    }

    // Trees

    fn docs_tree(&self) -> FileTree {
        self.build_directory_tree(Section::Docs)
    }

    async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        self.build_content_tree(repo).await
    }
}

impl ProjectFormatInternal for DiskFormat {
    fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    fn content_dir_name(&self) -> &str {
        self.layout.section_dir(Section::Content)
    }
}

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
