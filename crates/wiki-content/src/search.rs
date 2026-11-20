use std::collections::HashMap;

use markdown::mdast::Node;

pub const MAX_CONTENT_LEN: usize = 32 * 1024; // 32 kb

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHeading {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchText {
    pub headings: Vec<SearchHeading>,
    pub content: String,
}

pub fn extract_search_text(tree: &Node) -> SearchText {
    let mut state = ExtractState::default();
    state.extract(tree);

    let mut content = state
        .content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_char(&mut content, MAX_CONTENT_LEN);

    SearchText {
        headings: state.headings,
        content,
    }
}

#[derive(Default)]
struct ExtractState {
    headings: Vec<SearchHeading>,
    content: String,
    slugs: Slugger,
    seen_title: bool,
}

impl ExtractState {
    fn extract(&mut self, node: &Node) {
        match node {
            Node::Heading(h) => {
                let only_title = h.depth == 1 && matches!(h.children.as_slice(), [Node::Text(_)]);
                if only_title && !self.seen_title {
                    self.seen_title = true;
                    return;
                }

                let mut display = String::new();
                heading_text(node, false, &mut display);
                let display = display.split_whitespace().collect::<Vec<_>>().join(" ");
                if display.is_empty() {
                    return;
                }

                let id = if only_title {
                    String::new()
                } else {
                    let mut slug_source = String::new();
                    heading_text(node, true, &mut slug_source);
                    self.slugs.slug(&slug_source)
                };

                self.headings.push(SearchHeading { text: display, id });
                return;
            }
            Node::Text(t) => self.content.push_str(&t.value),
            Node::InlineCode(c) => self.content.push_str(&c.value),
            Node::Code(c) => {
                self.content.push(' ');
                self.content.push_str(&c.value);
                self.content.push(' ');
            }
            Node::Break(_) => self.content.push(' '),
            Node::Yaml(_) | Node::Toml(_) | Node::Html(_) | Node::MdxjsEsm(_) => return,
            Node::MdxFlowExpression(_) | Node::MdxTextExpression(_) => return,
            _ => {}
        }

        if let Some(children) = node.children() {
            let block = is_block(node);
            for child in children {
                self.extract(child);
                if block {
                    self.content.push(' ');
                }
            }
        }
    }
}

fn is_block(node: &Node) -> bool {
    matches!(
        node,
        Node::Root(_)
            | Node::Blockquote(_)
            | Node::List(_)
            | Node::ListItem(_)
            | Node::Table(_)
            | Node::TableRow(_)
            | Node::FootnoteDefinition(_)
            | Node::MdxJsxFlowElement(_)
    )
}

fn heading_text(node: &Node, include_image_alt: bool, out: &mut String) {
    match node {
        Node::Text(t) => out.push_str(&t.value),
        Node::InlineCode(c) => out.push_str(&c.value),
        Node::Image(img) if include_image_alt => out.push_str(&img.alt),
        _ => {}
    }
    if let Some(children) = node.children() {
        for child in children {
            heading_text(child, include_image_alt, out);
        }
    }
}

fn truncate_char(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
}

#[derive(Default)]
struct Slugger {
    occurrences: HashMap<String, usize>,
}

impl Slugger {
    fn slug(&mut self, value: &str) -> String {
        let base = slugify(value);
        let mut result = base.clone();
        while self.occurrences.contains_key(&result) {
            let count = self.occurrences.entry(base.clone()).or_default();
            *count += 1;
            result = format!("{base}-{count}");
        }
        self.occurrences.insert(result.clone(), 0);
        result
    }
}

fn slugify(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}
