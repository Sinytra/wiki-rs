use wiki_storage::ingestor::recipes::parser::RecipeParseError;

use crate::support::{parse_recipe, stub};

#[test]
fn string_ids_and_slot_names() {
    // language=json
    let json = r##"{
        "type": "somemod:machine",
        "input": {
            "left": "minecraft:a",
            "right": "#minecraft:b"
        },
        "output": {
            "out": "minecraft:c"
        }
    }"##;
    let recipe = parse_recipe("test:custom", json).recipe();

    assert_eq!(recipe.r#type, "somemod:machine");
    assert_eq!(
        recipe.ingredients,
        vec![
            stub("minecraft:a", "left", 1, true, false),
            stub("minecraft:b", "right", 1, true, true),
            stub("minecraft:c", "out", 1, false, false),
        ]
    );
}

#[test]
fn list_of_ids_expands_into_one_slot() {
    // language=json
    let json = r##"{
        "type": "somemod:machine",
        "input": {
            "1": ["minecraft:a", "#minecraft:b"]
        },
        "output": {}
    }"##;
    let recipe = parse_recipe("test:custom", json).recipe();

    assert_eq!(
        recipe.ingredients,
        vec![
            stub("minecraft:a", "1", 1, true, false),
            stub("minecraft:b", "1", 1, true, true),
        ]
    );
}

#[test]
fn object_with_id_and_count() {
    // language=json
    let json = r##"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "id": "minecraft:a",
                "count": 3
            }
        },
        "output": {
            "1": {
                "id": "#minecraft:b",
                "count": 7
            },
            "2": {
                "id": "minecraft:c"
            }
        }
    }"##;
    let recipe = parse_recipe("test:custom", json).recipe();

    assert_eq!(
        recipe.ingredients,
        vec![
            stub("minecraft:a", "1", 3, true, false),
            stub("minecraft:b", "1", 7, false, true),
            stub("minecraft:c", "2", 1, false, false),
        ]
    );
}

#[test]
fn object_with_id_list_shares_count() {
    // language=json
    let json = r##"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "id": ["minecraft:a", "#minecraft:b"],
                "count": 2
            }
        },
        "output": {}
    }"##;
    let recipe = parse_recipe("test:custom", json).recipe();

    assert_eq!(
        recipe.ingredients,
        vec![
            stub("minecraft:a", "1", 2, true, false),
            stub("minecraft:b", "1", 2, true, true),
        ]
    );
}

#[test]
fn slots_are_emitted_in_sorted_order() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "b": "minecraft:b",
            "a": "minecraft:a",
            "10": "minecraft:ten",
            "2": "minecraft:two"
        },
        "output": {}
    }"#;
    let recipe = parse_recipe("test:custom", json).recipe();

    let slots: Vec<&str> = recipe.ingredients.iter().map(|i| i.slot.as_str()).collect();
    assert_eq!(slots, vec!["10", "2", "a", "b"]);
}

#[test]
fn missing_input_and_output_default_to_empty() {
    // language=json
    let json = r#"{
        "type": "somemod:machine"
    }"#;
    let recipe = parse_recipe("test:custom", json).recipe();
    assert_eq!(recipe.ingredients, vec![]);

    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {},
        "output": {}
    }"#;
    let recipe = parse_recipe("test:custom", json).recipe();
    assert_eq!(recipe.ingredients, vec![]);
}

#[test]
fn extra_fields_are_ignored() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "energy": 4000,
        "input": {
            "1": "minecraft:a"
        },
        "output": {
            "1": {
                "id": "minecraft:b",
                "chance": 0.5
            }
        }
    }"#;
    let recipe = parse_recipe("test:custom", json).recipe();
    assert_eq!(recipe.ingredients.len(), 2);
}

#[test]
fn any_non_minecraft_namespace_is_handled() {
    // language=json
    let template = r#"{
        "type": "$TYPE",
        "input": {
            "1": "minecraft:a"
        }
    }"#;
    for ty in [
        "somemod:x",
        "a-b_c.d:thing/sub",
        "fabric:foo",
        "neoforge:foo",
    ] {
        let json = template.replace("$TYPE", ty);
        let recipe = parse_recipe("test:custom", &json).recipe();
        assert_eq!(recipe.r#type, ty);
    }
}

#[test]
fn loader_ingredient_objects_are_not_accepted_in_custom_recipes() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "neoforge:ingredient_type": "neoforge:compound",
                "children": ["minecraft:a"]
            }
        }
    }"#;
    let err = parse_recipe("test:custom", json).error();
    assert!(matches!(err, RecipeParseError::InvalidJsonPath(_)), "{err}");
    assert!(err.to_string().contains("id"), "{err}");
}

#[test]
fn vanilla_item_tag_objects_are_not_accepted_in_custom_recipes() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "item": "minecraft:a"
            }
        }
    }"#;
    let err = parse_recipe("test:custom", json).error();
    assert!(err.to_string().contains("id"), "{err}");
}

#[test]
fn invalid_count_type_is_rejected() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "id": "minecraft:a",
                "count": "3"
            }
        }
    }"#;
    let err = parse_recipe("test:custom", json).error();
    assert!(err.to_string().contains("count"), "{err}");
}

#[test]
fn invalid_id_list_entry_is_rejected() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": ["minecraft:a", 5]
        }
    }"#;
    parse_recipe("test:custom", json).error();
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": {
                "id": [
                    {
                        "id": "minecraft:a"
                    }
                ]
            }
        }
    }"#;
    parse_recipe("test:custom", json).error();
}

#[test]
fn input_must_be_an_object() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": ["minecraft:a"]
    }"#;
    let err = parse_recipe("test:custom", json).error();
    assert!(err.to_string().contains("input"), "{err}");
}

#[test]
fn scalar_ingredient_values_are_rejected() {
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": 5
        }
    }"#;
    parse_recipe("test:custom", json).error();
    // language=json
    let json = r#"{
        "type": "somemod:machine",
        "input": {
            "1": null
        }
    }"#;
    parse_recipe("test:custom", json).error();
}
