use std::borrow::ToOwned;
use std::collections::HashMap;
use std::string::ToString;
use std::sync::LazyLock;

use wiki_domain::content::{GameRecipeType, ItemSlot, ResourceLocation};
use wiki_storage::ingestor::recipes::types::StubRecipeType;

fn slots(entries: &[(&str, i32, i32)]) -> HashMap<String, ItemSlot> {
    entries
        .iter()
        .map(|(k, x, y)| (k.to_string(), ItemSlot { x: *x, y: *y }))
        .collect()
}

fn recipe_type(background: &str, inputs: &[(&str, i32, i32)], outputs: &[(&str, i32, i32)]) -> StubRecipeType {
    StubRecipeType {
        background: background.to_owned(),
        input_slots: slots(inputs),
        output_slots: slots(outputs),
    }
}

// TODO Move to recipe builtin.rs?
static BUILTIN_RECIPE_TYPES: LazyLock<HashMap<&'static str, StubRecipeType>> = LazyLock::new(|| {
    let crafting = recipe_type(
        "gui/recipe/crafting_shaped",
        &[
            ("1", 16, 16), ("2", 52, 16), ("3", 88, 16),
            ("4", 16, 52), ("5", 52, 52), ("6", 88, 52),
            ("7", 16, 88), ("8", 52, 88), ("9", 88, 88),
        ],
        &[("1", 204, 52)],
    );

    HashMap::from([
        ("crafting_shaped", crafting.clone()),
        ("crafting_shapeless", crafting),
        ("smelting", recipe_type(
            "gui/recipe/smelting",
            &[("1", 14, 14)],
            &[("1", 134, 50)],
        )),
        ("blasting", recipe_type(
            "gui/recipe/blasting",
            &[("1", 14, 14)],
            &[("1", 143, 50)],
        )),
        ("smoking", recipe_type(
            "gui/recipe/smoking",
            &[("1", 14, 14)],
            &[("1", 134, 50)],
        )),
        ("campfire_cooking", recipe_type(
            "gui/recipe/campfire_cooking",
            &[("1", 14, 29)],
            &[("1", 134, 50)],
        )),
        ("stonecutting", recipe_type(
            "gui/recipe/stonecutting",
            &[("1", 16, 50)],
            &[("1", 134, 50)],
        )),
        ("smithing_transform", recipe_type(
            "gui/recipe/smithing",
            &[("1", 16, 50), ("2", 52, 50), ("3", 88, 50)],
            &[("1", 206, 50)],
        )),
    ])
});

pub fn get_builtin_recipe_type(location: &ResourceLocation) -> Option<GameRecipeType> {
    if location.namespace != ResourceLocation::DEFAULT_NAMESPACE {
        return None;
    }
    let stub = BUILTIN_RECIPE_TYPES.get(location.path.as_str())?;
    Some(GameRecipeType {
        id: location.to_string(),
        localized_name: None,
        background: stub.background.clone(),
        input_slots: stub.input_slots.clone(),
        output_slots: stub.output_slots.clone(),
    })
}
