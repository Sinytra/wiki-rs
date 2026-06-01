use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use wiki_domain::content::ResourceLocation;

use crate::ingestor::JsonSource;
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
        data: &JsonSource,
        _issues: &FileIssues<'_>,
    ) -> Result<Option<StubRecipe>, RecipeParseError> {
        let raw: CustomRecipe = data.parse()?;

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

fn default_count() -> i32 {
    1
}

enum IdField {
    One(String),
    Many(Vec<String>),
}

impl<'de> Deserialize<'de> for IdField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdFieldVisitor;

        impl<'de> Visitor<'de> for IdFieldVisitor {
            type Value = IdField;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an id string or a list of id strings")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<IdField, E> {
                Ok(IdField::One(v.to_owned()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<IdField, E> {
                Ok(IdField::One(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<IdField, A::Error> {
                let mut ids = Vec::new();
                while let Some(id) = seq.next_element()? {
                    ids.push(id);
                }
                Ok(IdField::Many(ids))
            }
        }

        deserializer.deserialize_any(IdFieldVisitor)
    }
}

impl<'de> Deserialize<'de> for CustomIngredient {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CustomIngredientVisitor;

        impl<'de> Visitor<'de> for CustomIngredientVisitor {
            type Value = CustomIngredient;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an id string, a list of id strings, \
                    or an object with an 'id' field and an optional 'count'",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<CustomIngredient, E> {
                Ok(CustomIngredient::Single {
                    id: v.to_owned(),
                    count: 1,
                })
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<CustomIngredient, E> {
                Ok(CustomIngredient::Single { id: v, count: 1 })
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<CustomIngredient, A::Error> {
                let mut ids = Vec::new();
                while let Some(id) = seq.next_element()? {
                    ids.push(id);
                }
                Ok(CustomIngredient::Many { ids, count: 1 })
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<CustomIngredient, A::Error> {
                #[derive(Deserialize)]
                struct ObjForm {
                    id: IdField,
                    #[serde(default = "default_count")]
                    count: i32,
                }

                let obj = ObjForm::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(match obj.id {
                    IdField::One(id) => CustomIngredient::Single {
                        id,
                        count: obj.count,
                    },
                    IdField::Many(ids) => CustomIngredient::Many {
                        ids,
                        count: obj.count,
                    },
                })
            }
        }

        deserializer.deserialize_any(CustomIngredientVisitor)
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
