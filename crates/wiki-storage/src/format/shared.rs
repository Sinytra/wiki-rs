use crate::error::{StorageError, StorageResult};
use crate::format::reader::{FolderMetadata, FolderMetadataEntry, RawPage, RuntimeReadError};
use crate::format::{DOCS_FILE_DOT_EXT, DOCS_FILE_EXT, NO_ICON, reader};
use crate::format::{FOLDER_META_FILE, ProjectFormat};
use crate::ingestor::markdown::{parse_frontmatter, parse_mdast, read_first_h1, read_frontmatter};
use crate::ingestor::try_parse_json_path;
use convert_case::ccase;
use garde::Validate;
use std::cell::LazyCell;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tracing::warn;
use wiki_db::repo::ProjectRepo;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::{MetadataError, ProjectMetadata};
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::{
    ContentFileTree, ContentFileTreeEntry, FileTree, FileTreeEntry, FileType,
};

pub trait ProjectFormatInternal: ProjectFormat {
    fn locale(&self) -> Option<&str>;

    fn folder_meta_file_path(&self, dir: &Path) -> PathBuf {
        if let Some(loc) = &self.locale()
            && let Ok(rel) = dir.strip_prefix(self.root())
        {
            let candidate = self
                .translated_root()
                .join(loc)
                .join(rel)
                .join(FOLDER_META_FILE);
            if candidate.exists() {
                return candidate;
            }
        }
        dir.join(FOLDER_META_FILE)
    }

    fn localized_file_path(&self, path: &str) -> PathBuf {
        let trimmed = path.trim_start_matches('/');
        if let Some(loc) = self.locale() {
            let candidate = self.translated_root().join(loc).join(trimmed);
            if candidate.exists() {
                return candidate;
            }
        }
        self.root().join(trimmed)
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

    fn build_directory_tree(&self, dir: &Path) -> FileTree {
        self.build_directory_tree_recursive(dir, dir)
    }

    fn build_directory_tree_recursive(&self, root_dir: &Path, dir: &Path) -> FileTree {
        let mut tree_root = FileTree::new();
        let meta_path = self.folder_meta_file_path(dir);
        let folder_meta = self.read_folder_metadata(&meta_path);

        let read = match fs::read_dir(dir) {
            Ok(r) => r,
            Err(_) => return tree_root,
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
            let entry_path = entry.path();
            let rel_str = entry_path
                .strip_prefix(self.root())
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            let path_slug = if is_dir {
                rel_str.clone()
            } else {
                self.slug_from_path(root_dir, &entry_path).to_owned()
            };

            let fm = LazyCell::new(|| read_frontmatter_at(&entry_path));

            let (name, icon) = match folder_meta.entries.get(&file_name) {
                Some(e) => (Some(e.name.clone()), Some(e.icon.clone())),
                None => (
                    fm.as_ref()
                        .and_then(|f| self.read_page_title_at(f, &entry_path)),
                    None,
                ),
            };
            let name = name.unwrap_or_else(|| docs_entry_name(&file_name));
            let content_icon = if is_dir {
                None
            } else {
                fm.as_ref().and_then(|f| f.icon.to_owned())
            };

            let children = if is_dir {
                self.build_directory_tree_recursive(root_dir, &entry_path)
            } else {
                Vec::new()
            };

            tree_root.push(FileTreeEntry {
                name,
                icon,
                content_icon,
                path: path_slug,
                r#type: if is_dir {
                    FileType::Dir
                } else {
                    FileType::File
                },
                children,
            });
        }
        tree_root
    }

    async fn build_content_tree(&self, repo: &ProjectRepo) -> StorageResult<ContentFileTree> {
        let path = self.contents_root();
        let tree = self.build_directory_tree(&path);

        let mut paths = Vec::new();
        self.collect_file_paths(&tree, &mut paths);

        let refs = repo.get_page_refs(&paths).await?;
        Ok(self.construct_content_tree(tree, &refs))
    }

    fn construct_content_tree(
        &self,
        tree: FileTree,
        slug_to_ref: &HashMap<String, String>,
    ) -> ContentFileTree {
        tree.into_iter()
            .filter_map(|entry| match entry.r#type {
                FileType::Dir => Some(ContentFileTreeEntry {
                    r#ref: None,
                    name: entry.name,
                    icon: None,
                    path: entry.path,
                    r#type: FileType::Dir,
                    item_ids: Vec::new(),
                    children: self.construct_content_tree(entry.children, slug_to_ref),
                }),
                FileType::File => {
                    let page_path = self.content_page_path(&entry.path);
                    let page_ref = slug_to_ref.get(&entry.path)?.clone();
                    let fm = self.try_read_frontmatter_at(&page_path)?;
                    let icon = self.get_page_icon(fm.icon.clone(), &fm.id);

                    Some(ContentFileTreeEntry {
                        r#ref: Some(page_ref),
                        name: entry.name,
                        icon,
                        path: entry.path,
                        r#type: FileType::File,
                        item_ids: fm.id,
                        children: Vec::new(),
                    })
                }
            })
            .collect()
    }

    fn collect_file_paths(&self, tree: &FileTree, out: &mut Vec<String>) {
        for entry in tree {
            match entry.r#type {
                FileType::Dir => self.collect_file_paths(&entry.children, out),
                FileType::File => out.push(entry.path.clone()),
            }
        }
    }

    fn get_page_icon(&self, icon: Option<String>, ids: &[String]) -> Option<String> {
        icon.or_else(|| ids.first().map(|id| self.item_asset_id(id)))
    }
}

fn is_doc_file(name: &str) -> bool {
    name.ends_with(DOCS_FILE_DOT_EXT)
}

pub fn language_file_path(
    root: &Path,
    assets_dir: &str,
    assets_lang_dir: &str,
    namespace: &str,
    locale: &str,
) -> PathBuf {
    root.join(assets_dir)
        .join(namespace)
        .join(assets_lang_dir)
        .join(format!("{locale}.json"))
}

pub async fn read_metadata_async<F>(format: F) -> StorageResult<ProjectMetadata>
where
    F: ProjectFormat + Send + Clone + 'static,
{
    tokio::task::spawn_blocking({
        let format = format.clone();
        move || format.read_metadata()
    })
    .await
    .map_err(|e| StorageError::Internal(e.to_string()))?
}

pub fn read_metadata(path: &Path) -> StorageResult<ProjectMetadata> {
    if !path.exists() {
        return Err(StorageError::project(
            ProjectError::NoPath,
            format!("Metadata file '{}' missing", path.display()),
        ));
    }

    let metadata: ProjectMetadata = try_parse_json_path("project metadata", path)
        .map_err(StorageError::to_invalid_meta)?
        .value;

    metadata
        .validate()
        .map_err(|e| MetadataError::Validate(e.to_string()))
        .map_err(|e| {
            StorageError::project(
                ProjectError::InvalidMeta,
                format!("Failed to parse metadata file: {e}"),
            )
        })?;

    Ok(metadata)
}

pub fn read_page_at(path: &Path) -> Result<RawPage, RuntimeReadError> {
    let content = fs::read_to_string(path).map_err(|e| match e.kind() {
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

pub fn read_title_at(path: &Path) -> Option<String> {
    if let Some(fm) = read_frontmatter_at(path)
        && let Some(title) = fm.title
    {
        return Some(title);
    }
    read_first_h1(path)
}

pub fn read_frontmatter_at(path: &Path) -> Option<Frontmatter> {
    match read_frontmatter(path) {
        Ok(fm) => fm,
        Err(e) => {
            warn!(path = %path.display(), "failed to read frontmatter: {e}");
            None
        }
    }
}

pub fn append_doc_ext(slug: &str) -> String {
    format!("{slug}.{DOCS_FILE_EXT}")
}

fn docs_entry_name(file_name: &str) -> String {
    let stem = strip_doc_ext(file_name);
    ccase!(camel, stem)
}

pub fn strip_doc_ext(path: &str) -> &str {
    path.strip_suffix(DOCS_FILE_DOT_EXT).unwrap_or(path)
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
