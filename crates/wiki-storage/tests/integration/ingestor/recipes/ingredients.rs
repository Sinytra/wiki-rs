use crate::support::{ingredient_error, ingredients, item, tag};

#[test]
fn string_item() {
    // language=json
    let json = r#""minecraft:stone""#;
    assert_eq!(ingredients(json), vec![item("minecraft:stone")]);
}

#[test]
fn string_tag() {
    // language=json
    let json = r##""#minecraft:logs""##;
    assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
}

#[test]
fn string_without_namespace_is_kept_verbatim() {
    // language=json
    let json = r#""stone""#;
    assert_eq!(ingredients(json), vec![item("stone")]);
    // language=json
    let json = r##""#logs""##;
    assert_eq!(ingredients(json), vec![tag("logs")]);
}

#[test]
fn hash_inside_string_does_not_make_a_tag() {
    // language=json
    let json = r#""minecraft:a#b""#;
    assert_eq!(ingredients(json), vec![item("minecraft:a#b")]);
}

#[test]
fn only_leading_hash_is_stripped() {
    // language=json
    let json = r###""##minecraft:logs""###;
    assert_eq!(ingredients(json), vec![tag("#minecraft:logs")]);
}

#[test]
fn object_item() {
    // language=json
    let json = r#"{
        "item": "minecraft:stone"
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:stone")]);
}

#[test]
fn object_tag() {
    // language=json
    let json = r#"{
        "tag": "minecraft:logs"
    }"#;
    assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
}

#[test]
fn object_tag_value_is_not_hash_stripped() {
    // language=json
    let json = r##"{
        "tag": "#minecraft:logs"
    }"##;
    assert_eq!(ingredients(json), vec![tag("#minecraft:logs")]);
}

#[test]
fn object_with_item_and_tag_prefers_item() {
    // language=json
    let json = r#"{
        "item": "minecraft:stone",
        "tag": "minecraft:logs"
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:stone")]);
}

#[test]
fn object_extra_fields_are_ignored() {
    // language=json
    let json = r#"{
        "item": "minecraft:stone",
        "count": 4,
        "nbt": {
            "a": 1
        }
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:stone")]);
}

#[test]
fn object_item_must_be_string() {
    // language=json
    let json = r#"{
        "item": 5
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("string"), "{err}");
}

#[test]
fn object_tag_must_be_string() {
    // language=json
    let json = r#"{
        "tag": ["minecraft:logs"]
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("string"), "{err}");
}

#[test]
fn object_item_null_is_rejected() {
    // language=json
    let json = r#"{
        "item": null
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("string"), "{err}");
}

#[test]
fn empty_object_is_rejected() {
    let err = ingredient_error("{}");
    assert!(err.contains("'item'"), "{err}");
    assert!(err.contains("'tag'"), "{err}");
}

#[test]
fn object_without_known_fields_is_rejected() {
    // language=json
    let json = r#"{
        "id": "minecraft:stone"
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("'item'"), "{err}");
}

#[test]
fn empty_array_yields_no_ingredients() {
    assert_eq!(ingredients("[]"), vec![]);
}

#[test]
fn array_of_strings() {
    // language=json
    let json = r##"["minecraft:stone", "#minecraft:logs"]"##;
    assert_eq!(
        ingredients(json),
        vec![item("minecraft:stone"), tag("minecraft:logs")]
    );
}

#[test]
fn array_of_objects() {
    // language=json
    let json = r#"[
        {
            "item": "minecraft:stone"
        },
        {
            "tag": "minecraft:logs"
        }
    ]"#;
    assert_eq!(
        ingredients(json),
        vec![item("minecraft:stone"), tag("minecraft:logs")]
    );
}

#[test]
fn array_of_mixed_forms() {
    // language=json
    let json = r##"[
        "minecraft:a",
        {
            "item": "minecraft:b"
        },
        "#minecraft:c",
        {
            "tag": "minecraft:d"
        }
    ]"##;
    assert_eq!(
        ingredients(json),
        vec![
            item("minecraft:a"),
            item("minecraft:b"),
            tag("minecraft:c"),
            tag("minecraft:d")
        ]
    );
}

#[test]
fn nested_arrays_are_flattened_in_order() {
    // language=json
    let json = r#"[
        "minecraft:a",
        [
            "minecraft:b",
            ["minecraft:c"]
        ],
        "minecraft:d"
    ]"#;
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
fn nested_empty_arrays_contribute_nothing() {
    // language=json
    let json = r#"[
        [],
        "minecraft:a",
        []
    ]"#;
    assert_eq!(ingredients(json), vec![item("minecraft:a")]);
}

#[test]
fn duplicates_are_preserved() {
    // language=json
    let json = r#"["minecraft:a", "minecraft:a"]"#;
    assert_eq!(
        ingredients(json),
        vec![item("minecraft:a"), item("minecraft:a")]
    );
}

#[test]
fn array_with_invalid_element_is_rejected() {
    // language=json
    let json = r#"["minecraft:a", 5]"#;
    ingredient_error(json);
    // language=json
    let json = r#"[
        "minecraft:a",
        {}
    ]"#;
    ingredient_error(json);
    // language=json
    let json = r#"["minecraft:a", null]"#;
    ingredient_error(json);
}

#[test]
fn scalar_non_strings_are_rejected() {
    ingredient_error("5");
    ingredient_error("true");
    ingredient_error("null");
    ingredient_error("1.5");
}

#[test]
fn non_string_type_key_is_ignored() {
    // language=json
    let json = r#"{
        "type": 5,
        "item": "minecraft:stone"
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:stone")]);
    // language=json
    let json = r#"{
        "type": null,
        "tag": "minecraft:logs"
    }"#;
    assert_eq!(ingredients(json), vec![tag("minecraft:logs")]);
}

#[test]
fn unknown_type_key_value_is_rejected_even_with_item() {
    // language=json
    let json = r#"{
        "type": "somemod:special",
        "item": "minecraft:stone"
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("somemod:special"), "{err}");
}

#[test]
fn unknown_ingredient_type_error_names_the_type() {
    // language=json
    let json = r#"{
        "type": "somemod:special"
    }"#;
    let err = ingredient_error(json);
    assert!(err.contains("unknown custom ingredient type"), "{err}");
    assert!(err.contains("somemod:special"), "{err}");
}

#[test]
fn builtin_type_under_wrong_loader_key_still_resolves() {
    // language=json
    let json = r#"{
        "fabric:type": "neoforge:difference",
        "base": "minecraft:a",
        "subtracted": "minecraft:b"
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:a")]);
    // language=json
    let json = r#"{
        "neoforge:ingredient_type": "fabric:any",
        "ingredients": ["minecraft:a"]
    }"#;
    assert_eq!(ingredients(json), vec![item("minecraft:a")]);
}
