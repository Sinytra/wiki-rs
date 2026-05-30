use crate::error::{StorageError, StorageResult};
use crate::format::reader::{FolderMetadata, FolderMetadataEntry, RawPage, RuntimeReadError};
use crate::ingestor::markdown::{parse_frontmatter, parse_mdast, read_first_h1, read_frontmatter};
use convert_case::ccase;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tracing::warn;
use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::ProjectMetadata;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{
    ContentFileTree, ContentFileTreeEntry, FileTree, FileTreeEntry, FileType,
};
use wiki_domain::util::LogErr;
use crate::ingestor::try_parse_json_path;

pub mod builtin;
mod reader;

pub const DOCS_FILE_EXT: &str = "mdx";
const DOCS_FILE_DOT_EXT: &str = ".mdx";
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

const NO_ICON: &str = "_none";

#[derive(Debug, Clone)]
pub struct ProjectFormat {
    root: PathBuf,
    locale: Option<String>,
    data_root_override: Option<PathBuf>,
}

impl ProjectFormat {
    pub fn slug_from_path(path: &str) -> &str {
        path.strip_suffix(DOCS_FILE_DOT_EXT).unwrap_or(path)
    }

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

    pub fn clone_with_root(&self, root: PathBuf) -> Self {
        Self {
            root,
            locale: self.locale.clone(),
            data_root_override: None,
        }
    }

    pub fn doc_page_exists(&self, slug: &str) -> bool {
        self.doc_page_path(slug).exists()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets_root(&self) -> PathBuf {
        self.root.join(ASSETS_DIR)
    }

    pub fn data_root(&self) -> PathBuf {
        self.data_root_override
            .clone()
            .unwrap_or_else(|| self.root.join(DATA_DIR))
    }

    pub fn recipes_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(modid).join("recipes")
    }

    pub fn recipe_types_root(&self, modid: &str) -> PathBuf {
        self.data_root().join(modid).join("recipe_type")
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
        let ext = if location.path.contains('.') {
            ""
        } else {
            ".png"
        };
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

    pub async fn read_metadata_async(&self) -> StorageResult<ProjectMetadata> {
        tokio::task::spawn_blocking({
            let format = self.clone();
            move || format.read_metadata()
        })
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?
    }

    pub fn read_metadata(&self) -> StorageResult<ProjectMetadata> {
        let meta_path = self.wiki_metadata_path();
        if !meta_path.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!("Metadata file '{}' missing", meta_path.display()),
            ));
        }

        let text = fs::read_to_string(&meta_path)
            .map_err_log("error reading metadata file", |_| {
                StorageError::project(ProjectError::NoPath, "Failed to read metadata file")
            })?;

        ProjectMetadata::parse(&text).map_err(|e| {
            StorageError::project(
                ProjectError::InvalidMeta,
                format!("Failed to parse metadata file: {e}"),
            )
        })
    }

    pub fn read_page(&self, slug: &str) -> Result<RawPage, RuntimeReadError> {
        let path = self.doc_page_path(slug);
        let content = fs::read_to_string(&path).map_err(|e| match e.kind() {
            ErrorKind::NotFound => RuntimeReadError::NotFound,
            _ => RuntimeReadError::Io,
        })?;
        let tree = parse_mdast(&content).map_err(|_| RuntimeReadError::MalformedMarkdown)?;
        let frontmatter = parse_frontmatter(&tree)
            .map_err(|_| RuntimeReadError::MalformedFrontmatter)?
            .unwrap_or_default();
        Ok(RawPage {
            content,
            tree,
            frontmatter,
        })
    }

    pub fn try_read_frontmatter(&self, slug: &str) -> Option<Frontmatter> {
        read_frontmatter_at(&self.doc_page_path(slug))
    }

    pub fn read_page_title(&self, slug: &str) -> Option<String> {
        read_title_at(&self.doc_page_path(slug))
    }

    pub fn read_page_title_from(&self, frontmatter: &Frontmatter, slug: &str) -> Option<String> {
        if let Some(ref title) = frontmatter.title {
            return Some(title.clone());
        }
        read_first_h1(&self.doc_page_path(slug))
    }

    pub fn directory_tree(&self, dir: &Path) -> FileTree {
        let mut root = FileTree::new();
        let meta_path = self.folder_meta_file_path(dir);
        let folder_meta = self.read_folder_metadata(&meta_path);

        let read = match fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => return root,
        };

        let mut entries: Vec<_> = read
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name.starts_with('_') {
                    return false;
                }
                match e.file_type() {
                    Ok(ft) if ft.is_dir() => true,
                    Ok(ft) if ft.is_file() => is_doc_file(&name),
                    _ => false,
                }
            })
            .collect();

        entries.sort_by(|a, b| compare_entries(&folder_meta.keys, a, b));

        for entry in entries {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let rel = entry
                .path()
                .strip_prefix(self.root())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| entry.path());
            let rel_str = rel.to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            let display_path = if is_dir {
                rel_str.clone()
            } else {
                ProjectFormat::doc_slug_from_file_name(&rel_str)
                    .unwrap_or(&rel_str)
                    .to_owned()
            };

            let (name, icon) = match folder_meta.entries.get(&file_name) {
                Some(e) => (e.name.clone(), e.icon.clone()),
                None => (
                    read_title_at(&entry.path()).unwrap_or_default(),
                    String::new(),
                ),
            };
            let name = if name.is_empty() {
                docs_entry_name(&file_name)
            } else {
                name
            };

            let children = if is_dir {
                self.directory_tree(&entry.path())
            } else {
                Vec::new()
            };

            root.push(FileTreeEntry {
                name,
                icon: if icon.is_empty() { None } else { Some(icon) },
                path: display_path,
                r#type: if is_dir {
                    FileType::Dir
                } else {
                    FileType::File
                },
                children,
            });
        }
        root
    }

    pub async fn content_tree(
        &self,
        repo: &ProjectRepo,
        path: &Path,
    ) -> StorageResult<ContentFileTree> {
        let tree = self.directory_tree(path);

        let mut paths = Vec::new();
        self.collect_file_paths(&tree, &mut paths);

        let refs = repo.get_page_refs(&paths).await?;
        Ok(self.build_content_tree(tree, &refs))
    }

    pub fn validate_file(&self, path: &Path, ext: &str) -> StorageResult<()> {
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

    fn read_folder_metadata(&self, meta_file: &Path) -> FolderMetadata {
        let mut meta = FolderMetadata::default();
        if !meta_file.exists() {
            return meta;
        }
        let text = match fs::read_to_string(meta_file) {
            Ok(t) => t,
            Err(e) => {
                warn!(path = %meta_file.display(), "failed reading folder metadata: {e}");
                return meta;
            }
        };
        let raw: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                warn!(path = %meta_file.display(), "invalid folder metadata json: {e}");
                return meta;
            }
        };

        let parent_rel = meta_file
            .parent()
            .and_then(|p| p.strip_prefix(self.root()).ok())
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        for (key, val) in raw {
            let parsed: reader::RawMetaValue = match serde_json::from_value(val) {
                Ok(p) => p,
                Err(e) => {
                    warn!(path = %meta_file.display(), key = %key, "invalid folder metadata entry: {e}");
                    continue;
                }
            };
            let (mut name, icon) = match parsed {
                reader::RawMetaValue::String(s) => (s, String::new()),
                reader::RawMetaValue::Object { name, icon } => {
                    let icon_str = match icon {
                        Some(serde_json::Value::Null) => NO_ICON.to_owned(),
                        Some(serde_json::Value::String(s)) => s,
                        _ => String::new(),
                    };
                    (name.unwrap_or_default(), icon_str)
                }
            };

            if name.is_empty()
                && let Some(s) = parent_rel.join(&key).to_str()
            {
                name = read_title_at(&self.localized_file_path(s)).unwrap_or_default();
            }
            if name.is_empty() {
                name = docs_entry_name(&key);
            }

            meta.keys.push(key.clone());
            meta.entries.insert(key, FolderMetadataEntry { name, icon });
        }
        meta
    }

    fn build_content_tree(
        &self,
        tree: FileTree,
        refs: &HashMap<String, String>,
    ) -> ContentFileTree {
        tree.into_iter()
            .filter_map(|entry| match entry.r#type {
                FileType::Dir => Some(ContentFileTreeEntry {
                    r#ref: None,
                    name: entry.name,
                    icon: None,
                    path: entry.path,
                    r#type: FileType::Dir,
                    children: self.build_content_tree(entry.children, refs),
                }),
                FileType::File => {
                    let db_key = self.doc_db_path(&entry.path);
                    let page_ref = refs.get(&db_key)?.clone();
                    let fm = self.try_read_frontmatter(&entry.path)?;
                    let icon = Self::get_page_icon(fm.icon.clone(), &fm.id);

                    Some(ContentFileTreeEntry {
                        r#ref: Some(page_ref),
                        name: entry.name,
                        icon,
                        path: entry.path,
                        r#type: FileType::File,
                        children: Vec::new(),
                    })
                }
            })
            .collect()
    }

    fn folder_meta_file_path(&self, dir: &Path) -> PathBuf {
        if let Some(loc) = &self.locale
            && let Ok(rel) = dir.strip_prefix(&self.root)
        {
            let candidate = self
                .root
                .join(I18N_DIR)
                .join(loc)
                .join(rel)
                .join(FOLDER_META_FILE);
            if candidate.exists() {
                return candidate;
            }
        }
        dir.join(FOLDER_META_FILE)
    }

    fn collect_file_paths(&self, tree: &FileTree, out: &mut Vec<String>) {
        for entry in tree {
            match entry.r#type {
                FileType::Dir => self.collect_file_paths(&entry.children, out),
                FileType::File => out.push(self.doc_db_path(&entry.path)),
            }
        }
    }

    fn get_page_icon(icon: Option<String>, ids: &[String]) -> Option<String> {
        icon.or_else(|| ids.first().map(String::to_owned))
    }

    fn doc_db_path(&self, slug: &str) -> String {
        format!("{slug}.{DOCS_FILE_EXT}")
    }

    fn doc_page_path(&self, slug: &str) -> PathBuf {
        self.localized_file_path(&self.doc_db_path(slug))
    }

    fn localized_file_path(&self, path: &str) -> PathBuf {
        let trimmed = path.trim_start_matches('/');
        if let Some(loc) = &self.locale {
            let candidate = self.root.join(I18N_DIR).join(loc).join(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root.join(trimmed)
    }

    fn doc_slug_from_file_name(name: &str) -> Option<&str> {
        name.strip_suffix(DOCS_FILE_DOT_EXT)
    }
}

fn is_doc_file(name: &str) -> bool {
    name.ends_with(DOCS_FILE_DOT_EXT)
}

fn read_title_at(path: &Path) -> Option<String> {
    if let Some(fm) = read_frontmatter_at(path)
        && let Some(title) = fm.title
    {
        return Some(title);
    }
    read_first_h1(path)
}

fn read_frontmatter_at(path: &Path) -> Option<Frontmatter> {
    match read_frontmatter(path) {
        Ok(fm) => fm,
        Err(e) => {
            warn!(path = %path.display(), "failed to read frontmatter: {e}");
            None
        }
    }
}

fn docs_entry_name(file_name: &str) -> String {
    let stem = ProjectFormat::doc_slug_from_file_name(file_name).unwrap_or(file_name);
    ccase!(camel, stem)
}

fn compare_entries(keys: &[String], a: &fs::DirEntry, b: &fs::DirEntry) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let an = a.file_name().to_string_lossy().into_owned();
    let bn = b.file_name().to_string_lossy().into_owned();

    if keys.is_empty() {
        return an.cmp(&bn);
    }
    let ai = keys.iter().position(|k| k == &an);
    let bi = keys.iter().position(|k| k == &bn);
    match (ai, bi) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(a), Some(b)) => a.cmp(&b),
    }
}
