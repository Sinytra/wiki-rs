use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;
use serde::de::Error;
use serde_json::Value;

use crate::ingestor::recipes::types::{VanillaIngredient, VanillaIngredientList};

const TYPE_KEYS: [&str; 3] = ["neoforge:ingredient_type", "fabric:type", "type"];

type Parser = fn(Value) -> serde_json::Result<Vec<VanillaIngredient>>;

static REGISTRY: LazyLock<HashMap<&'static str, Parser>> = LazyLock::new(|| {
    HashMap::from([
        ("neoforge:compound", parse_compound as Parser),
        ("neoforge:components", parse_components),
        ("neoforge:difference", parse_base),
        ("neoforge:intersection", parse_intersection),
        ("neoforge:block_tag", parse_block_tag),
        ("neoforge:custom_display", parse_base),
        ("fabric:any", parse_any),
        ("fabric:all", parse_all),
        ("fabric:components", parse_base),
        ("fabric:custom_data", parse_base),
        ("fabric:difference", parse_base),
    ])
});

pub fn ingredient_type(value: &Value) -> Option<&str> {
    TYPE_KEYS
        .iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
}

pub fn resolve(type_id: &str, value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let Some(resolver) = REGISTRY.get(type_id) else {
        return Err(serde_json::Error::custom(format!(
            "unknown custom ingredient type: {type_id}"
        )));
    };
    resolver(value)
}

fn flatten(lists: Vec<VanillaIngredientList>) -> Vec<VanillaIngredient> {
    lists.into_iter().flat_map(|list| list.0).collect()
}

fn non_empty(
    name: &str,
    ingredients: Vec<VanillaIngredient>,
) -> serde_json::Result<Vec<VanillaIngredient>> {
    if ingredients.is_empty() {
        return Err(serde_json::Error::custom(format!(
            "{name} ingredient must have at least one child"
        )));
    }
    Ok(ingredients)
}

#[derive(Deserialize)]
struct CompoundIngredient {
    #[serde(alias = "ingredients")]
    children: Vec<VanillaIngredientList>,
}

fn parse_compound(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: CompoundIngredient = serde_json::from_value(value)?;
    non_empty("compound", flatten(parsed.children))
}

#[derive(Deserialize)]
struct DataComponentIngredient {
    items: VanillaIngredientList,
}

fn parse_components(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: DataComponentIngredient = serde_json::from_value(value)?;
    Ok(parsed.items.0)
}

#[derive(Deserialize)]
struct BaseIngredient {
    base: VanillaIngredientList,
}

fn parse_base(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: BaseIngredient = serde_json::from_value(value)?;
    Ok(parsed.base.0)
}

#[derive(Deserialize)]
struct IntersectionIngredient {
    children: Vec<VanillaIngredientList>,
}

fn parse_intersection(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: IntersectionIngredient = serde_json::from_value(value)?;
    let first = parsed.children.into_iter().next().map(|list| list.0);
    non_empty("intersection", first.unwrap_or_default())
}

#[derive(Deserialize)]
struct BlockTagIngredient {
    tag: String,
}

fn parse_block_tag(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: BlockTagIngredient = serde_json::from_value(value)?;
    let tag = parsed.tag.strip_prefix('#').unwrap_or(&parsed.tag);
    Ok(vec![VanillaIngredient::Tag(tag.to_owned())])
}

#[derive(Deserialize)]
struct CombinedIngredient {
    ingredients: Vec<VanillaIngredientList>,
}

fn parse_any(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: CombinedIngredient = serde_json::from_value(value)?;
    non_empty("any", flatten(parsed.ingredients))
}

fn parse_all(value: Value) -> serde_json::Result<Vec<VanillaIngredient>> {
    let parsed: CombinedIngredient = serde_json::from_value(value)?;
    let first = parsed.ingredients.into_iter().next().map(|list| list.0);
    non_empty("all", first.unwrap_or_default())
}
