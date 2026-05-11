use std::collections::HashMap;

use serde::Deserialize;
use wiki_domain::content::ResourceLocation;

use crate::ingestor::issues::FileIssues;
use crate::ingestor::recipes::parser::{RecipeParseError, RecipeParser};
use crate::ingestor::recipes::types::{
    StubRecipe, StubRecipeIngredient, VanillaIngredientList, VanillaResult,
};

type Processor = fn(
    &str,
    &str,
    &serde_json::Value,
    &FileIssues<'_>,
) -> Result<Option<StubRecipe>, RecipeParseError>;

pub struct VanillaRecipeParser {
    processors: HashMap<&'static str, Processor>,
}

impl Default for VanillaRecipeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl VanillaRecipeParser {
    pub fn new() -> Self {
        let mut processors: HashMap<&'static str, Processor> = HashMap::new();
        processors.insert("minecraft:crafting_shaped", parse_shaped);
        processors.insert("minecraft:crafting_shapeless", parse_shapeless);
        processors.insert("minecraft:smelting", parse_single);
        processors.insert("minecraft:blasting", parse_single);
        processors.insert("minecraft:campfire_cooking", parse_single);
        processors.insert("minecraft:smoking", parse_single);
        processors.insert("minecraft:stonecutting", parse_single);
        processors.insert("minecraft:smithing_transform", parse_smithing_transform);
        Self { processors }
    }
}

impl RecipeParser for VanillaRecipeParser {
    fn handles(&self, loc: &ResourceLocation) -> bool {
        loc.namespace == ResourceLocation::DEFAULT_NAMESPACE
    }

    fn parse(
        &self,
        id: &str,
        recipe_type: &str,
        data: &serde_json::Value,
        issues: &FileIssues<'_>,
    ) -> Result<Option<StubRecipe>, RecipeParseError> {
        let Some(proc) = self.processors.get(recipe_type) else {
            return Err(RecipeParseError::UnknownRecipeType(recipe_type.to_owned()));
        };
        proc(id, recipe_type, data, issues)
    }
}

fn push_list(out: &mut Vec<StubRecipeIngredient>, list: VanillaIngredientList, slot: &str) {
    for ing in list.0 {
        out.push(ing.into_stub(slot.to_owned(), 1, true));
    }
}

fn push_result(out: &mut Vec<StubRecipeIngredient>, result: VanillaResult, slot: &str) {
    out.push(StubRecipeIngredient {
        item_id: result.id,
        slot: slot.to_owned(),
        count: result.count,
        input: false,
        is_tag: false,
    });
}

#[derive(Deserialize)]
struct ShapedRecipe {
    pattern: Vec<String>,
    key: HashMap<String, VanillaIngredientList>,
    result: VanillaResult,
}

fn parse_shaped(
    id: &str,
    r#type: &str,
    data: &serde_json::Value,
    _issues: &FileIssues<'_>,
) -> Result<Option<StubRecipe>, RecipeParseError> {
    let ShapedRecipe {
        pattern,
        key,
        result,
    } = serde_json::from_value(data.clone())
        .map_err(RecipeParseError::InvalidJson)?;

    let mut ingredients = Vec::new();
    for (row_idx, row) in pattern.iter().enumerate() {
        for (col_idx, ch) in row.chars().enumerate() {
            let slot_idx = row_idx * 3 + col_idx + 1;
            let slot = slot_idx.to_string();

            let k = ch.to_string();
            if k.trim().is_empty() {
                continue;
            }

            if let Some(list) = key.get(&k).cloned() {
                push_list(&mut ingredients, list, &slot);
            }
        }
    }
    push_result(&mut ingredients, result, "1");
    Ok(Some(StubRecipe {
        id: id.to_owned(),
        r#type: r#type.to_owned(),
        ingredients,
    }))
}

#[derive(Deserialize)]
struct ShapelessRecipe {
    ingredients: Vec<VanillaIngredientList>,
    result: VanillaResult,
}

fn parse_shapeless(
    id: &str,
    r#type: &str,
    data: &serde_json::Value,
    _issues: &FileIssues<'_>,
) -> Result<Option<StubRecipe>, RecipeParseError> {
    let ShapelessRecipe {
        ingredients,
        result,
    } = serde_json::from_value(data.clone()).map_err(RecipeParseError::InvalidJson)?;

    let mut out = Vec::new();
    for (i, list) in ingredients.into_iter().enumerate() {
        let slot = (i + 1).to_string();
        push_list(&mut out, list, &slot);
    }
    push_result(&mut out, result, "1");
    Ok(Some(StubRecipe {
        id: id.to_owned(),
        r#type: r#type.to_owned(),
        ingredients: out,
    }))
}

#[derive(Deserialize)]
struct SingleIngredientRecipe {
    ingredient: VanillaIngredientList,
    result: VanillaResult,
}

fn parse_single(
    id: &str,
    r#type: &str,
    data: &serde_json::Value,
    _issues: &FileIssues<'_>,
) -> Result<Option<StubRecipe>, RecipeParseError> {
    let SingleIngredientRecipe { ingredient, result } =
        serde_json::from_value(data.clone()).map_err(RecipeParseError::InvalidJson)?;

    let mut out = Vec::new();
    push_list(&mut out, ingredient, "1");
    push_result(&mut out, result, "1");
    Ok(Some(StubRecipe {
        id: id.to_owned(),
        r#type: r#type.to_owned(),
        ingredients: out,
    }))
}

#[derive(Deserialize)]
struct SmithingTransformRecipe {
    template: VanillaIngredientList,
    base: VanillaIngredientList,
    addition: VanillaIngredientList,
    result: VanillaResult,
}

fn parse_smithing_transform(
    id: &str,
    r#type: &str,
    data: &serde_json::Value,
    _issues: &FileIssues<'_>,
) -> Result<Option<StubRecipe>, RecipeParseError> {
    let SmithingTransformRecipe {
        template,
        base,
        addition,
        result,
    } = serde_json::from_value(data.clone()).map_err(RecipeParseError::InvalidJson)?;

    let mut out = Vec::new();
    push_list(&mut out, template, "0");
    push_list(&mut out, base, "1");
    push_list(&mut out, addition, "2");
    push_result(&mut out, result, "1");
    Ok(Some(StubRecipe {
        id: id.to_owned(),
        r#type: r#type.to_owned(),
        ingredients: out,
    }))
}
