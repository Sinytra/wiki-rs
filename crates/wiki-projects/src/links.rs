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

#[derive(Debug, Clone)]
struct LinkParts {
    original: String,
    href: String,
    options: Option<String>,
}

type LinkLookups = HashMap<String, Vec<LinkParts>>;

fn split_link(raw: &str) -> LinkParts {
    let (href, anchor) = raw.split_once('#').unwrap_or((raw, ""));
    let options = (!anchor.is_empty()).then(|| format!("#{anchor}"));
    LinkParts {
        original: raw.to_owned(),
        href: href.to_owned(),
        options,
    }
}

pub async fn resolve_page_links(
    format: &dyn ProjectFormat,
    repo: &ProjectRepo,
    current: &dyn Project,
    builtin: &dyn Project,
    modid: Option<&str>,
    links: &[String],
) -> DomainResult<HashMap<String, ResolvedLink>> {
    let mut out = HashMap::new();
    let mut ref_lookups: LinkLookups = HashMap::new();
    let mut content_lookups: LinkLookups = HashMap::new();

    for raw in links {
        let link = split_link(raw);

        if let Some(slug) = link.href.strip_prefix(DOCS_PREFIX) {
            let page_path = format.docs_page_path(slug);
            if page_path.exists() {
                let title = format.read_page_title(&page_path);
                out.insert(
                    link.original,
                    ResolvedLink {
                        r#type: ResolvedLinkType::Docs,
                        r#ref: slug.to_owned(),
                        title,
                        options: link.options,
                    },
                );
            }
        } else if let Some(rest) = link.href.strip_prefix(CONTENT_PREFIX) {
            let Some(loc) = ResourceLocation::parse(rest) else {
                continue;
            };
            if loc.namespace == ResourceLocation::DEFAULT_NAMESPACE {
                let title = builtin.item_name(rest).await.ok().map(|d| d.name);
                out.insert(
                    link.original,
                    ResolvedLink {
                        r#type: ResolvedLinkType::Vanilla,
                        r#ref: loc.path,
                        title,
                        options: link.options,
                    },
                );
            } else if matches!(modid, Some(m) if m == loc.namespace) {
                content_lookups
                    .entry(loc.to_string())
                    .or_default()
                    .push(link);
            }
        } else if let Some(rest) = link.href.strip_prefix(REF_PREFIX) {
            ref_lookups.entry(rest.to_owned()).or_default().push(link);
        }
    }

    if !content_lookups.is_empty() {
        let locs: Vec<String> = content_lookups.keys().cloned().collect();
        let resolved = repo.resolve_item_page_paths(&locs).await?;

        for (loc, entries) in content_lookups {
            if let Some(p_ref) = resolved.get(&loc) {
                let title = current.item_name(&loc).await.ok().map(|d| d.name);

                for link in entries {
                    out.insert(
                        link.original,
                        ResolvedLink {
                            r#type: ResolvedLinkType::Content,
                            r#ref: p_ref.clone(),
                            title: title.clone(),
                            options: link.options,
                        },
                    );
                }
            }
        }
    }

    if !ref_lookups.is_empty() {
        let refs: Vec<String> = ref_lookups.keys().cloned().collect();
        let resolved = repo.resolve_page_ref_paths(&refs).await?;

        for (p_ref, entries) in ref_lookups {
            if let Some(slug) = resolved.get(&p_ref) {
                // TODO Cache page titles
                let page_path = format.content_page_path(slug);
                let title = format.read_page_title(&page_path);

                for link in entries {
                    out.insert(
                        link.original,
                        ResolvedLink {
                            r#type: ResolvedLinkType::Content,
                            r#ref: p_ref.clone(),
                            title: title.clone(),
                            options: link.options,
                        },
                    );
                }
            }
        }
    }

    Ok(out)
}
