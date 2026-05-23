use convert_case::ccase;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tracing::warn;
use wiki_domain::project::{
    ContentFileTree, ContentFileTreeEntry, FileTree, FileTreeEntry, FileType, Frontmatter,
};
use wiki_storage::format::{DOCS_FILE_EXT, ProjectFormat};
use wiki_storage::ingestor::markdown::{read_first_h1, read_frontmatter};

const NO_ICON: &str = "_none";

pub fn read_page_attributes(format: &ProjectFormat, path: &str) -> Option<Frontmatter> {
    let file = format.localized_file_path(path.trim_start_matches('/'));
    match read_frontmatter(&file) {
        Ok(fm) => fm,
        Err(e) => {
            warn!(path = %file.display(), "failed to read frontmatter: {e}");
            None
        }
    }
}

pub fn read_page_title(format: &ProjectFormat, path: &str) -> Option<String> {
    if let Some(fm) = read_page_attributes(format, path)
        && !fm.title.is_empty()
    {
        return Some(fm.title);
    }
    let file = format.localized_file_path(path);
    read_first_h1(&file)
}

pub fn docs_entry_name(file_name: &str) -> String {
    let stem = file_name
        .strip_suffix(&format!(".{DOCS_FILE_EXT}"))
        .unwrap_or(file_name);
    ccase!(camel, stem)
}

#[derive(Debug, Default)]
pub struct FolderMetadata {
    pub keys: Vec<String>,
    pub entries: BTreeMap<String, FolderMetadataEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct FolderMetadataEntry {
    pub name: String,
    pub icon: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawMetaValue {
    String(String),
    Object {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        icon: Option<serde_json::Value>,
    },
}

pub fn read_folder_metadata(format: &ProjectFormat, meta_file: &Path) -> FolderMetadata {
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
    let raw: BTreeMap<String, RawMetaValue> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warn!(path = %meta_file.display(), "invalid folder metadata json: {e}");
            return meta;
        }
    };

    let parent_rel = meta_file
        .parent()
        .and_then(|p| p.strip_prefix(format.root()).ok())
        .map(|p| p.to_path_buf())
        .unwrap_or_default();

    for (key, val) in raw {
        let (mut name, icon) = match val {
            RawMetaValue::String(s) => (s, String::new()),
            RawMetaValue::Object { name, icon } => {
                let icon_str = match icon {
                    Some(serde_json::Value::Null) => NO_ICON.to_owned(),
                    Some(serde_json::Value::String(s)) => s,
                    _ => String::new(),
                };
                (name.unwrap_or_default(), icon_str)
            }
        };

        if name.is_empty() {
            let rel_page = parent_rel.join(&key);
            if let Some(s) = rel_page.to_str() {
                name = read_page_title(format, s).unwrap_or_default();
            }
        }
        if name.is_empty() {
            name = docs_entry_name(&key);
        }

        meta.keys.push(key.clone());
        meta.entries.insert(key, FolderMetadataEntry { name, icon });
    }
    meta
}

pub fn directory_tree(format: &ProjectFormat, dir: &Path) -> FileTree {
    let mut root = FileTree::new();
    let meta_path = format.folder_meta_file_path(dir);
    let folder_meta = read_folder_metadata(format, &meta_path);

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
                Ok(ft) if ft.is_file() => name.ends_with(&format!(".{DOCS_FILE_EXT}")),
                _ => false,
            }
        })
        .collect();

    entries.sort_by(|a, b| compare_entries(&folder_meta.keys, a, b));

    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let rel = entry
            .path()
            .strip_prefix(format.root())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| entry.path());
        let rel_str = rel.to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        let display_path = if is_dir {
            rel_str.clone()
        } else {
            let strip = format!(".{DOCS_FILE_EXT}");
            rel_str.strip_suffix(&strip).unwrap_or(&rel_str).to_owned()
        };

        let (name, icon) = match folder_meta.entries.get(&file_name) {
            Some(e) => (e.name.clone(), e.icon.clone()),
            None => (
                read_page_title(format, &rel_str).unwrap_or_default(),
                String::new(),
            ),
        };
        let name = if name.is_empty() {
            docs_entry_name(&file_name)
        } else {
            name
        };

        let children = if is_dir {
            directory_tree(format, &entry.path())
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

pub fn add_page_metadata(format: &ProjectFormat, tree: FileTree) -> ContentFileTree {
    tree.into_iter()
        .filter_map(|entry| match entry.r#type {
            FileType::Dir => Some(ContentFileTreeEntry {
                id: None,
                name: entry.name,
                icon: entry.icon,
                path: entry.path,
                r#type: FileType::Dir,
                children: add_page_metadata(format, entry.children),
            }),
            FileType::File => {
                let path = format!("{}.{DOCS_FILE_EXT}", entry.path);
                let fm = read_page_attributes(format, &path)?;
                Some(ContentFileTreeEntry {
                    id: Some(fm.id),
                    name: entry.name,
                    icon: fm.icon,
                    path: entry.path,
                    r#type: FileType::File,
                    children: Vec::new(),
                })
            }
        })
        .collect()
}
