use serde::Serialize;
use wiki_domain::content::ResourceLocation;

const DOCS_PREFIX: char = '$';
const CONTENT_PREFIX: char = '@';
const REF_PREFIX: char = '+';

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkTarget {
    Docs { slug: String },
    Vanilla { location: ResourceLocation },
    Content { location: ResourceLocation },
    Ref { r#ref: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageLink {
    pub original: String,
    pub target: LinkTarget,
    pub options: Option<String>,
}

pub fn parse_link(raw: &str) -> Option<PageLink> {
    let (href, anchor) = raw.split_once('#').unwrap_or((raw, ""));
    let options = (!anchor.is_empty()).then(|| format!("#{anchor}"));

    let target = if let Some(slug) = href.strip_prefix(DOCS_PREFIX) {
        LinkTarget::Docs {
            slug: slug.to_owned(),
        }
    } else if let Some(rest) = href.strip_prefix(CONTENT_PREFIX) {
        let location = ResourceLocation::parse(rest)?;
        if location.namespace == ResourceLocation::DEFAULT_NAMESPACE {
            LinkTarget::Vanilla { location }
        } else {
            LinkTarget::Content { location }
        }
    } else if let Some(rest) = href.strip_prefix(REF_PREFIX) {
        LinkTarget::Ref {
            r#ref: rest.to_owned(),
        }
    } else {
        return None;
    };

    Some(PageLink {
        original: raw.to_owned(),
        target,
        options,
    })
}

pub fn parse_links(raw: &[String]) -> Vec<PageLink> {
    raw.iter().filter_map(|l| parse_link(l)).collect()
}
