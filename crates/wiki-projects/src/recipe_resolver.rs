use std::collections::HashMap;
use std::sync::Arc;

use wiki_db::entity::{recipe, recipe_ingredient_item, recipe_ingredient_tag};
use wiki_db::query;
use wiki_db::query::project as project_query;
use wiki_domain::content::{
    RecipeIngredientSummary, RecipeSummary, ResolvedGameRecipe, ResolvedItem, ResolvedSlot,
};
use wiki_domain::error::DomainError;

use crate::resolver::ProjectResolver;

pub struct RecipeResolver {
    resolver: Arc<ProjectResolver>,
    locale: Option<String>,
}

impl RecipeResolver {
    pub fn new(resolver: Arc<ProjectResolver>, locale: Option<String>) -> Self {
        Self { resolver, locale }
    }

    pub async fn resolve(&self, recipe: &recipe::Model) -> Result<ResolvedGameRecipe, DomainError> {
        let db = self.resolver.db();

        let r_type = query::recipe::get_recipe_type(db, recipe.type_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let item_ings = query::recipe::get_item_ingredients(db, recipe.id).await?;
        let tag_ings = query::recipe::get_tag_ingredients(db, recipe.id).await?;

        let mut slots: Vec<ResolvedSlot> = Vec::new();
        for ing in item_ings {
            slots.push(self.resolve_item_ingredient(&ing).await?);
        }
        for ing in tag_ings {
            slots.push(self.resolve_tag_ingredient(&ing).await?);
        }

        let inputs = merge_slots(&slots, true);
        let outputs = merge_slots(&slots, false);
        let summary = RecipeSummary {
            inputs: ingredient_summary(&inputs),
            outputs: ingredient_summary(&outputs),
        };

        Ok(ResolvedGameRecipe {
            id: recipe.loc.clone(),
            r#type: r_type.loc,
            inputs,
            outputs,
            summary,
        })
    }

    async fn resolve_item_ingredient(
        &self,
        ing: &recipe_ingredient_item::Model,
    ) -> Result<ResolvedSlot, DomainError> {
        let db = self.resolver.db();
        let item_row = query::recipe::get_item(db, ing.item_id).await?;

        let mut items: Vec<ResolvedItem> = Vec::new();
        if let Some(item_row) = item_row {
            let sources = project_query::get_item_source_projects(db, item_row.id)
                .await?;
            for project_id in sources {
                items.push(self.resolve_item(&project_id, &item_row.loc).await);
            }
        }
        Ok(ResolvedSlot {
            input: ing.input,
            slot: ing.slot.clone(),
            count: ing.count,
            items,
            tag: None,
        })
    }

    async fn resolve_tag_ingredient(
        &self,
        ing: &recipe_ingredient_tag::Model,
    ) -> Result<ResolvedSlot, DomainError> {
        let db = self.resolver.db();
        let tag_row = query::recipe::get_tag(db, ing.tag_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let items_in_tag = self
            .resolver
            .get_global_tag_items(ing.tag_id)
            .await?;
        let mut items: Vec<ResolvedItem> = Vec::new();
        for entry in items_in_tag {
            let Some(project_id) = entry.project_id.as_deref() else {
                continue;
            };
            items.push(self.resolve_item(project_id, &entry.loc).await);
        }

        Ok(ResolvedSlot {
            input: ing.input,
            slot: ing.slot.clone(),
            count: ing.count,
            items,
            tag: Some(tag_row.loc),
        })
    }

    async fn resolve_item(&self, project_id: &str, loc: &str) -> ResolvedItem {
        match self
            .resolver
            .resolve_item_data(project_id, loc, self.locale.as_deref())
            .await
        {
            Some(data) => ResolvedItem {
                id: loc.to_owned(),
                name: Some(data.name),
                project: project_id.to_owned(),
                page_ref: data.page_ref,
            },
            None => ResolvedItem {
                id: loc.to_owned(),
                name: None,
                project: project_id.to_owned(),
                page_ref: None,
            },
        }
    }
}

fn merge_slots(slots: &[ResolvedSlot], input: bool) -> Vec<ResolvedSlot> {
    let mut map: HashMap<String, ResolvedSlot> = HashMap::new();
    for slot in slots.iter().filter(|s| s.input == input) {
        match map.get_mut(&slot.slot) {
            Some(existing) => existing.items.extend(slot.items.iter().cloned()),
            None => {
                map.insert(slot.slot.clone(), slot.clone());
            }
        }
    }
    map.into_values().collect()
}

fn ingredient_summary(slots: &[ResolvedSlot]) -> Vec<RecipeIngredientSummary> {
    let mut counts: HashMap<String, i32> = HashMap::new();
    let mut items: HashMap<String, ResolvedItem> = HashMap::new();
    for slot in slots {
        let Some(first) = slot.items.first() else {
            continue;
        };
        let key = match &slot.tag {
            Some(t) => format!("#{t}"),
            None => first.id.clone(),
        };
        if !counts.contains_key(&key) {
            items.insert(key.clone(), first.clone());
        }
        *counts.entry(key).or_insert(0) += slot.count;
    }

    counts
        .into_iter()
        .map(|(key, count)| {
            let item = items.remove(&key).unwrap();
            let tag = key.strip_prefix('#').map(str::to_owned);
            RecipeIngredientSummary { count, item, tag }
        })
        .collect()
}
