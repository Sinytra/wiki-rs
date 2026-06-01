use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, de};
use std::collections::HashMap;
use std::fmt;
use wiki_domain::content::ItemSlot;

#[derive(Debug, Clone, Deserialize)]
pub struct StubRecipeType {
    pub background: String,
    #[serde(alias = "inputSlots")]
    pub input_slots: HashMap<String, ItemSlot>,
    #[serde(alias = "outputSlots")]
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

    fn parse(s: &str) -> VanillaIngredient {
        if let Some(rest) = s.strip_prefix('#') {
            VanillaIngredient::Tag(rest.to_owned())
        } else {
            VanillaIngredient::Item(s.to_owned())
        }
    }
}

impl<'de> Deserialize<'de> for VanillaIngredient {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct VanillaIngredientVisitor;

        impl<'de> Visitor<'de> for VanillaIngredientVisitor {
            type Value = VanillaIngredient;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "an item or tag id string, an object with an 'item' field, \
                    or an object with a 'tag' field",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<VanillaIngredient, E> {
                Ok(VanillaIngredient::parse(v))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<VanillaIngredient, E> {
                Ok(VanillaIngredient::parse(&v))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<VanillaIngredient, A::Error> {
                let value: serde_json::Value =
                    Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))?;

                if value.get("item").is_some() {
                    #[derive(Deserialize)]
                    struct ItemForm {
                        item: String,
                    }
                    let form: ItemForm =
                        serde_json::from_value(value).map_err(de::Error::custom)?;
                    Ok(VanillaIngredient::Item(form.item))
                } else if value.get("tag").is_some() {
                    #[derive(Deserialize)]
                    struct TagForm {
                        tag: String,
                    }
                    let form: TagForm = serde_json::from_value(value).map_err(de::Error::custom)?;
                    Ok(VanillaIngredient::Tag(form.tag))
                } else {
                    Err(de::Error::custom(
                        "ingredient object must have either an 'item' or a 'tag' field",
                    ))
                }
            }
        }

        deserializer.deserialize_any(VanillaIngredientVisitor)
    }
}

#[derive(Debug, Clone)]
pub struct VanillaIngredientList(pub Vec<VanillaIngredient>);

impl<'de> Deserialize<'de> for VanillaIngredientList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct VanillaIngredientListVisitor;

        impl<'de> Visitor<'de> for VanillaIngredientListVisitor {
            type Value = VanillaIngredientList;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an ingredient or a list of ingredients")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<VanillaIngredientList, E> {
                Ok(VanillaIngredientList(vec![VanillaIngredient::parse(v)]))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<VanillaIngredientList, E> {
                Ok(VanillaIngredientList(vec![VanillaIngredient::parse(&v)]))
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<VanillaIngredientList, A::Error> {
                let ingredient =
                    VanillaIngredient::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(VanillaIngredientList(vec![ingredient]))
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<VanillaIngredientList, A::Error> {
                let mut items = Vec::new();
                while let Some(ingredient) = seq.next_element()? {
                    items.push(ingredient);
                }
                Ok(VanillaIngredientList(items))
            }
        }

        deserializer.deserialize_any(VanillaIngredientListVisitor)
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
                fn default_count() -> i32 {
                    1
                }

                let obj = ObjectForm::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(VanillaResult {
                    id: obj.id,
                    count: obj.count,
                })
            }
        }

        deserializer.deserialize_any(VanillaResultVisitor)
    }
}
