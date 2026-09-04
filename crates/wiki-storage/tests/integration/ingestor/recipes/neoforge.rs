use crate::support::{ingredient_error, ingredients, item, tag};

mod compound {
    use super::*;

    #[test]
    fn children_strings() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": ["#minecraft:planks", "#minecraft:logs", "minecraft:bedrock"]
        }"##;
        assert_eq!(
            ingredients(json),
            vec![
                tag("minecraft:planks"),
                tag("minecraft:logs"),
                item("minecraft:bedrock")
            ]
        );
    }

    #[test]
    fn single_child() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": ["minecraft:a"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn ingredients_alias() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "ingredients": ["minecraft:a", "minecraft:b"]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn both_children_and_ingredients_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": ["minecraft:a"],
            "ingredients": ["minecraft:b"]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn empty_children_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": []
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn children_only_containing_empty_arrays_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": [
                [],
                []
            ]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn missing_children_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("children"), "{err}");
    }

    #[test]
    fn children_must_be_array() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": "minecraft:a"
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn nested_compound() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": [
                "minecraft:a",
                {
                    "neoforge:ingredient_type": "neoforge:compound",
                    "children": ["minecraft:b", "minecraft:c"]
                },
                "minecraft:d"
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![
                item("minecraft:a"),
                item("minecraft:b"),
                item("minecraft:c"),
                item("minecraft:d")
            ]
        );
    }

    #[test]
    fn children_with_nested_arrays() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": [
                ["minecraft:a", "minecraft:b"],
                "minecraft:c"
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![
                item("minecraft:a"),
                item("minecraft:b"),
                item("minecraft:c")
            ]
        );
    }

    #[test]
    fn children_with_components_ingredient() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": [
                "#minecraft:planks",
                "#minecraft:logs",
                {
                    "neoforge:ingredient_type": "neoforge:components",
                    "components": {
                        "minecraft:damage": 3
                    },
                    "items": "minecraft:stone_pickaxe",
                    "strict": true
                }
            ]
        }"##;
        assert_eq!(
            ingredients(json),
            vec![
                tag("minecraft:planks"),
                tag("minecraft:logs"),
                item("minecraft:stone_pickaxe")
            ]
        );
    }

    #[test]
    fn invalid_child_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": [
                "minecraft:a",
                {}
            ]
        }"#;
        ingredient_error(json);
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:compound",
            "children": ["minecraft:a", 1]
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_type_key_with_objects() {
        // language=json
        let json = r#"{
            "type": "neoforge:compound",
            "children": [
                {
                    "tag": "minecraft:planks"
                },
                {
                    "item": "minecraft:bedrock"
                }
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![tag("minecraft:planks"), item("minecraft:bedrock")]
        );
    }
}

mod components {
    use super::*;

    #[test]
    fn single_item() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": "minecraft:stone_pickaxe",
            "components": {
                "minecraft:damage": 3
            }
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:stone_pickaxe")]);
    }

    #[test]
    fn item_tag() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": "#minecraft:swords",
            "components": {}
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:swords")]);
    }

    #[test]
    fn item_list() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": ["minecraft:iron_sword", "minecraft:gold_sword"],
            "components": {}
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:iron_sword"), item("minecraft:gold_sword")]
        );
    }

    #[test]
    fn empty_item_list_yields_nothing() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": [],
            "components": {}
        }"#;
        assert_eq!(ingredients(json), vec![]);
    }

    #[test]
    fn components_and_strict_are_ignored() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": "minecraft:a",
            "strict": true
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": "minecraft:a",
            "components": "garbage",
            "strict": "garbage"
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn missing_items_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "components": {}
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("items"), "{err}");
    }

    #[test]
    fn invalid_items_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": 5
        }"#;
        ingredient_error(json);
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:components",
            "items": {}
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_type_key_with_full_component_map() {
        // language=json
        let json = r##"{
            "type": "neoforge:components",
            "components": {
                "minecraft:attribute_modifiers": {
                    "modifiers": [
                        {
                            "type": "minecraft:generic.attack_damage",
                            "amount": 2.0
                        }
                    ]
                },
                "minecraft:damage": 3,
                "minecraft:lore": [],
                "minecraft:tool": {
                    "rules": [
                        {
                            "blocks": "#minecraft:mineable/pickaxe",
                            "speed": 4.0
                        }
                    ]
                }
            },
            "items": "minecraft:stone_pickaxe",
            "strict": true
        }"##;
        assert_eq!(ingredients(json), vec![item("minecraft:stone_pickaxe")]);
    }
}

mod difference {
    use super::*;

    #[test]
    fn uses_base_only() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": "#minecraft:fences",
            "subtracted": "#minecraft:non_flammable_wood"
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:fences")]);
    }

    #[test]
    fn base_list() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": ["minecraft:a", "minecraft:b"],
            "subtracted": "minecraft:a"
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn base_custom_ingredient() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": {
                "neoforge:ingredient_type": "neoforge:compound",
                "children": ["minecraft:a", "minecraft:b"]
            },
            "subtracted": "minecraft:a"
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn subtracted_is_not_validated() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": "minecraft:a",
            "subtracted": 5
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": "minecraft:a"
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn missing_base_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "subtracted": "minecraft:a"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("base"), "{err}");
    }

    #[test]
    fn invalid_base_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "base": {},
            "subtracted": "minecraft:a"
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_type_key_with_objects() {
        // language=json
        let json = r#"{
            "type": "neoforge:difference",
            "base": {
                "tag": "minecraft:fences"
            },
            "subtracted": {
                "tag": "minecraft:non_flammable_wood"
            }
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:fences")]);
    }
}

mod intersection {
    use super::*;

    #[test]
    fn uses_first_child_only() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": ["#minecraft:planks", "#minecraft:non_flammable_wood"]
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:planks")]);
    }

    #[test]
    fn single_child() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": ["minecraft:a"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn first_child_array_is_kept_whole() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": [
                ["minecraft:a", "minecraft:b"],
                "minecraft:c"
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn first_child_compound_is_kept_whole() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": [
                {
                    "neoforge:ingredient_type": "neoforge:compound",
                    "children": ["minecraft:a", "minecraft:b"]
                },
                "minecraft:c"
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn empty_children_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": []
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn empty_first_child_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": [
                [],
                "minecraft:a"
            ]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn missing_children_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("children"), "{err}");
    }

    #[test]
    fn ingredients_alias_is_not_accepted() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "ingredients": ["minecraft:a"]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("children"), "{err}");
    }

    #[test]
    fn later_invalid_children_are_still_validated() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:intersection",
            "children": [
                "minecraft:a",
                {}
            ]
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_type_key_with_objects() {
        // language=json
        let json = r#"{
            "type": "neoforge:intersection",
            "children": [
                {
                    "tag": "minecraft:planks"
                },
                {
                    "tag": "minecraft:non_flammable_wood"
                }
            ]
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:planks")]);
    }
}

mod block_tag {
    use super::*;

    #[test]
    fn tag_becomes_tag_ingredient() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:block_tag",
            "tag": "minecraft:logs"
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
    }

    #[test]
    fn leading_hash_is_tolerated() {
        // language=json
        let json = r##"{
            "neoforge:ingredient_type": "neoforge:block_tag",
            "tag": "#minecraft:logs"
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
    }

    #[test]
    fn missing_tag_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:block_tag"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("tag"), "{err}");
    }

    #[test]
    fn non_string_tag_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:block_tag",
            "tag": ["minecraft:logs"]
        }"#;
        ingredient_error(json);
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:block_tag",
            "tag": {
                "tag": "minecraft:logs"
            }
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_type_key() {
        // language=json
        let json = r#"{
            "type": "neoforge:block_tag",
            "tag": "minecraft:logs"
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
    }
}

mod custom_display {
    use super::*;

    #[test]
    fn uses_base_and_ignores_display() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:custom_display",
            "base": ["minecraft:stone", "minecraft:dirt"],
            "display": {
                "type": "minecraft:item",
                "item": "minecraft:diamond"
            }
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:stone"), item("minecraft:dirt")]
        );
    }

    #[test]
    fn base_custom_ingredient() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:custom_display",
            "base": {
                "neoforge:ingredient_type": "neoforge:block_tag",
                "tag": "minecraft:logs"
            },
            "display": {
                "type": "minecraft:tag",
                "tag": "minecraft:logs"
            }
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
    }

    #[test]
    fn missing_base_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:custom_display",
            "display": {
                "type": "minecraft:empty"
            }
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("base"), "{err}");
    }
}

mod type_key {
    use super::*;

    #[test]
    fn ingredient_type_key_wins_over_type() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "type": "neoforge:compound",
            "base": "minecraft:a",
            "subtracted": "minecraft:b"
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn ingredient_type_key_wins_over_fabric_type() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:difference",
            "fabric:type": "fabric:any",
            "base": "minecraft:a",
            "ingredients": ["minecraft:b"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn unknown_neoforge_type_is_rejected() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:sized",
            "count": 3,
            "ingredient": "minecraft:a"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("neoforge:sized"), "{err}");
    }

    #[test]
    fn type_id_is_case_sensitive() {
        // language=json
        let json = r#"{
            "neoforge:ingredient_type": "neoforge:Compound",
            "children": ["minecraft:a"]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("neoforge:Compound"), "{err}");
    }
}
