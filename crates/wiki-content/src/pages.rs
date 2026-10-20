use std::cmp::Ordering;
use std::collections::HashSet;

use wiki_domain::content::ResourceLocation;

const DOCS_FILE_DOT_EXT: &str = ".mdx";

#[derive(Debug, Default)]
pub struct PageRefs {
    existing: HashSet<String>,
}

impl PageRefs {
    pub fn allocate(&mut self, user_ref: Option<&str>, ids: &[String], slug: &str) -> String {
        let page_ref = self.derive(user_ref, ids, slug);
        self.existing.insert(page_ref.clone());
        page_ref
    }

    fn derive(&self, user_ref: Option<&str>, ids: &[String], slug: &str) -> String {
        if let Some(custom) = user_ref
            && !self.existing.contains(custom)
        {
            return custom.to_owned();
        }

        if let [id] = ids
            && let Some(parsed) = ResourceLocation::parse(id)
        {
            let primary_ref = parsed.path.replace('/', "_");
            if !self.existing.contains(&primary_ref) {
                return primary_ref;
            }
        }

        if let Some(file_name) = slug.rsplit('/').next()
            && !self.existing.contains(file_name)
        {
            return file_name.to_owned();
        }

        slug.replace('/', "_")
    }
}

pub fn is_tree_entry(name: &str, is_dir: bool) -> bool {
    if name.starts_with('.') || name.starts_with('_') {
        return false;
    }
    is_dir || name.ends_with(DOCS_FILE_DOT_EXT)
}

pub fn compare_tree_entries(keys: &[String], a: &str, b: &str) -> Ordering {
    let position = |name: &str| keys.iter().position(|k| k == name);
    match (position(a), position(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

pub fn strip_doc_ext(path: &str) -> &str {
    path.strip_suffix(DOCS_FILE_DOT_EXT).unwrap_or(path)
}

pub fn docs_entry_name(file_name: &str) -> String {
    let stem = strip_doc_ext(file_name);

    let mut words = Vec::new();
    let mut current = String::new();
    for ch in stem.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
            current.push(ch);
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
        .iter()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + &c.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn page_icon(
    icon: Option<String>,
    ids: &[String],
    asset_id: impl Fn(&str) -> String,
) -> Option<String> {
    icon.or_else(|| ids.first().map(|id| asset_id(id)))
}
