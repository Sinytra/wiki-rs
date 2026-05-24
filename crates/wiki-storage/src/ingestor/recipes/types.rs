use std::collections::HashMap;
use std::fmt;
use serde::{de, Deserialize, Deserializer};
use serde::de::{MapAccess, Visitor};
use wiki_domain::content::ItemSlot;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")] // TODO snake_case (part of format v1)
pub struct StubRecipeType {
    pub background: String,
    pub input_slots: HashMap<String, ItemSlot>,
    pub output_slots: HashMap<String, ItemSlot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PreparedRecipeType {
    pub id: String,
    pub background: String,
    pub input_slots: HashMap<String, ItemSlot>,
    pub output_slots: HashMap<String, ItemSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubRecipeIngredient {
    pub item_id: String,
    pub slot: String,
    pub count: i32,
    pub input: bool,
    pub is_tag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubRecipe {
    pub id: String,
    pub r#type: String,
    pub ingredients: Vec<StubRecipeIngredient>,
}

#[derive(Debug, Clone)]
pub enum VanillaIngredient {
    Item(String),
    Tag(String),
}

impl VanillaIngredient {
    pub fn into_stub(self, slot: String, count: i32, input: bool) -> StubRecipeIngredient {
        let (id, is_tag) = match self {
            VanillaIngredient::Item(s) => (s, false),
            VanillaIngredient::Tag(s) => (s, true),
        };
        StubRecipeIngredient {
            item_id: id,
            slot,
            count,
            input,
            is_tag,
        }
    }
}

impl<'de> Deserialize<'de> for VanillaIngredient {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)] // TODO Improve error reporting
        enum Raw {
            Str(String),
            Item { item: String },
            Tag { tag: String },
        }

        Ok(match Raw::deserialize(d)? {
            Raw::Str(s) => {
                if let Some(rest) = s.strip_prefix('#') {
                    VanillaIngredient::Tag(rest.to_owned())
                } else {
                    VanillaIngredient::Item(s)
                }
            }
            Raw::Item { item } => VanillaIngredient::Item(item),
            Raw::Tag { tag } => VanillaIngredient::Tag(tag),
        })
    }
}

#[derive(Debug, Clone)]
pub struct VanillaIngredientList(pub Vec<VanillaIngredient>);

impl<'de> Deserialize<'de> for VanillaIngredientList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Many(Vec<VanillaIngredient>),
            One(VanillaIngredient),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Many(v) => VanillaIngredientList(v),
            Raw::One(v) => VanillaIngredientList(vec![v]),
        })
    }
}

#[derive(Debug, Clone)]
pub struct VanillaResult {
    pub id: String, // TODO Must be valid ResLoc
    pub count: i32,
}

impl<'de> Deserialize<'de> for VanillaResult {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct VanillaResultVisitor;

        impl<'de> Visitor<'de> for VanillaResultVisitor {
            type Value = VanillaResult;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string item id or an object with 'id' and optional 'count'")
            }

            // Plain string form
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(VanillaResult {
                    id: v.to_owned(),
                    count: 1,
                })
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(VanillaResult { id: v, count: 1 })
            }

            // Object form
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                struct ObjectForm {
                    id: String,
                    #[serde(default = "default_count")]
                    count: i32,
                }
                fn default_count() -> i32 { 1 }

                let obj = ObjectForm::deserialize(
                    de::value::MapAccessDeserializer::new(map),
                )?;
                Ok(VanillaResult {
                    id: obj.id,
                    count: obj.count,
                })
            }
        }

        deserializer.deserialize_any(VanillaResultVisitor)
    }
}
