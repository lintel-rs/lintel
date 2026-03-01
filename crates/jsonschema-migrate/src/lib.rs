#![doc = include_str!("../README.md")]

mod draft;
mod keywords;
mod regex;

pub use crate::draft::{Draft, detect_draft};
pub use crate::regex::normalize_ecma_regex;

use crate::keywords::{
    migrate_dependencies, migrate_exclusive_bound, migrate_id, migrate_string_booleans,
    rewrite_definition_refs,
};

/// Migrate a JSON Schema document to draft 2020-12 in-place.
///
/// Applies all necessary keyword transformations for drafts 04 through 2019-09.
/// Safe to call on schemas that are already 2020-12 (idempotent).
pub fn migrate_to_2020_12(schema: &mut serde_json::Value) {
    let draft = detect_draft(schema);

    // Pass 1: Set $schema at root
    if let Some(obj) = schema.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            serde_json::Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
        );
    }

    // Pass 2: Recursive keyword transformations
    migrate_keywords(schema, draft);

    // Pass 3: Rewrite $ref paths (#/definitions/ → #/$defs/)
    // Only needed for pre-2020-12 drafts (or unknown).
    if draft != Some(Draft::Draft2020_12) {
        rewrite_definition_refs(schema);
    }
}

/// Recursively apply keyword transformations.
fn migrate_keywords(value: &mut serde_json::Value, draft: Option<Draft>) {
    match value {
        serde_json::Value::Object(map) => {
            migrate_object_keywords(map, draft);
            for v in map.values_mut() {
                migrate_keywords(v, draft);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                migrate_keywords(v, draft);
            }
        }
        _ => {}
    }
}

/// Apply all keyword transforms to a single JSON object.
fn migrate_object_keywords(
    map: &mut serde_json::Map<String, serde_json::Value>,
    draft: Option<Draft>,
) {
    let needs_keyword_migration = draft != Some(Draft::Draft2020_12);

    if needs_keyword_migration {
        // definitions → $defs (drafts 04 through 2019-09)
        if map.contains_key("definitions")
            && !map.contains_key("$defs")
            && let Some(defs) = map.remove("definitions")
        {
            map.insert("$defs".to_string(), defs);
        }

        // id → $id (draft 04 only, but safe for all pre-2020-12)
        migrate_id(map);

        // Array items → prefixItems (pre-2020-12)
        if let Some(items) = map.get("items")
            && items.is_array()
        {
            if let Some(items_val) = map.remove("items") {
                map.insert("prefixItems".to_string(), items_val);
            }
            if let Some(additional) = map.remove("additionalItems") {
                map.insert("items".to_string(), additional);
            }
        }

        // Boolean exclusiveMinimum/exclusiveMaximum (draft-04 style, type-guarded)
        migrate_exclusive_bound(map, "exclusiveMinimum", "minimum");
        migrate_exclusive_bound(map, "exclusiveMaximum", "maximum");

        // dependencies → dependentSchemas + dependentRequired (pre-2020-12)
        migrate_dependencies(map);
    }

    // --- Cleanup/normalization fixes (always applied) ---

    // Strip fragment from $schema (e.g. "…/draft-07/schema#" → "…/draft-07/schema").
    // Fragments in base URIs cause compilation failures in jsonschema 0.42.2+.
    if let Some(serde_json::Value::String(s)) = map.get("$schema")
        && let Some(pos) = s.find('#')
    {
        let stripped = s[..pos].to_string();
        map.insert("$schema".to_string(), serde_json::Value::String(stripped));
    }

    // String "deprecated" → boolean true
    if let Some(dep) = map.get("deprecated")
        && dep.is_string()
    {
        map.insert("deprecated".to_string(), serde_json::Value::Bool(true));
    }

    // String "false"/"true" in schema-boolean positions
    migrate_string_booleans(map);

    // Null annotation keywords → remove
    // Some generators emit "description": null, "title": null, etc.
    // The meta-schema requires these to be strings.
    for key in &["description", "title", "$comment"] {
        if let Some(v) = map.get(*key)
            && v.is_null()
        {
            map.remove(*key);
        }
    }

    // Normalize regex patterns for Rust regex_syntax compatibility
    if let Some(serde_json::Value::String(pat)) = map.get("pattern") {
        let norm = normalize_ecma_regex(pat);
        if norm != *pat {
            map.insert("pattern".to_string(), serde_json::Value::String(norm));
        }
    }
    if let Some(serde_json::Value::Object(pp)) = map.get("patternProperties") {
        let any_changed = pp.keys().any(|k| normalize_ecma_regex(k) != *k);
        if any_changed && let Some(serde_json::Value::Object(pp)) = map.remove("patternProperties")
        {
            let new_pp: serde_json::Map<String, serde_json::Value> = pp
                .into_iter()
                .map(|(k, v)| (normalize_ecma_regex(&k), v))
                .collect();
            map.insert(
                "patternProperties".to_string(),
                serde_json::Value::Object(new_pp),
            );
        }
    }
}

#[cfg(test)]
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
}
