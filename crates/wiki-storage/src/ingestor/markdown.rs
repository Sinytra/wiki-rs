use std::path::Path;

use wiki_domain::pages::metadata::Frontmatter;

pub use wiki_content::markdown::*;

pub fn read_first_h1(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let tree = parse_mdast(&text).ok()?;
    first_h1(&tree)
}

pub fn read_frontmatter(path: &Path) -> Result<Frontmatter, FrontmatterError> {
    let text = std::fs::read_to_string(path)?;
    parse_frontmatter_text(&text)
}
