use crate::support::{ingredient_error, ingredients, item, tag};

mod any {
    use super::*;

    #[test]
    fn flattens_all_ingredients() {
        // language=json
        let json = r##"{
            "fabric:type": "fabric:any",
            "ingredients": ["minecraft:a", "#minecraft:b"]
        }"##;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), tag("minecraft:b")]
        );
    }

    #[test]
    fn single_ingredient() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": ["minecraft:a"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn nested_arrays_and_custom_ingredients() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": [
                "minecraft:a",
                ["minecraft:b", "minecraft:c"],
                {
                    "fabric:type": "fabric:components",
                    "base": "minecraft:d",
                    "components": {
                        "minecraft:damage": 1
                    }
                },
                {
                    "fabric:type": "fabric:any",
                    "ingredients": ["minecraft:e"]
                }
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![
                item("minecraft:a"),
                item("minecraft:b"),
                item("minecraft:c"),
                item("minecraft:d"),
                item("minecraft:e")
            ]
        );
    }

    #[test]
    fn empty_ingredients_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": []
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn missing_ingredients_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("ingredients"), "{err}");
    }

    #[test]
    fn children_alias_is_not_accepted() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "children": ["minecraft:a"]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("ingredients"), "{err}");
    }

    #[test]
    fn ingredients_must_be_array() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": "minecraft:a"
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn invalid_entry_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": [
                "minecraft:a",
                {
                    "nope": 1
                }
            ]
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_format_with_objects() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:any",
            "ingredients": [
                {
                    "item": "minecraft:a"
                },
                {
                    "tag": "minecraft:b"
                }
            ]
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), tag("minecraft:b")]
        );
    }
}

mod all {
    use super::*;

    #[test]
    fn uses_first_ingredient_only() {
        // language=json
        let json = r##"{
            "fabric:type": "fabric:all",
            "ingredients": ["#minecraft:a", "#minecraft:b"]
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:a")]);
    }

    #[test]
    fn single_ingredient() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": ["minecraft:a"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn first_ingredient_array_is_kept_whole() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": [
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
    fn first_ingredient_any_is_kept_whole() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": [
                {
                    "fabric:type": "fabric:any",
                    "ingredients": ["minecraft:a", "minecraft:b"]
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
    fn empty_ingredients_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": []
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn empty_first_ingredient_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": [
                [],
                "minecraft:a"
            ]
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("at least one child"), "{err}");
    }

    #[test]
    fn missing_ingredients_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("ingredients"), "{err}");
    }

    #[test]
    fn later_invalid_ingredients_are_still_validated() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": ["minecraft:a", 5]
        }"#;
        ingredient_error(json);
    }

    #[test]
    fn legacy_format_with_objects() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:all",
            "ingredients": [
                {
                    "tag": "minecraft:a"
                },
                {
                    "tag": "minecraft:b"
                }
            ]
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:a")]);
    }
}

mod components {
    use super::*;

    #[test]
    fn uses_base() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:components",
            "base": "minecraft:a",
            "components": {
                "minecraft:damage": 1
            }
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn base_list_and_tag() {
        // language=json
        let json = r##"{
            "fabric:type": "fabric:components",
            "base": ["minecraft:a", "#minecraft:b"],
            "components": {}
        }"##;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), tag("minecraft:b")]
        );
    }

    #[test]
    fn components_are_not_validated() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:components",
            "base": "minecraft:a"
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
        // language=json
        let json = r#"{
            "fabric:type": "fabric:components",
            "base": "minecraft:a",
            "components": 5
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn missing_base_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:components",
            "components": {}
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("base"), "{err}");
    }

    #[test]
    fn legacy_format_with_object_base() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:components",
            "base": {
                "item": "minecraft:a"
            },
            "components": {
                "minecraft:damage": 1
            }
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }
}

mod custom_data {
    use super::*;

    #[test]
    fn uses_base_and_ignores_nbt_string() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:custom_data",
            "base": "minecraft:a",
            "nbt": "{foo:1b}"
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn ignores_nbt_object() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:custom_data",
            "base": "minecraft:a",
            "nbt": {
                "foo": 1
            }
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn missing_base_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:custom_data",
            "nbt": "{}"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("base"), "{err}");
    }

    #[test]
    fn legacy_format_with_object_base() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:custom_data",
            "base": {
                "tag": "minecraft:a"
            },
            "nbt": "{foo:1b}"
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:a")]);
    }
}

mod difference {
    use super::*;

    #[test]
    fn uses_base_only() {
        // language=json
        let json = r##"{
            "fabric:type": "fabric:difference",
            "base": "#minecraft:a",
            "subtracted": "#minecraft:b"
        }"##;
        assert_eq!(ingredients(json), vec![tag("minecraft:a")]);
    }

    #[test]
    fn base_custom_ingredient() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:difference",
            "base": {
                "fabric:type": "fabric:any",
                "ingredients": ["minecraft:a", "minecraft:b"]
            },
            "subtracted": "minecraft:b"
        }"#;
        assert_eq!(
            ingredients(json),
            vec![item("minecraft:a"), item("minecraft:b")]
        );
    }

    #[test]
    fn missing_base_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:difference",
            "subtracted": "minecraft:a"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("base"), "{err}");
    }

    #[test]
    fn legacy_format_with_objects() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:difference",
            "base": {
                "tag": "minecraft:a"
            },
            "subtracted": {
                "tag": "minecraft:b"
            }
        }"#;
        assert_eq!(ingredients(json), vec![tag("minecraft:a")]);
    }
}

mod type_key {
    use super::*;

    #[test]
    fn fabric_type_key_wins_over_type() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:difference",
            "type": "fabric:any",
            "base": "minecraft:a",
            "ingredients": ["minecraft:b"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn fabric_types_also_resolve_under_plain_type_key() {
        // language=json
        let json = r#"{
            "type": "fabric:any",
            "ingredients": ["minecraft:a"]
        }"#;
        assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    }

    #[test]
    fn unknown_fabric_type_is_rejected() {
        // language=json
        let json = r#"{
            "fabric:type": "fabric:nbt",
            "base": "minecraft:a"
        }"#;
        let err = ingredient_error(json);
        assert!(err.contains("fabric:nbt"), "{err}");
    }
}
