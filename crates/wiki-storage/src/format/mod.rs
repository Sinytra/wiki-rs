use crate::error::{StorageError, StorageResult};
use crate::format::disk::DiskFs;
use crate::format::reader::RawPage;
use crate::ingestor::markdown::{parse_frontmatter, parse_mdast, read_first_h1, read_frontmatter};
use crate::ingestor::try_parse_json_path;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tracing::warn;
use wiki_content::content::{ContentScan, content_tree, scan_content_pages, tree_file_paths};
use wiki_content::layout::{Layout, Section};
use wiki_content::links::PageLink;
use wiki_content::metadata::{ProjectMetadataError, parse_project_metadata};
use wiki_content::resolve::{LinkData, ProjectView};
use wiki_content::tree::build_tree;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::{ProjectMetadata, ProjectMetadataStub, ProjectSchemaVersion};
use wiki_domain::pages::links::ResolvedLink;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{ContentFileTree, FileTree};

mod disk;
mod reader;

pub use reader::RuntimeReadError;
pub use wiki_content::layout::{DOCS_FILE_EXT, FOLDER_META_FILE, WIKI_META_FILE};

pub const JSON_EXT: &str = "json";

pub fn create_project_format(
    root: PathBuf,
    locale: Option<String>,
    relative_root: Option<&Path>,
) -> StorageResult<Arc<ProjectFormat>> {
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

    Ok(Arc::new(
        ProjectFormat::new(meta.schema, root).with_locale(locale),
    ))
}

#[derive(Debug, Clone)]
pub struct ProjectFormat {
    layout: Layout,
    root: PathBuf,
    locale: Option<String>,
    data_root_override: Option<PathBuf>,
}

impl ProjectFormat {
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

    pub fn clone_with_root(&self, root: PathBuf) -> Arc<ProjectFormat> {
        Arc::new(Self {
            layout: self.layout,
            root,
            locale: self.locale.clone(),
            data_root_override: None,
        })
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

    fn localized_file_path(&self, path: &str) -> PathBuf {
        let trimmed = path.trim_start_matches('/');
        if let Some(loc) = &self.locale {
            let candidate = self.translated_root().join(loc).join(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root.join(trimmed)
    }

    // Layout

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn data_root(&self) -> PathBuf {
        self.data_root_override
            .clone()
            .unwrap_or_else(|| self.path(self.layout.data_dir()))
    }

    pub fn recipes_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(self.layout.data_recipes_path(modid))
    }

    pub fn recipe_types_root(&self, modid: &str) -> PathBuf {
        self.data_root()
            .join(self.layout.data_recipe_types_path(modid))
    }

    pub fn contents_root(&self) -> PathBuf {
        self.path(self.layout.section_dir(Section::Content))
    }

    pub fn workbenches_path(&self, modid: &str) -> PathBuf {
        self.data_root()
            .join(self.layout.data_workbenches_path(modid))
    }

    pub fn wiki_metadata_path(&self) -> PathBuf {
        self.path(WIKI_META_FILE)
    }

    pub fn translated_root(&self) -> PathBuf {
        self.path(self.layout.translations_dir())
    }

    pub fn item_properties_path(&self, modid: &str) -> PathBuf {
        self.localized_file_path(&self.layout.item_properties_path(modid))
    }

    pub fn language_file_path(&self, namespace: &str, locale: &str) -> PathBuf {
        self.path(&self.layout.language_file_path(namespace, locale))
    }

    // Paths

    pub fn docs_index_page_path(&self) -> PathBuf {
        self.localized_file_path(&self.layout.docs_index_page_path())
    }

    pub fn docs_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&self.layout.page_path(Section::Docs, slug))
    }

    pub fn content_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&self.layout.page_path(Section::Content, slug))
    }

    pub fn item_asset_id(&self, location: &str) -> String {
        self.layout.item_asset_id(location)
    }

    pub fn item_asset_from(&self, item_id: &ResourceLocation) -> ResourceLocation {
        self.layout.item_asset_location(item_id)
    }

    pub fn asset_paths(&self, location: &ResourceLocation) -> Vec<PathBuf> {
        self.layout
            .asset_paths(location)
            .iter()
            .map(|p| self.path(p))
            .collect()
    }

    pub fn rel_path_with_ext(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }

    pub fn is_content_page(&self, path: &Path) -> bool {
        if path.starts_with(self.contents_root()) {
            return true;
        }

        let Ok(rel) = path.strip_prefix(self.translated_root()) else {
            return false;
        };

        let mut comps = rel.components();

        if !matches!(comps.next(), Some(Component::Normal(_))) {
            return false;
        }

        let content_dir = self.layout.section_dir(Section::Content);
        if !matches!(comps.next(), Some(Component::Normal(s)) if s == content_dir) {
            return false;
        }

        !comps.as_path().as_os_str().is_empty()
    }

    // Content

    pub fn scan_content_pages(&self, modid: &str) -> ContentScan {
        scan_content_pages(&DiskFs::new(&self.root), self.layout, modid)
    }

    pub fn lang_key(&self, namespace: &str, lang: &str, key: &str) -> Option<String> {
        self.view(&DiskFs::new(&self.root), lang)
            .lang_key(namespace, key)
    }

    pub fn item_name(&self, id: &str, lang: &str, page_slug: Option<&str>) -> Option<String> {
        self.view(&DiskFs::new(&self.root), lang)
            .item_name(id, page_slug)
    }

    pub fn resolve_links(
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

    pub async fn read_metadata_async(&self) -> StorageResult<ProjectMetadata> {
        let format = self.clone();
        tokio::task::spawn_blocking(move || format.read_metadata())
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?
    }

    pub fn read_metadata(&self) -> StorageResult<ProjectMetadata> {
        let path = self.wiki_metadata_path();
        if !path.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!("Metadata file '{}' missing", path.display()),
            ));
        }

        let text = fs::read_to_string(&path)
            .map_err(|e| StorageError::project(ProjectError::InvalidMeta, e.to_string()))?;
        let metadata = parse_project_metadata(&text).map_err(|e| match e {
            ProjectMetadataError::Json(e) => {
                StorageError::project(ProjectError::InvalidMeta, e.to_string())
            }
            ProjectMetadataError::Validate(_) => StorageError::project(
                ProjectError::InvalidMeta,
                format!("Failed to parse metadata file: {e}"),
            ),
        })?;

        Ok(metadata)
    }

    #[tracing::instrument(name = "Reading page at path", skip(self))]
    pub fn read_page(&self, path: &Path) -> Result<RawPage, RuntimeReadError> {
        let content = fs::read_to_string(path).map_err(|e| match e.kind() {
            ErrorKind::NotFound => RuntimeReadError::NotFound,
            _ => RuntimeReadError::Io,
        })?;
        let tree = parse_mdast(&content).map_err(|e| RuntimeReadError::MalformedMarkdown {
            message: e.to_string(),
        })?;
        let frontmatter =
            parse_frontmatter(&tree).map_err(|e| RuntimeReadError::MalformedFrontmatter {
                message: e.to_string(),
            })?;
        Ok(RawPage {
            content,
            tree,
            frontmatter,
        })
    }

    pub fn try_read_frontmatter_at(&self, path: &Path) -> Option<Frontmatter> {
        match read_frontmatter(path) {
            Ok(fm) => Some(fm),
            Err(e) => {
                warn!(path = %path.display(), "failed to read frontmatter: {e}");
                None
            }
        }
    }

    pub fn read_page_title_at(&self, frontmatter: &Frontmatter, path: &Path) -> Option<String> {
        if let Some(ref title) = frontmatter.title {
            return Some(title.clone());
        }
        read_first_h1(path)
    }

    pub fn validate_file(&self, path: &Path, ext: &str) -> StorageResult<()> {
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

    // Trees

    fn build_directory_tree(&self, section: Section) -> FileTree {
        let result = build_tree(
            &DiskFs::new(&self.root),
            self.layout,
            section,
            self.locale.as_deref(),
        );
        for issue in &result.issues {
            warn!(path = %issue.path, "{}", issue.message);
        }
        result.tree
    }

    pub fn docs_tree(&self) -> FileTree {
        self.build_directory_tree(Section::Docs)
    }

    pub async fn content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        let tree = self.build_directory_tree(Section::Content);
        let refs = repo.get_page_refs(&tree_file_paths(&tree)).await?;
        Ok(content_tree(
            &DiskFs::new(&self.root),
            self.layout,
            self.locale.as_deref(),
            tree,
            &refs,
        ))
    }
}
