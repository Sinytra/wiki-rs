use std::path::Path;

use markdown::mdast::Node;
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use wiki_domain::project::Frontmatter;

#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("markdown parse error: {0}")]
    Markdown(String),
    #[error("invalid YAML: {0}")]
    Yaml(String),
}

#[derive(Debug, Deserialize)]
struct RawFrontmatter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    icon: Option<String>,
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

fn parse_mdast(text: &str) -> Result<Node, FrontmatterError> {
    markdown::to_mdast(text, &parse_options())
        .map_err(|e| FrontmatterError::Markdown(e.to_string()))
}

pub fn read_frontmatter(path: &Path) -> Result<Option<Frontmatter>, FrontmatterError> {
    let text = std::fs::read_to_string(path)?;
    parse_frontmatter(&text)
}

pub fn parse_frontmatter(content: &str) -> Result<Option<Frontmatter>, FrontmatterError> {
    let tree = parse_mdast(content)?;

    let Some(children) = tree.children() else {
        return Ok(None);
    };
    let Some(yaml) = children.iter().find_map(|n| match n {
        Node::Yaml(y) => Some(&y.value),
        _ => None,
    }) else {
        return Ok(None);
    };

    let raw: RawFrontmatter =
        serde_yml::from_str(yaml).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;

    Ok(Some(Frontmatter {
        id: raw.id.unwrap_or_default(),
        title: raw.title.unwrap_or_default(),
        icon: raw.icon,
    }))
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
