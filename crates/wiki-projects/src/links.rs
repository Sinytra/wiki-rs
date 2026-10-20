use std::collections::HashMap;

use wiki_content::links::parse_links;
use wiki_content::resolve::{LinkData, PageInfo, link_requests};
use wiki_db::repo::ProjectRepo;
use wiki_domain::error::DomainResult;
use wiki_domain::pages::links::ResolvedLink;
use wiki_domain::project::Project;
use wiki_storage::format::ProjectFormat;

pub async fn resolve_page_links(
    format: &dyn ProjectFormat,
    repo: &ProjectRepo,
    builtin: &dyn Project,
    modid: Option<&str>,
    lang: &str,
    links: &[String],
) -> DomainResult<HashMap<String, ResolvedLink>> {
    let links = parse_links(links);
    let requests = link_requests(&links, modid);

    let item_refs = repo.resolve_item_page_paths(&requests.items).await?;
    let mut refs = requests.refs;
    refs.extend(item_refs.values().cloned());
    refs.sort();
    refs.dedup();
    let ref_slugs = repo.resolve_page_ref_paths(&refs).await?;

    let item_pages = item_refs
        .into_iter()
        .filter_map(|(id, page_ref)| {
            let slug = ref_slugs.get(&page_ref)?.clone();
            Some((
                id,
                PageInfo {
                    r#ref: page_ref,
                    slug,
                },
            ))
        })
        .collect();

    let mut vanilla_names = HashMap::new();
    for id in requests.vanilla {
        if let Ok(item) = builtin.item_name(&id).await {
            vanilla_names.insert(id, item.name);
        }
    }

    let data = LinkData {
        item_pages,
        ref_slugs,
        vanilla_names,
    };
    Ok(format.resolve_links(&links, modid, lang, &data))
}
