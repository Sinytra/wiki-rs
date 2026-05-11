use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};
use wiki_domain::content::ResourceLocation;

use crate::ingestor::issues::FileIssues;
use crate::ingestor::recipes::parser::{RecipeParseError, RecipeParser};
use crate::ingestor::recipes::types::{StubRecipe, StubRecipeIngredient};

pub struct CustomRecipeParser;

impl RecipeParser for CustomRecipeParser {
    fn handles(&self, loc: &ResourceLocation) -> bool {
        loc.namespace != ResourceLocation::DEFAULT_NAMESPACE
    }

    fn parse(
        &self,
        id: &str,
        recipe_type: &str,
        data: &serde_json::Value,
        _issues: &FileIssues<'_>,
    ) -> Result<Option<StubRecipe>, RecipeParseError> {
        let raw: CustomRecipe =
            serde_json::from_value(data.clone()).map_err(RecipeParseError::InvalidJson)?;

        let mut ingredients = Vec::new();
        for (slot, ing) in raw.input {
            extend_ingredients(&mut ingredients, &slot, true, ing);
        }
        for (slot, ing) in raw.output {
            extend_ingredients(&mut ingredients, &slot, false, ing);
        }

        Ok(Some(StubRecipe {
            id: id.to_owned(),
            r#type: recipe_type.to_owned(),
            ingredients,
        }))
    }
}

#[derive(Debug, Deserialize)]
struct CustomRecipe {
    #[serde(default)]
    input: BTreeMap<String, CustomIngredient>,
    #[serde(default)]
    output: BTreeMap<String, CustomIngredient>,
}

#[derive(Debug, Clone)]
pub enum CustomIngredient {
    Single { id: String, count: i32 },
    Many { ids: Vec<String>, count: i32 },
}

impl<'de> Deserialize<'de> for CustomIngredient {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum IdField {
            One(String),
            Many(Vec<String>),
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            Array(Vec<String>),
            Obj {
                id: IdField,
                #[serde(default = "default_count")]
                count: i32,
            },
        }

        fn default_count() -> i32 {
            1
        }

        Ok(match Raw::deserialize(d)? {
            Raw::Str(s) => CustomIngredient::Single { id: s, count: 1 },
            Raw::Array(v) => CustomIngredient::Many { ids: v, count: 1 },
            Raw::Obj {
                id: IdField::One(s),
                count,
            } => CustomIngredient::Single { id: s, count },
            Raw::Obj {
                id: IdField::Many(v),
                count,
            } => CustomIngredient::Many { ids: v, count },
        })
    }
}

fn extend_ingredients(
    out: &mut Vec<StubRecipeIngredient>,
    slot: &str,
    input: bool,
    ing: CustomIngredient,
) {
    match ing {
        CustomIngredient::Single { id, count } => {
            out.push(single(slot, input, &id, count));
        }
        CustomIngredient::Many { ids, count } => {
            for id in ids {
                out.push(single(slot, input, &id, count));
            }
        }
    }
}

fn single(slot: &str, input: bool, id: &str, count: i32) -> StubRecipeIngredient {
    let is_tag = id.starts_with('#');
    let item_id = if is_tag {
        id[1..].to_owned()
    } else {
        id.to_owned()
    };
    StubRecipeIngredient {
        item_id,
        slot: slot.to_owned(),
        count,
        input,
        is_tag,
    }
}
