use std::collections::HashMap;

use garde::Validate;
use markdown::mdast::Node;
use markdown::{Constructs, ParseOptions};
use serde::Deserialize;
use wiki_domain::pages::metadata::{
    Changelog, Frontmatter, GameContentType, Infobox, InfoboxTab, InvItem, check_resource_location,
};
use wiki_domain::util::{string_or_seq, string_or_seq_opt};

#[derive(Default, Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
struct RawFrontmatter {
    /// Must be valid ResourceLocation
    #[serde(default, deserialize_with = "string_or_seq")]
    #[garde(inner(custom(check_resource_location)))]
    pub id: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// Must be alphanum and '_' only
    #[serde(default)]
    #[garde(inner(pattern(r"^[a-z0-9_]+$")))]
    pub r#ref: Option<String>,
    /// Must be valid ResourceLocation
    #[serde(default)]
    #[garde(inner(custom(check_resource_location)))]
    pub icon: Option<String>,
    #[serde(default)]
    #[garde(dive)]
    pub infobox: Option<RawInfobox>,
    #[serde(default, rename = "type")]
    pub r#type: Option<GameContentType>,
    #[serde(default)]
    pub custom: Option<HashMap<String, String>>,
    #[serde(default)]
    pub history: Option<Changelog>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[garde(allow_unvalidated)]
struct RawInfobox {
    pub title: Option<String>,
    /// Must be list of valid ResourceLocations
    /// Mutually exclusive with "tabs"
    #[serde(default, deserialize_with = "string_or_seq_opt")]
    #[garde(
        inner(inner(custom(check_resource_location))),
        custom(|_, _| check_display_tabs_exclusive(self))
    )]
    pub display: Option<Vec<String>>,
    #[serde(default)]
    #[garde(dive)]
    pub tabs: Option<Vec<InfoboxTab>>,
    /// Must be list of valid ResourceLocations
    #[serde(default, deserialize_with = "string_or_seq_opt")]
    #[garde(inner(inner(custom(check_resource_location))))]
    pub inventory: Option<Vec<String>>,
}

fn check_display_tabs_exclusive(infobox: &RawInfobox) -> garde::Result {
    if infobox.display.is_some() && infobox.tabs.is_some() {
        return Err(garde::Error::new(
            "infobox 'display' and 'tabs' are mutually exclusive",
        ));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("markdown parse error: {0}")]
    Markdown(String),
    #[error("invalid YAML: {0}")]
    Yaml(String),
    #[error("invalid frontmatter: {0}")]
    Validate(String),
}

#[allow(clippy::field_reassign_with_default)]
fn parse_options() -> ParseOptions {
    let mut opts = ParseOptions::default();
    opts.constructs = Constructs {
        frontmatter: true,
        html_flow: false,
        code_indented: false,
        ..Constructs::default()
    };
    opts
}

pub fn first_h1(tree: &Node) -> Option<String> {
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

pub fn collect_links(tree: &Node) -> Vec<String> {
    let mut urls = Vec::new();
    visit(tree, &mut urls);
    urls
}

pub fn parse_frontmatter_text(text: &str) -> Result<Frontmatter, FrontmatterError> {
    let tree = parse_mdast(frontmatter_block(text))?;
    parse_frontmatter(&tree)
}

fn frontmatter_block(text: &str) -> &str {
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return "";
    };
    if first.trim_end() != "---" {
        return "";
    }

    let mut end = first.len();
    for line in lines {
        end += line.len();
        if line.trim_end() == "---" {
            return &text[..end];
        }
    }
    ""
}

pub fn parse_frontmatter(tree: &Node) -> Result<Frontmatter, FrontmatterError> {
    let Some(children) = tree.children() else {
        return Ok(Frontmatter::default());
    };
    let Some(yaml) = children.iter().find_map(|n| match n {
        Node::Yaml(y) => Some(&y.value),
        _ => None,
    }) else {
        return Ok(Frontmatter::default());
    };

    let mut frontmatter: RawFrontmatter =
        serde_yml::from_str(yaml).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;

    frontmatter
        .validate()
        .map_err(|e| FrontmatterError::Validate(e.to_string()))?;

    if let Some(ref mut infobox) = frontmatter.infobox
        && let Some(display) = infobox.display.take()
    {
        infobox.tabs.replace(Vec::from(&[InfoboxTab {
            name: "".into(),
            display,
        }]));
    }

    Ok(Frontmatter {
        id: frontmatter.id,
        title: frontmatter.title,
        r#ref: frontmatter.r#ref,
        icon: frontmatter.icon,
        infobox: frontmatter.infobox.map(|i| Infobox {
            title: i.title,
            tabs: i.tabs,
            inventory: i.inventory.map(|inv| {
                inv.into_iter()
                    .map(|asset_id| InvItem {
                        id: None,
                        name: None,
                        asset_id,
                    })
                    .collect()
            }),
        }),
        r#type: frontmatter.r#type,
        custom: frontmatter.custom,
        history: frontmatter.history,
    })
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
