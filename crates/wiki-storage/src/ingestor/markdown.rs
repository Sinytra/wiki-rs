use std::collections::HashMap;
use std::path::Path;

use markdown::mdast::Node;
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use wiki_domain::error::DomainError;
use wiki_domain::pages::metadata::{Changelog, Frontmatter, GameContentType, Infobox, InfoboxTab};
use wiki_domain::util::string_or_seq;

#[derive(Default, Debug, Clone, Deserialize)]
struct RawFrontmatter {
    #[serde(default, deserialize_with = "string_or_seq")]
    pub id: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    // TODO Validate: must be alphanum and '_' only
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub infobox: Option<RawInfobox>,
    #[serde(default, rename = "type")]
    pub r#type: Option<GameContentType>,
    #[serde(default)]
    pub custom: Option<HashMap<String, String>>,
    #[serde(default)]
    pub history: Option<Changelog>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawInfobox {
    pub title: Option<String>,
    #[serde(default)]
    pub display: Option<Vec<String>>,
    #[serde(default)]
    pub tabs: Option<Vec<InfoboxTab>>,
    #[serde(default, deserialize_with = "string_or_seq")]
    pub inventory: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("markdown parse error: {0}")]
    Markdown(String),
    #[error("invalid YAML: {0}")]
    Yaml(String),
}

impl From<FrontmatterError> for DomainError {
    fn from(err: FrontmatterError) -> Self {
        DomainError::Internal(err.to_string())
    }
}

#[allow(clippy::field_reassign_with_default)]
fn parse_options() -> ParseOptions {
    let mut opts = ParseOptions::default();
    opts.constructs = Constructs {
        frontmatter: true,
        ..Constructs::default()
    };
    opts
}

pub fn read_first_h1(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let tree = parse_mdast(&text).ok()?;
    let children = tree.children()?;

    for node in children {
        if let Node::Heading(h) = node
            && h.depth == 1
        {
            let mut text = String::new();
            collect_text(node, &mut text);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            return Some(trimmed.to_owned());
        }
    }
    None
}

pub fn read_frontmatter(path: &Path) -> Result<Option<Frontmatter>, FrontmatterError> {
    let tree = read_tree(path)?;
    parse_frontmatter(&tree)
}

pub fn read_tree(path: &Path) -> Result<Node, FrontmatterError> {
    let text = std::fs::read_to_string(path)?;
    let tree = parse_mdast(&text)?;
    Ok(tree)
}

pub fn collect_links(tree: &Node) -> Vec<String> {
    let mut urls = Vec::new();
    visit(tree, &mut urls);
    urls
}

pub fn parse_frontmatter(tree: &Node) -> Result<Option<Frontmatter>, FrontmatterError> {
    let Some(children) = tree.children() else {
        return Ok(None);
    };
    let Some(yaml) = children.iter().find_map(|n| match n {
        Node::Yaml(y) => Some(&y.value),
        _ => None,
    }) else {
        return Ok(None);
    };

    let mut frontmatter: RawFrontmatter =
        serde_yml::from_str(yaml).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;

    // TODO Validate frontmatter (common between ingestor and project)
    if let Some(ref mut infobox) = frontmatter.infobox
        && let Some(display) = infobox.display.take()
    {
        if infobox.tabs.is_some() {
            // TODO Return error
        }

        infobox.tabs.replace(Vec::from(&[InfoboxTab {
            name: "".into(),
            display
        }]));
    }

    Ok(Some(Frontmatter {
        id: frontmatter.id,
        title: frontmatter.title,
        r#ref: frontmatter.r#ref,
        icon: frontmatter.icon,
        infobox: frontmatter.infobox.map(|i| Infobox {
            title: i.title,
            tabs: i.tabs,
            inventory: i.inventory
        }),
        r#type: frontmatter.r#type,
        custom: frontmatter.custom,
        history: frontmatter.history,
    }))
}

fn visit(node: &Node, urls: &mut Vec<String>) {
    if let Node::Link(link) = node {
        urls.push(link.url.clone())
    }

    if let Some(children) = node.children() {
        for child in children {
            visit(child, urls);
        }
    }
}

pub fn parse_mdast(text: &str) -> Result<Node, FrontmatterError> {
    markdown::to_mdast(text, &parse_options())
        .map_err(|e| FrontmatterError::Markdown(e.to_string()))
}

fn collect_text(node: &Node, out: &mut String) {
    if let Node::Text(t) = node {
        out.push_str(&t.value);
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_text(child, out);
        }
    }
}
