use std::collections::{HashMap, HashSet};

use wiki_domain::content::ResourceLocation;
use wiki_domain::pages::links::{ResolvedLink, ResolvedLinkType};

use crate::content::ContentPage;
use crate::fs::ContentFs;
use crate::layout::{Layout, Section};
use crate::links::{LinkTarget, PageLink};
use crate::markdown::{first_h1, parse_frontmatter_text, parse_mdast};
use crate::tree::localized_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    pub r#ref: String,
    pub slug: String,
}

#[derive(Debug, Default)]
pub struct LinkData {
    pub item_pages: HashMap<String, PageInfo>,
    pub ref_slugs: HashMap<String, String>,
    pub vanilla_names: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct LinkRequests {
    pub items: Vec<String>,
    pub refs: Vec<String>,
    pub vanilla: Vec<String>,
}

#[derive(Clone, Copy)]
pub struct ProjectView<'a, F: ContentFs> {
    pub fs: &'a F,
    pub layout: Layout,
    pub locale: Option<&'a str>,
    pub lang: &'a str,
}

pub fn page_title_at(fs: &impl ContentFs, path: &str) -> Option<String> {
    let text = fs.read_to_string(path)?;
    if let Ok(frontmatter) = parse_frontmatter_text(&text)
        && let Some(title) = frontmatter.title
    {
        return Some(title);
    }
    parse_mdast(&text).ok().and_then(|tree| first_h1(&tree))
}

pub fn best_item_pages(pages: &[ContentPage]) -> HashMap<String, PageInfo> {
    let mut best: HashMap<String, (bool, &ContentPage)> = HashMap::new();
    for page in pages {
        let items: HashSet<&String> = page.ids.iter().collect();
        let dedicated = items.len() == 1;
        for id in items {
            let better = match best.get(id) {
                None => true,
                Some((best_dedicated, best_page)) => {
                    (dedicated, std::cmp::Reverse(&page.r#ref))
                        > (*best_dedicated, std::cmp::Reverse(&best_page.r#ref))
                }
            };
            if better {
                best.insert(id.clone(), (dedicated, page));
            }
        }
    }
    best.into_iter()
        .map(|(id, (_, page))| {
            (
                id,
                PageInfo {
                    r#ref: page.r#ref.clone(),
                    slug: page.slug.clone(),
                },
            )
        })
        .collect()
}

pub fn link_data_from_pages(pages: &[ContentPage]) -> LinkData {
    LinkData {
        item_pages: best_item_pages(pages),
        ref_slugs: pages
            .iter()
            .map(|p| (p.r#ref.clone(), p.slug.clone()))
            .collect(),
        vanilla_names: HashMap::new(),
    }
}

pub fn link_requests(links: &[PageLink], modid: Option<&str>) -> LinkRequests {
    let mut requests = LinkRequests::default();
    for link in links {
        match &link.target {
            LinkTarget::Docs { .. } => {}
            LinkTarget::Vanilla { location } => requests.vanilla.push(location.to_string()),
            LinkTarget::Content { location } => {
                if modid == Some(location.namespace.as_str()) {
                    requests.items.push(location.to_string());
                }
            }
            LinkTarget::Ref { r#ref } => requests.refs.push(r#ref.clone()),
        }
    }
    requests
}

impl<F: ContentFs> ProjectView<'_, F> {
    fn localized(&self, path: &str) -> String {
        localized_path(self.fs, self.layout, self.locale, path)
    }

    pub fn content_page_title(&self, slug: &str) -> Option<String> {
        page_title_at(
            self.fs,
            &self.localized(&self.layout.page_path(Section::Content, slug)),
        )
    }

    pub fn lang_key(&self, namespace: &str, key: &str) -> Option<String> {
        let text = self
            .fs
            .read_to_string(&self.layout.language_file_path(namespace, self.lang))?;
        let entries: serde_json::Value = serde_json::from_str(&text).ok()?;
        entries.get(key)?.as_str().map(str::to_owned)
    }

    pub fn item_name(&self, id: &str, page_slug: Option<&str>) -> Option<String> {
        let location = ResourceLocation::parse(id)?;
        ["item", "block"]
            .iter()
            .find_map(|kind| {
                self.lang_key(
                    &location.namespace,
                    &format!("{kind}.{}.{}", location.namespace, location.path),
                )
            })
            .or_else(|| page_slug.and_then(|slug| self.content_page_title(slug)))
    }

    pub fn resolve_links(
        &self,
        links: &[PageLink],
        modid: Option<&str>,
        data: &LinkData,
    ) -> HashMap<String, ResolvedLink> {
        let mut out = HashMap::new();
        for link in links {
            let resolved = match &link.target {
                LinkTarget::Docs { slug } => {
                    let path = self.localized(&self.layout.page_path(Section::Docs, slug));
                    self.fs.exists(&path).then(|| ResolvedLink {
                        r#type: ResolvedLinkType::Docs,
                        r#ref: slug.clone(),
                        title: page_title_at(self.fs, &path),
                        options: link.options.clone(),
                    })
                }
                LinkTarget::Vanilla { location } => Some(ResolvedLink {
                    r#type: ResolvedLinkType::Vanilla,
                    r#ref: location.path.clone(),
                    title: data.vanilla_names.get(&location.to_string()).cloned(),
                    options: link.options.clone(),
                }),
                LinkTarget::Content { location } => {
                    let id = location.to_string();
                    (modid == Some(location.namespace.as_str()))
                        .then(|| data.item_pages.get(&id))
                        .flatten()
                        .map(|page| ResolvedLink {
                            r#type: ResolvedLinkType::Content,
                            r#ref: page.r#ref.clone(),
                            title: self.item_name(&id, Some(&page.slug)),
                            options: link.options.clone(),
                        })
                }
                LinkTarget::Ref { r#ref } => data.ref_slugs.get(r#ref).map(|slug| ResolvedLink {
                    r#type: ResolvedLinkType::Content,
                    r#ref: r#ref.clone(),
                    title: self.content_page_title(slug),
                    options: link.options.clone(),
                }),
            };
            if let Some(resolved) = resolved {
                out.insert(link.original.clone(), resolved);
            }
        }
        out
    }
}
