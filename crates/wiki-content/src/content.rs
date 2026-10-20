use std::collections::HashMap;

use serde::Serialize;
use wiki_domain::content::ResourceLocation;
use wiki_domain::tree::{ContentFileTree, ContentFileTreeEntry, FileTree, FileType};

use crate::fs::ContentFs;
use crate::layout::{DOCS_FILE_EXT, Layout, Section};
use crate::markdown::parse_frontmatter_text;
use crate::pages::{PageRefs, page_icon, strip_doc_ext};
use crate::tree::{localized_path, page_title};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentIssueKind {
    InvalidFrontmatter,
    MissingIds,
    InvalidId,
    NoPageTitle,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentIssue {
    pub kind: ContentIssueKind,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentPage {
    pub r#ref: String,
    pub slug: String,
    pub path: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ContentScan {
    pub pages: Vec<ContentPage>,
    pub issues: Vec<ContentIssue>,
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn collect_pages(fs: &impl ContentFs, dir: &str, slug: &str, out: &mut Vec<(String, String)>) {
    let mut entries = fs.read_dir(dir).unwrap_or_default();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let page_suffix = format!(".{DOCS_FILE_EXT}");
    for entry in entries {
        let path = join(dir, &entry.name);
        let entry_slug = join(slug, &entry.name);
        if entry.is_dir {
            collect_pages(fs, &path, &entry_slug, out);
        } else if entry.name.ends_with(&page_suffix) && !entry.name.starts_with('.') {
            out.push((path, strip_doc_ext(&entry_slug).to_owned()));
        }
    }
}

fn parse_ids(ids: &[String], modid: &str) -> Result<Vec<String>, String> {
    ids.iter()
        .map(|id| {
            let location = ResourceLocation::parse(id).ok_or_else(|| id.clone())?;
            if location.namespace != modid {
                return Err(format!(
                    "id '{location}' namespace mismatch: expected '{modid}'"
                ));
            }
            Ok(location.to_string())
        })
        .collect()
}

pub fn scan_content_pages(fs: &impl ContentFs, layout: Layout, modid: &str) -> ContentScan {
    let mut files = Vec::new();
    collect_pages(fs, layout.section_dir(Section::Content), "", &mut files);

    let mut scan = ContentScan::default();
    let mut refs = PageRefs::default();
    let mut issue = |kind, path: &str, message: String| {
        scan.issues.push(ContentIssue {
            kind,
            path: path.to_owned(),
            message,
        })
    };

    for (path, slug) in files {
        let Some(text) = fs.read_to_string(&path) else {
            continue;
        };
        let frontmatter = match parse_frontmatter_text(&text) {
            Ok(frontmatter) => frontmatter,
            Err(e) => {
                issue(ContentIssueKind::InvalidFrontmatter, &path, e.to_string());
                continue;
            }
        };
        if frontmatter.id.is_empty() {
            issue(
                ContentIssueKind::MissingIds,
                &path,
                "Missing required property 'id'".to_owned(),
            );
            continue;
        }
        let ids = match parse_ids(&frontmatter.id, modid) {
            Ok(ids) => ids,
            Err(message) => {
                issue(ContentIssueKind::InvalidId, &path, message);
                continue;
            }
        };
        if page_title(&text, &frontmatter).is_none() {
            issue(
                ContentIssueKind::NoPageTitle,
                &path,
                "Page is missing a title. Please add a H1 heading.".to_owned(),
            );
            continue;
        }

        let page_ref = refs.allocate(frontmatter.r#ref.as_deref(), &ids, &slug);
        scan.pages.push(ContentPage {
            r#ref: page_ref,
            slug,
            path,
            ids,
        });
    }
    scan
}

pub fn tree_file_paths(tree: &FileTree) -> Vec<String> {
    let mut out = Vec::new();
    for entry in tree {
        match entry.r#type {
            FileType::Dir => out.extend(tree_file_paths(&entry.children)),
            FileType::File => out.push(entry.path.clone()),
        }
    }
    out
}

pub fn content_tree(
    fs: &impl ContentFs,
    layout: Layout,
    locale: Option<&str>,
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
                item_ids: Vec::new(),
                children: content_tree(fs, layout, locale, entry.children, refs),
            }),
            FileType::File => {
                let page_ref = refs.get(&entry.path)?.clone();
                let page_path = localized_path(
                    fs,
                    layout,
                    locale,
                    &layout.page_path(Section::Content, &entry.path),
                );
                let frontmatter = parse_frontmatter_text(&fs.read_to_string(&page_path)?).ok()?;
                let icon = page_icon(frontmatter.icon, &frontmatter.id, |id| {
                    layout.item_asset_id(id)
                });

                Some(ContentFileTreeEntry {
                    r#ref: Some(page_ref),
                    name: entry.name,
                    icon,
                    path: entry.path,
                    r#type: FileType::File,
                    item_ids: frontmatter.id,
                    children: Vec::new(),
                })
            }
        })
        .collect()
}
