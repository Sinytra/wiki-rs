use wiki_domain::pages::metadata::{Frontmatter, Infobox, InfoboxTab, InvItem};

use crate::tab_names::infer_tab_names;

pub fn default_infobox(
    frontmatter: &Frontmatter,
    names: &[String],
    asset_id: impl Fn(&str) -> String,
) -> Infobox {
    let ids = &frontmatter.id;

    let mut tabs: Vec<InfoboxTab> = ids
        .iter()
        .zip(infer_tab_names(names))
        .map(|(id, name)| InfoboxTab {
            name,
            display: vec![asset_id(id)],
        })
        .collect();

    if let [tab] = tabs.as_mut_slice()
        && let Some(ref icon) = frontmatter.icon
    {
        tab.display = vec![icon.clone()];
    }

    let inventory = ids
        .iter()
        .zip(names)
        .map(|(id, name)| InvItem {
            id: Some(id.to_owned()),
            name: Some(name.to_owned()),
            asset_id: asset_id(id),
        })
        .collect();

    Infobox {
        title: frontmatter.title.clone(),
        tabs: Some(tabs),
        inventory: Some(inventory),
    }
}

pub fn merge_infobox(default: Infobox, user: Option<Infobox>) -> Infobox {
    let Some(user) = user else {
        return default;
    };
    Infobox {
        title: user.title.or(default.title),
        tabs: user.tabs.or(default.tabs),
        inventory: user.inventory.or(default.inventory),
    }
}
