use std::collections::HashMap;

use serde::Serialize;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::tree::{FileTree, FileTreeEntry, FileType};

use crate::fs::ContentFs;
use crate::layout::{FOLDER_META_FILE, Layout, Section};
use crate::markdown::{first_h1, parse_frontmatter_text, parse_mdast};
use crate::metadata::parse_folder_metadata;
use crate::pages::{compare_tree_entries, docs_entry_name, is_tree_entry, strip_doc_ext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeIssueKind {
    FolderMetadata,
    Frontmatter,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeIssue {
    pub kind: TreeIssueKind,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct DocsTree {
    pub tree: FileTree,
    pub issues: Vec<TreeIssue>,
}

struct FolderEntry {
    name: String,
    icon: Option<String>,
}

struct TreeBuilder<'a, F: ContentFs> {
    fs: &'a F,
    layout: Layout,
    locale: Option<&'a str>,
    issues: Vec<TreeIssue>,
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

pub fn build_tree(
    fs: &impl ContentFs,
    layout: Layout,
    section: Section,
    locale: Option<&str>,
) -> DocsTree {
    let mut builder = TreeBuilder {
        fs,
        layout,
        locale,
        issues: Vec::new(),
    };
    let root = layout.section_dir(section);
    let tree = builder.build(root, "");
    DocsTree {
        tree,
        issues: builder.issues,
    }
}

pub fn localized_path(
    fs: &impl ContentFs,
    layout: Layout,
    locale: Option<&str>,
    path: &str,
) -> String {
    if let Some(locale) = locale {
        let candidate = layout.localized_path(path, locale);
        if fs.exists(&candidate) {
            return candidate;
        }
    }
    path.to_owned()
}

pub fn page_title(text: &str, frontmatter: &Frontmatter) -> Option<String> {
    frontmatter
        .title
        .clone()
        .or_else(|| parse_mdast(text).ok().and_then(|tree| first_h1(&tree)))
}

impl<F: ContentFs> TreeBuilder<'_, F> {
    fn localized(&self, path: &str) -> String {
        localized_path(self.fs, self.layout, self.locale, path)
    }

    fn read_frontmatter(&mut self, path: &str) -> Option<(String, Frontmatter)> {
        let text = self.fs.read_to_string(path)?;
        match parse_frontmatter_text(&text) {
            Ok(frontmatter) => Some((text, frontmatter)),
            Err(e) => {
                self.issues.push(TreeIssue {
                    kind: TreeIssueKind::Frontmatter,
                    path: path.to_owned(),
                    message: e.to_string(),
                });
                None
            }
        }
    }

    fn folder_metadata(&mut self, dir: &str) -> (Vec<String>, HashMap<String, FolderEntry>) {
        let mut keys = Vec::new();
        let mut entries = HashMap::new();

        let meta_path = self.localized(&join(dir, FOLDER_META_FILE));
        let Some(text) = self.fs.read_to_string(&meta_path) else {
            return (keys, entries);
        };
        let parsed = match parse_folder_metadata(&text) {
            Ok(parsed) => parsed,
            Err(e) => {
                self.issues.push(TreeIssue {
                    kind: TreeIssueKind::FolderMetadata,
                    path: meta_path,
                    message: e.to_string(),
                });
                return (keys, entries);
            }
        };
        for message in parsed.errors {
            self.issues.push(TreeIssue {
                kind: TreeIssueKind::FolderMetadata,
                path: meta_path.clone(),
                message,
            });
        }

        for entry in parsed.entries {
            let name = entry
                .name
                .or_else(|| {
                    let path = self.localized(&join(dir, &entry.key));
                    let (text, frontmatter) = self.read_frontmatter(&path)?;
                    page_title(&text, &frontmatter)
                })
                .unwrap_or_else(|| docs_entry_name(&entry.key));

            keys.push(entry.key.clone());
            entries.insert(
                entry.key,
                FolderEntry {
                    name,
                    icon: entry.icon,
                },
            );
        }
        (keys, entries)
    }

    fn build(&mut self, dir: &str, slug: &str) -> FileTree {
        let (keys, mut folder) = self.folder_metadata(dir);

        let mut entries: Vec<_> = self
            .fs
            .read_dir(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| is_tree_entry(&e.name, e.is_dir))
            .collect();
        entries.sort_by(|a, b| compare_tree_entries(&keys, &a.name, &b.name));

        let mut tree = FileTree::new();
        for entry in entries {
            let path = join(dir, &entry.name);
            let entry_slug = join(slug, &entry.name);
            let meta = folder.remove(&entry.name);

            let (page, children) = if entry.is_dir {
                (None, self.build(&path, &entry_slug))
            } else {
                (self.read_frontmatter(&self.localized(&path)), Vec::new())
            };

            let name = meta
                .as_ref()
                .map(|m| m.name.clone())
                .or_else(|| {
                    page.as_ref()
                        .and_then(|(text, frontmatter)| page_title(text, frontmatter))
                })
                .unwrap_or_else(|| docs_entry_name(&entry.name));

            tree.push(FileTreeEntry {
                name,
                icon: meta.and_then(|m| m.icon),
                path: if entry.is_dir {
                    entry_slug
                } else {
                    strip_doc_ext(&entry_slug).to_owned()
                },
                r#type: if entry.is_dir {
                    FileType::Dir
                } else {
                    FileType::File
                },
                content_icon: page.and_then(|(_, frontmatter)| frontmatter.icon),
                children,
            });
        }
        tree
    }
}
