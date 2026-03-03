#![doc = include_str!("../README.md")]

mod builder;
mod draft;
mod regex;

pub use crate::draft::{Draft, detect_draft};
pub use crate::regex::normalize_ecma_regex;
pub use jsonschema_schema::Schema;

/// Migrate a JSON Schema document to draft 2020-12, returning a typed [`Schema`].
///
/// Applies all necessary keyword transformations for drafts 04 through 2019-09,
/// then deserializes the result into a strongly-typed `Schema`.
/// Safe to call on schemas that are already 2020-12 (idempotent).
///
/// # Errors
///
/// Returns an error if the input cannot be deserialized into a `Schema`.
pub fn migrate(mut schema: serde_json::Value) -> Result<Schema, serde_json::Error> {
    migrate_to_2020_12(&mut schema);
    serde_json::from_value(schema)
}

/// Migrate a JSON Schema document to draft 2020-12 in-place.
///
/// Applies all necessary keyword transformations for drafts 04 through 2019-09.
/// Safe to call on schemas that are already 2020-12 (idempotent).
pub fn migrate_to_2020_12(schema: &mut serde_json::Value) {
    let draft = draft::detect_draft(schema);
    let serde_json::Value::Object(obj) = schema else {
        return;
    };
    obj.insert(
        "$schema".to_string(),
        serde_json::Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
    );
    builder::migrate_in_place(obj, draft);

    // Post-pass: flatten nested definition containers and rewrite $ref paths.
    // This runs on the entire tree so refs and defs stay in sync.
    flatten::flatten_nested_defs(schema, draft);
}

mod flatten {
    use serde_json::{Map, Value};

    use crate::builder;

    /// Schema keywords that indicate an object is a real schema, not a
    /// namespace container.
    const SCHEMA_KEYWORDS: &[&str] = &[
        "type",
        "properties",
        "$ref",
        "allOf",
        "oneOf",
        "anyOf",
        "items",
        "required",
        "enum",
        "not",
        "if",
        "then",
        "else",
        "patternProperties",
        "additionalProperties",
    ];

    /// Flatten nested definition containers in `$defs` and rewrite `$ref` paths.
    ///
    /// Some schemas store definitions in a nested namespace structure like:
    /// ```json
    /// { "$defs": { "schemas": { "pattern-a": {...}, "format": {...} } } }
    /// ```
    /// These can't be deserialized because the container object itself isn't a
    /// valid schema (field-name collisions, e.g. `format` as a schema vs string).
    ///
    /// This function:
    /// 1. Finds nested containers (objects with no schema keywords where all values
    ///    are schemas)
    /// 2. Builds ref-rewrite mappings (e.g. `/$defs/schemas/format` → `/$defs/format`)
    /// 3. Rewrites all `$ref` values across the tree
    /// 4. Promotes the entries from containers into the parent `$defs`
    pub fn flatten_nested_defs(value: &mut Value, draft: Option<crate::draft::Draft>) {
        // Collect ref rewrites from ALL levels first
        let mut rewrites: Vec<(String, String)> = Vec::new();
        collect_rewrites(value, &mut rewrites);

        if rewrites.is_empty() {
            return;
        }

        // Rewrite all $ref values
        rewrite_all_refs(value, &rewrites);

        // Actually flatten the containers
        apply_flatten(value, draft);
    }

    /// Walk the tree, find nested containers in `$defs`, and record ref rewrites.
    fn collect_rewrites(value: &Value, rewrites: &mut Vec<(String, String)>) {
        let Value::Object(obj) = value else { return };

        if let Some(Value::Object(defs)) = obj.get("$defs") {
            for (key, val) in defs {
                if is_nested_defs_map(val) {
                    // This container will be flattened: /$defs/{key}/{child} → /$defs/{child}
                    if let Value::Object(inner) = val {
                        for child_key in inner.keys() {
                            rewrites.push((
                                format!("/$defs/{key}/{child_key}"),
                                format!("/$defs/{child_key}"),
                            ));
                        }
                    }
                }
            }
        }

        // Recurse into all nested objects
        for v in obj.values() {
            match v {
                Value::Object(_) => collect_rewrites(v, rewrites),
                Value::Array(arr) => {
                    for item in arr {
                        collect_rewrites(item, rewrites);
                    }
                }
                _ => {}
            }
        }
    }

    /// Rewrite all `$ref` values in the tree using the collected rewrites.
    fn rewrite_all_refs(value: &mut Value, rewrites: &[(String, String)]) {
        let Value::Object(obj) = value else { return };

        if let Some(Value::String(ref_str)) = obj.get_mut("$ref") {
            for (old, new) in rewrites {
                // Match fragment refs like #/$defs/schemas/format
                let old_frag = format!("#{old}");
                let new_frag = format!("#{new}");
                if ref_str.contains(&old_frag) {
                    *ref_str = ref_str.replace(&old_frag, &new_frag);
                }
            }
        }

        for v in obj.values_mut() {
            match v {
                Value::Object(_) => rewrite_all_refs(v, rewrites),
                Value::Array(arr) => {
                    for item in arr {
                        rewrite_all_refs(item, rewrites);
                    }
                }
                _ => {}
            }
        }
    }

    /// Walk the tree and actually flatten nested containers in `$defs`.
    /// After promoting entries, run `migrate_in_place` on them so they get
    /// the full cleanup treatment (the initial migration couldn't recurse into
    /// unrecognized schema positions inside the container).
    fn apply_flatten(value: &mut Value, draft: Option<crate::draft::Draft>) {
        let Value::Object(obj) = value else { return };

        if let Some(Value::Object(defs)) = obj.get_mut("$defs") {
            let nested_keys: Vec<String> = defs
                .iter()
                .filter(|(_, v)| is_nested_defs_map(v))
                .map(|(k, _)| k.clone())
                .collect();

            let mut to_merge = Map::new();
            for key in &nested_keys {
                if let Some(Value::Object(inner)) = defs.remove(key) {
                    for (k, v) in inner {
                        to_merge.entry(k).or_insert(v);
                    }
                }
            }
            for (k, mut v) in to_merge {
                // Run migration on newly-promoted entries
                if let Value::Object(entry) = &mut v {
                    builder::migrate_in_place(entry, draft);
                }
                defs.entry(k).or_insert(v);
            }
        }

        // Recurse
        for v in obj.values_mut() {
            match v {
                Value::Object(_) => apply_flatten(v, draft),
                Value::Array(arr) => {
                    for item in arr {
                        apply_flatten(item, draft);
                    }
                }
                _ => {}
            }
        }
    }

    /// Check if a value looks like a nested definitions map — an object with no
    /// schema keywords where all values are objects/bools (i.e., each entry is a
    /// schema definition, not data). Requires at least 2 entries to avoid false
    /// positives on single-entry objects.
    fn is_nested_defs_map(value: &Value) -> bool {
        let Value::Object(map) = value else {
            return false;
        };
        if map.len() < 2 {
            return false;
        }
        let has_schema_kw = SCHEMA_KEYWORDS.iter().any(|kw| map.contains_key(*kw));
        if has_schema_kw {
            return false;
        }
        map.values()
            .all(|v| matches!(v, Value::Object(_) | Value::Bool(_)))
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn flattens_and_rewrites_refs() {
            let mut schema = json!({
                "$defs": {
                    "schemas": {
                        "format": {"type": "string", "example": "epoch"},
                        "request-pattern": {"type": "object"}
                    }
                },
                "properties": {
                    "x": {"$ref": "#/$defs/schemas/request-pattern"},
                    "y": {"$ref": "#/$defs/schemas/format"}
                }
            });
            flatten_nested_defs(&mut schema, None);

            // Container removed, entries promoted
            let defs = schema["$defs"].as_object().unwrap();
            assert!(!defs.contains_key("schemas"));
            assert!(defs.contains_key("format"));
            assert!(defs.contains_key("request-pattern"));

            // Refs rewritten
            assert_eq!(schema["properties"]["x"]["$ref"], "#/$defs/request-pattern");
            assert_eq!(schema["properties"]["y"]["$ref"], "#/$defs/format");
        }

        #[test]
        fn does_not_flatten_schema_like_objects() {
            let mut schema = json!({
                "$defs": {
                    "Real": {"type": "object", "properties": {"x": {"type": "string"}}}
                }
            });
            flatten_nested_defs(&mut schema, None);
            let defs = schema["$defs"].as_object().unwrap();
            assert!(defs.contains_key("Real"));
        }

        #[test]
        fn does_not_flatten_single_entry_object() {
            let mut schema = json!({
                "$defs": {
                    "inner": {
                        "only-one": {"type": "string"}
                    }
                }
            });
            flatten_nested_defs(&mut schema, None);
            let defs = schema["$defs"].as_object().unwrap();
            assert!(defs.contains_key("inner"));
        }

        #[test]
        fn noop_when_no_nested_containers() {
            let mut schema = json!({
                "$defs": {
                    "Foo": {"type": "string"},
                    "Bar": {"type": "number"}
                }
            });
            let original = schema.clone();
            flatten_nested_defs(&mut schema, None);
            assert_eq!(schema, original);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn sets_schema_at_root() {
        let mut schema = json!({"type": "object"});
        migrate_to_2020_12(&mut schema);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn renames_definitions_to_defs_top_level() {
        let mut schema = json!({
            "definitions": {
                "Foo": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("definitions").is_none());
        assert_eq!(schema["$defs"]["Foo"]["type"], "string");
    }

    #[test]
    fn renames_definitions_to_defs_nested() {
        let mut schema = json!({
            "properties": {
                "nested": {
                    "definitions": {
                        "Bar": {"type": "number"}
                    }
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"]["nested"].get("definitions").is_none());
        assert_eq!(
            schema["properties"]["nested"]["$defs"]["Bar"]["type"],
            "number"
        );
    }

    #[test]
    fn does_not_rename_id_inside_properties() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "record ID"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"].get("id").is_some());
    }

    #[test]
    fn array_items_becomes_prefix_items() {
        let mut schema = json!({
            "items": [
                {"type": "string"},
                {"type": "number"}
            ]
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("items").is_none());
        assert_eq!(schema["prefixItems"][0]["type"], "string");
        assert_eq!(schema["prefixItems"][1]["type"], "number");
    }

    #[test]
    fn additional_items_becomes_items_with_tuple() {
        let mut schema = json!({
            "items": [
                {"type": "string"}
            ],
            "additionalItems": {"type": "number"}
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("additionalItems").is_none());
        assert_eq!(schema["prefixItems"][0]["type"], "string");
        assert_eq!(schema["items"]["type"], "number");
    }

    #[test]
    fn dependencies_property_not_migrated() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "dependencies": {
                    "type": "object",
                    "description": "Cargo dependencies",
                    "additionalProperties": {"type": "string"}
                },
                "dev-dependencies": {
                    "type": "object"
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"].get("dependencies").is_some());
        assert!(schema["properties"].get("dependentSchemas").is_none());
    }

    #[test]
    fn string_deprecated_becomes_bool() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "old_field": {
                    "type": "string",
                    "deprecated": "Use new_field instead."
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["properties"]["old_field"]["deprecated"], json!(true));
    }

    #[test]
    fn bool_deprecated_unchanged() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "old_field": {
                    "type": "string",
                    "deprecated": true
                }
            }
        });
        let expected = schema.clone();
        migrate_to_2020_12(&mut schema);
        assert_eq!(
            schema["properties"]["old_field"]["deprecated"],
            expected["properties"]["old_field"]["deprecated"]
        );
    }

    #[test]
    fn null_description_removed() {
        let mut schema = json!({
            "$defs": {
                "Entry": {
                    "type": "object",
                    "description": null,
                    "title": null
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["$defs"]["Entry"].get("description").is_none());
        assert!(schema["$defs"]["Entry"].get("title").is_none());
    }

    #[test]
    fn regex_pattern_normalized() {
        let mut schema = json!({
            "type": "string",
            "pattern": r"^{?[a-zA-Z0-9]+}?$"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["pattern"], r"^\{?[a-zA-Z0-9]+\}?$");
    }

    #[test]
    fn pattern_properties_keys_normalized() {
        let mut schema = json!({
            "type": "object",
            "patternProperties": {
                "^{[a-z]+}$": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["patternProperties"].get(r"^\{[a-z]+\}$").is_some());
        assert!(schema["patternProperties"].get("^{[a-z]+}$").is_none());
    }

    #[test]
    fn schema_fragment_stripped() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object"
        });
        migrate_to_2020_12(&mut schema);
        // After migration $schema is replaced with 2020-12 (no fragment).
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn nested_schema_fragment_stripped() {
        let mut schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": {
                "Sub": {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "string"
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        // The nested $schema fragment should also be stripped.
        assert_eq!(
            schema["$defs"]["Sub"]["$schema"],
            "http://json-schema.org/draft-07/schema"
        );
    }

    #[test]
    fn already_2020_12_is_idempotent() {
        let original = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.com/test",
            "type": "object",
            "$defs": {
                "Foo": {"type": "string"}
            },
            "properties": {
                "x": {"$ref": "#/$defs/Foo"}
            }
        });
        let mut schema = original.clone();
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema, original);
    }

    #[test]
    fn already_2020_12_preserves_dependencies_property() {
        let original = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "dependencies": {
                    "type": "object",
                    "additionalProperties": {"type": "string"}
                }
            }
        });
        let mut schema = original.clone();
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"].get("dependencies").is_some());
    }

    #[test]
    fn properties_without_type_gets_type_object() {
        let mut schema = json!({
            "allOf": [
                {
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "required": ["name"]
                }
            ]
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["allOf"][0]["type"], "object");
    }

    #[test]
    fn properties_with_type_unchanged() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            }
        });
        let original = schema.clone();
        migrate_to_2020_12(&mut schema);
        // type was already present, should not be duplicated or changed
        assert_eq!(schema["type"], original["type"]);
    }

    #[test]
    fn non_object_properties_not_inferred() {
        // Extension data where "properties" is a string, not a schema keyword.
        // x-custom is not a known schema position, so it is not recursed into.
        let mut schema = json!({
            "x-custom": {
                "properties": "some-string-value",
                "additionalProperties": "version-sort"
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(
            schema["x-custom"].get("type").is_none(),
            "should not add type when properties is not an object"
        );
    }

    #[test]
    fn already_2020_12_still_normalizes_regex() {
        let mut schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string",
            "pattern": r"^{?[a-zA-Z0-9]+}?$"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["pattern"], r"^\{?[a-zA-Z0-9]+\}?$");
    }

    #[test]
    fn already_2020_12_still_removes_null_annotations() {
        let mut schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "description": null
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("description").is_none());
    }

    #[test]
    fn combined_old_draft_features() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-04/schema#",
            "id": "https://example.com/old",
            "type": "object",
            "definitions": {
                "Pos": {
                    "type": "number",
                    "minimum": 0,
                    "exclusiveMinimum": true
                }
            },
            "properties": {
                "coords": {
                    "items": [
                        {"$ref": "#/definitions/Pos"},
                        {"$ref": "#/definitions/Pos"}
                    ],
                    "additionalItems": false
                },
                "id": {"type": "string"}
            },
            "dependencies": {
                "coords": ["id"]
            }
        });
        migrate_to_2020_12(&mut schema);

        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert!(schema.get("definitions").is_none());
        assert!(schema["$defs"]["Pos"].get("minimum").is_none());
        assert_eq!(schema["$defs"]["Pos"]["exclusiveMinimum"], 0);
        assert_eq!(
            schema["properties"]["coords"]["prefixItems"][0]["$ref"],
            "#/$defs/Pos"
        );
        assert_eq!(schema["properties"]["coords"]["items"], false);
        assert!(
            schema["properties"]["coords"]
                .get("additionalItems")
                .is_none()
        );
        assert!(schema.get("dependencies").is_none());
        assert_eq!(schema["dependentRequired"]["coords"], json!(["id"]));
        assert!(schema["properties"].get("id").is_some());
    }

    // --- Edge case tests from keywords.rs ---

    #[test]
    fn id_renamed_on_schema_like_objects() {
        let mut schema = json!({
            "id": "https://example.com/schema",
            "type": "object"
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("id").is_none());
        assert_eq!(schema["$id"], "https://example.com/schema");
    }

    #[test]
    fn fragment_only_id_dropped() {
        let mut schema = json!({
            "id": "#/definitions/Foo",
            "type": "object"
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("id").is_none());
        assert!(schema.get("$id").is_none());
    }

    #[test]
    fn fragment_only_dollar_id_dropped() {
        let mut schema = json!({
            "$id": "#fragment",
            "type": "object"
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("$id").is_none());
    }

    #[test]
    fn string_booleans_converted() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": "false"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn dependencies_split_with_schema_keywords() {
        let mut schema = json!({
            "type": "object",
            "dependencies": {
                "bar": {"properties": {"baz": {"type": "string"}}},
                "quux": ["foo", "bar"]
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("dependencies").is_none());
        assert!(
            schema["dependentSchemas"]["bar"]
                .get("properties")
                .is_some()
        );
        assert_eq!(schema["dependentRequired"]["quux"], json!(["foo", "bar"]));
    }

    #[test]
    fn exclusive_minimum_bool_true() {
        let mut schema = json!({
            "type": "number",
            "minimum": 5,
            "exclusiveMinimum": true
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["exclusiveMinimum"], 5);
        assert!(schema.get("minimum").is_none());
    }

    #[test]
    fn exclusive_minimum_bool_false() {
        let mut schema = json!({
            "type": "number",
            "minimum": 5,
            "exclusiveMinimum": false
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("exclusiveMinimum").is_none());
        assert_eq!(schema["minimum"], 5);
    }

    #[test]
    fn exclusive_minimum_numeric_unchanged() {
        let mut schema = json!({
            "type": "number",
            "exclusiveMinimum": 5
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["exclusiveMinimum"], 5);
    }

    #[test]
    fn ref_definitions_rewritten() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "x": {"$ref": "#/definitions/Foo"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["properties"]["x"]["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn ref_defs_unchanged_for_2020_12() {
        let mut schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "x": {"$ref": "#/$defs/Foo"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["properties"]["x"]["$ref"], "#/$defs/Foo");
    }

    // --- Tests via migrate() returning Schema directly ---

    #[test]
    fn migrate_returns_typed_schema() {
        let schema = migrate(json!({
            "$schema": "http://json-schema.org/draft-04/schema#",
            "id": "https://example.com/test",
            "type": "object",
            "definitions": {
                "Foo": { "type": "string" }
            }
        }))
        .unwrap();

        assert_eq!(
            schema.schema.as_deref(),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
        assert_eq!(schema.id.as_deref(), Some("https://example.com/test"));
        assert!(schema.defs.is_some());
        let defs = schema.defs.as_ref().unwrap();
        assert!(defs.contains_key("Foo"));
    }

    #[test]
    fn array_in_property_becomes_enum() {
        let schema = migrate(json!({
            "type": "object",
            "properties": {
                "Type": ["Custom", "Steam"]
            }
        }))
        .unwrap();

        let props = schema.properties.as_ref().unwrap();
        let type_schema = props.get("Type").unwrap().as_schema().unwrap();
        assert_eq!(
            type_schema.enum_,
            Some(vec![json!("Custom"), json!("Steam")])
        );
    }

    #[test]
    fn string_in_properties_map_removed() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "real": {"type": "string"},
                "bad": "not a schema"
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"].get("real").is_some());
        assert!(schema["properties"].get("bad").is_none());
    }

    #[test]
    fn migrate_preserves_extra_fields() {
        let schema = migrate(json!({
            "type": "object",
            "x-custom": "hello",
            "x-other": 42
        }))
        .unwrap();

        assert_eq!(schema.extra.get("x-custom").unwrap(), "hello");
        assert_eq!(schema.extra.get("x-other").unwrap(), 42);
    }
}
