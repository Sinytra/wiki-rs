use wiki_domain::error::ProjectError;
use wiki_storage::ingestor::recipes::parser::RecipeParseError;

use crate::support::{input, input_tag, output, parse_fixture_recipe, parse_recipe};

mod shaped {
    use super::*;

    #[test]
    fn full_grid_slot_indices() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": "minecraft:a",
                "B": "minecraft:b",
                "C": "minecraft:c"
            },
            "pattern": ["ABC", "CAB", "BCA"],
            "result": {
                "id": "minecraft:out",
                "count": 3
            }
        }"#;
        let recipe = parse_recipe("test:full", json).recipe();

        assert_eq!(recipe.id, "test:full");
        assert_eq!(recipe.r#type, "minecraft:crafting_shaped");
        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input("minecraft:b", "2"),
                input("minecraft:c", "3"),
                input("minecraft:c", "4"),
                input("minecraft:a", "5"),
                input("minecraft:b", "6"),
                input("minecraft:b", "7"),
                input("minecraft:c", "8"),
                input("minecraft:a", "9"),
                output("minecraft:out", "1", 3),
            ]
        );
    }

    #[test]
    fn short_rows_still_use_three_wide_slot_math() {
        // language=json
        let json = r##"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "#": "minecraft:a"
            },
            "pattern": ["#", "#"],
            "result": {
                "id": "minecraft:out"
            }
        }"##;
        let recipe = parse_recipe("test:short", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input("minecraft:a", "4"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn spaces_are_empty_slots() {
        // language=json
        let json = r####"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "#": "minecraft:a"
            },
            "pattern": [" # ", "###", " # "],
            "result": "minecraft:out"
        }"####;
        let recipe = parse_recipe("test:spaces", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "2"),
                input("minecraft:a", "4"),
                input("minecraft:a", "5"),
                input("minecraft:a", "6"),
                input("minecraft:a", "8"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn pattern_char_missing_from_key_is_skipped() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": "minecraft:a"
            },
            "pattern": ["AX"],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:missing_key", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![input("minecraft:a", "1"), output("minecraft:out", "1", 1),]
        );
    }

    #[test]
    fn unused_key_entries_are_ignored() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": "minecraft:a",
                "Z": "minecraft:z"
            },
            "pattern": ["A"],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:unused_key", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![input("minecraft:a", "1"), output("minecraft:out", "1", 1),]
        );
    }

    #[test]
    fn key_array_expands_into_same_slot() {
        // language=json
        let json = r##"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": ["minecraft:a", "#minecraft:b"]
            },
            "pattern": ["A", "A"],
            "result": "minecraft:out"
        }"##;
        let recipe = parse_recipe("test:key_array", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input_tag("minecraft:b", "1"),
                input("minecraft:a", "4"),
                input_tag("minecraft:b", "4"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn key_legacy_objects() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": {
                    "item": "minecraft:a"
                },
                "B": {
                    "tag": "minecraft:b"
                }
            },
            "pattern": ["AB"],
            "result": {
                "id": "minecraft:out",
                "count": 2
            }
        }"#;
        let recipe = parse_recipe("test:legacy", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input_tag("minecraft:b", "2"),
                output("minecraft:out", "1", 2),
            ]
        );
    }

    #[test]
    fn key_with_neoforge_and_fabric_ingredients() {
        // language=json
        let json = r##"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "N": {
                    "neoforge:ingredient_type": "neoforge:compound",
                    "children": ["#minecraft:planks", "minecraft:bedrock"]
                },
                "F": {
                    "fabric:type": "fabric:difference",
                    "base": "minecraft:a",
                    "subtracted": "minecraft:b"
                }
            },
            "pattern": ["NF"],
            "result": "minecraft:out"
        }"##;
        let recipe = parse_recipe("test:loaders", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input_tag("minecraft:planks", "1"),
                input("minecraft:bedrock", "1"),
                input("minecraft:a", "2"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn result_forms() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "pattern": [],
            "result": "minecraft:out"
        }"#;
        let string_form = parse_recipe("test:r1", json).recipe();
        assert_eq!(
            string_form.ingredients,
            vec![output("minecraft:out", "1", 1)]
        );

        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "pattern": [],
            "result": {
                "id": "minecraft:out"
            }
        }"#;
        let object_no_count = parse_recipe("test:r2", json).recipe();
        assert_eq!(
            object_no_count.ingredients,
            vec![output("minecraft:out", "1", 1)]
        );

        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "pattern": [],
            "result": {
                "id": "minecraft:out",
                "count": 64
            }
        }"#;
        let object_count = parse_recipe("test:r3", json).recipe();
        assert_eq!(
            object_count.ingredients,
            vec![output("minecraft:out", "1", 64)]
        );
    }

    #[test]
    fn result_with_components_is_accepted() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "pattern": [],
            "result": {
                "id": "minecraft:out",
                "count": 1,
                "components": {
                    "minecraft:damage": 5
                }
            }
        }"#;
        let recipe = parse_recipe("test:components", json).recipe();
        assert_eq!(recipe.ingredients, vec![output("minecraft:out", "1", 1)]);
    }

    #[test]
    fn extra_top_level_fields_are_ignored() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "category": "building",
            "group": "stuff",
            "show_notification": false,
            "key": {
                "A": "minecraft:a"
            },
            "pattern": ["A"],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:extra", json).recipe();
        assert_eq!(recipe.ingredients.len(), 2);
    }

    #[test]
    fn missing_pattern_is_an_error_naming_the_field() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(matches!(err, RecipeParseError::InvalidJsonPath(_)), "{err}");
        assert!(err.to_string().contains("pattern"), "{err}");
    }

    #[test]
    fn missing_result_is_an_error() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {},
            "pattern": []
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(err.to_string().contains("result"), "{err}");
    }

    #[test]
    fn invalid_key_ingredient_error_names_the_key() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": "minecraft:a",
                "B": {
                    "nope": 1
                }
            },
            "pattern": ["AB"],
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        let msg = err.to_string();
        assert!(msg.contains("key"), "{msg}");
        assert!(msg.contains("'item'"), "{msg}");
    }

    #[test]
    fn unknown_custom_ingredient_type_fails_the_recipe() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shaped",
            "key": {
                "A": {
                    "neoforge:ingredient_type": "somemod:thing"
                }
            },
            "pattern": ["A"],
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(err.to_string().contains("somemod:thing"), "{err}");
    }
}

mod shapeless {
    use super::*;

    #[test]
    fn slot_per_ingredient_index() {
        // language=json
        let json = r##"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                "minecraft:a",
                "#minecraft:b",
                {
                    "item": "minecraft:c"
                }
            ],
            "result": {
                "id": "minecraft:out",
                "count": 4
            }
        }"##;
        let recipe = parse_recipe("test:shapeless", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input_tag("minecraft:b", "2"),
                input("minecraft:c", "3"),
                output("minecraft:out", "1", 4),
            ]
        );
    }

    #[test]
    fn nested_array_expands_into_one_slot() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                ["minecraft:a", "minecraft:b"],
                "minecraft:c"
            ],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:shapeless", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input("minecraft:b", "1"),
                input("minecraft:c", "2"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn custom_ingredient_expands_into_one_slot() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                {
                    "fabric:type": "fabric:any",
                    "ingredients": ["minecraft:a", "minecraft:b"]
                },
                {
                    "neoforge:ingredient_type": "neoforge:intersection",
                    "children": ["minecraft:c", "minecraft:d"]
                }
            ],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:shapeless", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:a", "1"),
                input("minecraft:b", "1"),
                input("minecraft:c", "2"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn empty_ingredient_list() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": [],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:shapeless", json).recipe();
        assert_eq!(recipe.ingredients, vec![output("minecraft:out", "1", 1)]);
    }

    #[test]
    fn ingredients_must_be_an_array() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": "minecraft:a",
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(err.to_string().contains("ingredients"), "{err}");
    }
}

mod single_ingredient {
    use super::*;

    const TYPES: [&str; 5] = [
        "minecraft:smelting",
        "minecraft:blasting",
        "minecraft:smoking",
        "minecraft:campfire_cooking",
        "minecraft:stonecutting",
    ];

    #[test]
    fn all_types_use_slot_one() {
        // language=json
        let template = r#"{
            "type": "$TYPE",
            "ingredient": "minecraft:a",
            "result": {
                "id": "minecraft:out",
                "count": 2
            },
            "experience": 0.7,
            "cookingtime": 200
        }"#;
        for ty in TYPES {
            let json = template.replace("$TYPE", ty);
            let recipe = parse_recipe("test:single", &json).recipe();

            assert_eq!(recipe.r#type, ty);
            assert_eq!(
                recipe.ingredients,
                vec![input("minecraft:a", "1"), output("minecraft:out", "1", 2)],
                "type {ty}"
            );
        }
    }

    #[test]
    fn ingredient_array_and_custom_types() {
        // language=json
        let json = r##"{
            "type": "minecraft:smelting",
            "ingredient": [
                "#minecraft:a",
                {
                    "neoforge:ingredient_type": "neoforge:block_tag",
                    "tag": "minecraft:b"
                }
            ],
            "result": "minecraft:out"
        }"##;
        let recipe = parse_recipe("test:single", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input_tag("minecraft:a", "1"),
                input_tag("minecraft:b", "1"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn missing_ingredient_is_an_error() {
        // language=json
        let json = r#"{
            "type": "minecraft:blasting",
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(err.to_string().contains("ingredient"), "{err}");
    }
}

mod smithing_transform {
    use super::*;

    #[test]
    fn template_base_addition_slots() {
        // language=json
        let json = r##"{
            "type": "minecraft:smithing_transform",
            "template": "minecraft:template",
            "base": ["minecraft:base_a", "minecraft:base_b"],
            "addition": "#minecraft:addition",
            "result": {
                "id": "minecraft:out"
            }
        }"##;
        let recipe = parse_recipe("test:smith", json).recipe();

        assert_eq!(
            recipe.ingredients,
            vec![
                input("minecraft:template", "0"),
                input("minecraft:base_a", "1"),
                input("minecraft:base_b", "1"),
                input_tag("minecraft:addition", "2"),
                output("minecraft:out", "1", 1),
            ]
        );
    }

    #[test]
    fn missing_addition_is_an_error() {
        // language=json
        let json = r#"{
            "type": "minecraft:smithing_transform",
            "template": "minecraft:t",
            "base": "minecraft:b",
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(err.to_string().contains("addition"), "{err}");
    }
}

mod dispatch {
    use super::*;

    #[test]
    fn missing_type() {
        // language=json
        let json = r#"{
            "ingredients": [],
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(matches!(err, RecipeParseError::MissingType), "{err}");
    }

    #[test]
    fn null_type_counts_as_missing() {
        // language=json
        let json = r#"{
            "type": null,
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(matches!(err, RecipeParseError::MissingType), "{err}");
    }

    #[test]
    fn non_string_type_is_invalid_json() {
        // language=json
        let json = r#"{
            "type": 5,
            "result": "minecraft:out"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(matches!(err, RecipeParseError::InvalidJson(_)), "{err}");
    }

    #[test]
    fn unknown_vanilla_type() {
        // language=json
        let json = r#"{
            "type": "minecraft:smithing_trim",
            "template": "minecraft:t",
            "base": "minecraft:b",
            "addition": "minecraft:a"
        }"#;
        let err = parse_recipe("test:bad", json).error();
        match err {
            RecipeParseError::UnknownRecipeType(t) => assert_eq!(t, "minecraft:smithing_trim"),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn unknown_vanilla_type_is_not_treated_as_custom() {
        // language=json
        let json = r#"{
            "type": "minecraft:whatever",
            "input": {
                "1": "minecraft:a"
            },
            "output": {
                "1": "minecraft:b"
            }
        }"#;
        let err = parse_recipe("test:bad", json).error();
        assert!(
            matches!(err, RecipeParseError::UnknownRecipeType(_)),
            "{err}"
        );
    }

    #[test]
    fn type_without_namespace_defaults_to_minecraft() {
        // language=json
        let json = r#"{
            "type": "crafting_shapeless",
            "ingredients": ["minecraft:a"],
            "result": "minecraft:out"
        }"#;
        let recipe = parse_recipe("test:ns", json).result;
        assert!(
            matches!(recipe, Err(RecipeParseError::UnknownRecipeType(ref t)) if t == "crafting_shapeless"),
            "{recipe:?}"
        );
    }

    #[test]
    fn invalid_type_resloc_is_skipped_with_issue() {
        // language=json
        let json = r#"{
            "type": "Minecraft:Crafting_Shaped",
            "key": {},
            "pattern": [],
            "result": "minecraft:out"
        }"#;
        let parsed = parse_recipe("test:bad", json);
        assert_eq!(parsed.subjects(), vec![ProjectError::InvalidResloc]);
        let issues = parsed.skipped();
        assert_eq!(
            issues[0].details.as_deref(),
            Some("Minecraft:Crafting_Shaped")
        );
    }

    #[test]
    fn empty_type_is_skipped_with_issue() {
        // language=json
        let json = r#"{
            "type": "",
            "result": "minecraft:out"
        }"#;
        let parsed = parse_recipe("test:bad", json);
        assert_eq!(parsed.subjects(), vec![ProjectError::InvalidResloc]);
        parsed.skipped();
    }

    #[test]
    fn successful_parse_emits_no_issues() {
        // language=json
        let json = r#"{
            "type": "minecraft:crafting_shapeless",
            "ingredients": ["minecraft:a"],
            "result": "minecraft:out"
        }"#;
        let parsed = parse_recipe("test:ok", json);
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        parsed.recipe();
    }
}

mod neoforge_fixtures {
    use super::*;

    fn both_versions(name: &str) -> [(&'static str, crate::support::ParsedRecipe); 2] {
        [
            (
                "1.21.1",
                parse_fixture_recipe(&format!("recipes/neoforge/1.21.1/{name}.json")),
            ),
            (
                "1.21.x",
                parse_fixture_recipe(&format!("recipes/neoforge/1.21.x/{name}.json")),
            ),
        ]
    }

    #[test]
    fn compound_ingredient_only_vanilla() {
        for (version, parsed) in both_versions("compound_ingredient_only_vanilla") {
            let recipe = parsed.recipe();
            assert_eq!(
                recipe.ingredients,
                vec![
                    input_tag("minecraft:planks", "1"),
                    input_tag("minecraft:logs", "1"),
                    input("minecraft:bedrock", "1"),
                    input_tag("minecraft:planks", "2"),
                    input_tag("minecraft:logs", "2"),
                    input("minecraft:bedrock", "2"),
                    input_tag("minecraft:planks", "3"),
                    input_tag("minecraft:logs", "3"),
                    input("minecraft:bedrock", "3"),
                    input_tag("minecraft:planks", "5"),
                    input_tag("minecraft:logs", "5"),
                    input("minecraft:bedrock", "5"),
                    output("minecraft:dirt", "1", 1),
                ],
                "version {version}"
            );
        }
    }

    #[test]
    fn compound_ingredient_custom_types() {
        for (version, parsed) in both_versions("compound_ingredient_custom_types") {
            let recipe = parsed.recipe();
            assert_eq!(
                recipe.ingredients,
                vec![
                    input_tag("minecraft:planks", "1"),
                    input_tag("minecraft:logs", "1"),
                    input("minecraft:stone_pickaxe", "1"),
                    input_tag("minecraft:planks", "4"),
                    input_tag("minecraft:logs", "4"),
                    input("minecraft:stone_pickaxe", "4"),
                    output("minecraft:gold_block", "1", 1),
                ],
                "version {version}"
            );
        }
    }

    #[test]
    fn difference_ingredient() {
        for (version, parsed) in both_versions("difference_ingredient") {
            let recipe = parsed.recipe();
            assert_eq!(
                recipe.ingredients,
                vec![
                    input_tag("minecraft:fences", "2"),
                    input_tag("minecraft:fences", "4"),
                    input_tag("minecraft:fences", "5"),
                    input_tag("minecraft:fences", "6"),
                    input_tag("minecraft:fences", "8"),
                    output("minecraft:flint_and_steel", "1", 1),
                ],
                "version {version}"
            );
        }
    }

    #[test]
    fn intersection_ingredient() {
        for (version, parsed) in both_versions("intersection_ingredient") {
            let recipe = parsed.recipe();
            assert_eq!(
                recipe.ingredients,
                vec![
                    input_tag("minecraft:planks", "1"),
                    input_tag("minecraft:planks", "2"),
                    input_tag("minecraft:planks", "3"),
                    input_tag("minecraft:planks", "4"),
                    input_tag("minecraft:planks", "5"),
                    input_tag("minecraft:planks", "6"),
                    input_tag("minecraft:planks", "8"),
                    output("minecraft:netherrack", "1", 1),
                ],
                "version {version}"
            );
        }
    }
}
