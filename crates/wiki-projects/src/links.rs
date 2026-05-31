use std::collections::HashMap;

use wiki_db::repo::ProjectRepo;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::DomainResult;
use wiki_domain::pages::links::{ResolvedLink, ResolvedLinkType};
use wiki_domain::project::Project;
use wiki_storage::format::ProjectFormat;

const DOCS_PREFIX: char = '$';
const CONTENT_PREFIX: char = '@';
const REF_PREFIX: char = '+';

pub async fn resolve_page_links(
    format: &ProjectFormat,
    repo: &ProjectRepo,
    current: &dyn Project,
    builtin: &dyn Project,
    modid: Option<&str>,
    links: &[String],
) -> DomainResult<HashMap<String, ResolvedLink>> {
    let mut out = HashMap::new();
    let mut ref_lookups: HashMap<String, String> = HashMap::new();
    let mut content_lookups: HashMap<String, String> = HashMap::new();

    for raw in links {
        if let Some(rest) = raw.strip_prefix(DOCS_PREFIX) {
            if format.doc_page_exists(rest) {
                let title = format.read_page_title(rest);
                out.insert(
                    raw.clone(),
                    ResolvedLink {
                        r#type: ResolvedLinkType::Docs,
                        r#ref: rest.to_owned(),
                        title,
                    },
                );
            }
        } else if let Some(rest) = raw.strip_prefix(CONTENT_PREFIX) {
            let Some(loc) = ResourceLocation::parse(rest) else {
                continue;
            };
            if loc.namespace == ResourceLocation::DEFAULT_NAMESPACE {
                let title = builtin.item_name(rest).await.ok().map(|d| d.name);
                out.insert(
                    raw.clone(),
                    ResolvedLink {
                        r#type: ResolvedLinkType::Vanilla,
                        r#ref: loc.path,
                        title,
                    },
                );
            } else if matches!(modid, Some(m) if m == loc.namespace) {
                content_lookups.insert(loc.to_string(), raw.clone());
            }
        } else if let Some(rest) = raw.strip_prefix(REF_PREFIX) {
            ref_lookups.insert(rest.to_owned(), raw.clone());
        }
    }

    if !content_lookups.is_empty() {
        let locs: Vec<String> = content_lookups.keys().cloned().collect();
        let resolved = repo.resolve_item_page_paths(&locs).await?;

        for (loc, raw) in content_lookups {
            if let Some(p_ref) = resolved.get(&loc) {
                let title = current.item_name(&loc).await.ok().map(|d| d.name);
                out.insert(
                    raw,
                    ResolvedLink {
                        r#type: ResolvedLinkType::Content,
                        r#ref: p_ref.clone(),
                        title,
                    },
                );
            }
        }
    }

    if !ref_lookups.is_empty() {
        let refs: Vec<String> = ref_lookups.keys().cloned().collect();
        let resolved = repo.resolve_page_ref_paths(&refs).await?;

        for (p_ref, raw) in ref_lookups {
            if let Some(path) = resolved.get(&p_ref) {
                // TODO Cache page titles
                let slug = ProjectFormat::slug_from_path(path);
                let title = format.read_page_title(slug);

                out.insert(
                    raw,
                    ResolvedLink {
                        r#type: ResolvedLinkType::Content,
                        r#ref: p_ref.clone(),
                        title,
                    },
                );
            }
        }
    }

    Ok(out)
}
