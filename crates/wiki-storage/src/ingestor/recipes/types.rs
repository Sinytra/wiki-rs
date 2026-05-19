use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use wiki_domain::content::ItemSlot;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
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
        StubRecipeIngredient { item_id: id, slot, count, input, is_tag }
    }
}

impl<'de> Deserialize<'de> for VanillaIngredient {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
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
    pub id: String,
    pub count: i32,
}

impl<'de> Deserialize<'de> for VanillaResult {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            Obj {
                id: String,
                #[serde(default = "default_count")]
                count: i32,
            },
        }
        fn default_count() -> i32 {
            1
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Str(id) => VanillaResult { id, count: 1 },
            Raw::Obj { id, count } => VanillaResult { id, count },
        })
    }
}
