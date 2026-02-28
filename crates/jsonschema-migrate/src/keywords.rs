/// Schema keywords used to distinguish schema-like objects from data properties.
const SCHEMA_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "$ref",
    "allOf",
    "oneOf",
    "anyOf",
    "definitions",
    "$defs",
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

/// Migrate `id` → `$id` (draft-04) and remove fragment-only identifiers.
pub(crate) fn migrate_id(map: &mut serde_json::Map<String, serde_json::Value>) {
    // id → $id (only non-fragment, schema-like objects)
    if !map.contains_key("$id")
        && let Some(serde_json::Value::String(id_str)) = map.get("id")
        && !id_str.starts_with('#')
    {
        let looks_like_schema = SCHEMA_KEYWORDS.iter().any(|kw| map.contains_key(*kw));
        if looks_like_schema && let Some(id_val) = map.remove("id") {
            map.insert("$id".to_string(), id_val);
        }
    }
    // Remove fragment-only id values
    if let Some(serde_json::Value::String(id_str)) = map.get("id")
        && id_str.starts_with('#')
    {
        map.remove("id");
    }
    // Remove fragment-only $id values (invalid in 2020-12)
    if let Some(serde_json::Value::String(id_str)) = map.get("$id")
        && id_str.starts_with('#')
    {
        map.remove("$id");
    }
}

/// Fix string `"false"`/`"true"` in positions that require boolean or schema.
pub(crate) fn migrate_string_booleans(map: &mut serde_json::Map<String, serde_json::Value>) {
    for key in &[
        "additionalProperties",
        "additionalItems",
        "unevaluatedProperties",
        "unevaluatedItems",
    ] {
        if let Some(serde_json::Value::String(s)) = map.get(*key) {
            let replacement = match s.as_str() {
                "false" => Some(false),
                "true" => Some(true),
                _ => None,
            };
            if let Some(b) = replacement {
                map.insert((*key).to_string(), serde_json::Value::Bool(b));
            }
        }
    }
}

/// Split `dependencies` into `dependentSchemas` + `dependentRequired`.
///
/// Only applies when the containing object looks like a schema (has schema keywords),
/// to avoid transforming property definitions named "dependencies"
/// (e.g. Cargo.toml's `[dependencies]` section inside a `properties` map).
pub(crate) fn migrate_dependencies(map: &mut serde_json::Map<String, serde_json::Value>) {
    if !map.contains_key("dependencies") {
        return;
    }
    let looks_like_schema = SCHEMA_KEYWORDS.iter().any(|kw| map.contains_key(*kw));
    if !looks_like_schema {
        return;
    }
    let Some(serde_json::Value::Object(deps)) = map.remove("dependencies") else {
        return;
    };
    let mut schemas = serde_json::Map::new();
    let mut required = serde_json::Map::new();
    for (key, val) in deps {
        if val.is_array() {
            required.insert(key, val);
        } else {
            schemas.insert(key, val);
        }
    }
    if !schemas.is_empty() && !map.contains_key("dependentSchemas") {
        map.insert(
            "dependentSchemas".to_string(),
            serde_json::Value::Object(schemas),
        );
    }
    if !required.is_empty() && !map.contains_key("dependentRequired") {
        map.insert(
            "dependentRequired".to_string(),
            serde_json::Value::Object(required),
        );
    }
}

/// Handle boolean `exclusiveMinimum`/`exclusiveMaximum` (draft-04).
///
/// - `true` + companion present: set exclusive = companion value, remove companion
/// - `false`: just remove exclusive
pub(crate) fn migrate_exclusive_bound(
    map: &mut serde_json::Map<String, serde_json::Value>,
    exclusive_key: &str,
    companion_key: &str,
) {
    if let Some(serde_json::Value::Bool(b)) = map.get(exclusive_key) {
        let b = *b;
        if b {
            if let Some(companion) = map.get(companion_key).cloned() {
                map.insert(exclusive_key.to_string(), companion);
                map.remove(companion_key);
            } else {
                map.remove(exclusive_key);
            }
        } else {
            map.remove(exclusive_key);
        }
    }
}

/// Rewrite `#/definitions/` → `#/$defs/` in `$ref` and `$id` strings.
pub(crate) fn rewrite_definition_refs(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in &["$ref", "$id"] {
                if let Some(serde_json::Value::String(s)) = map.get(*key) {
                    let new_s = s.replace("#/definitions/", "#/$defs/");
                    if new_s != *s {
                        map.insert((*key).to_string(), serde_json::Value::String(new_s));
                    }
                }
            }
            for v in map.values_mut() {
                rewrite_definition_refs(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                rewrite_definition_refs(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn id_renamed_on_schema_like_objects() {
        let mut map = json!({
            "id": "https://example.com/schema",
            "type": "object"
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_id(&mut map);
        assert!(map.get("id").is_none());
        assert_eq!(map["$id"], "https://example.com/schema");
    }

    #[test]
    fn id_not_renamed_without_schema_keywords() {
        let mut map = json!({
            "id": "https://example.com/record",
            "name": "test"
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_id(&mut map);
        // No schema keywords → id should stay
        assert!(map.get("id").is_some());
        assert!(map.get("$id").is_none());
    }

    #[test]
    fn fragment_only_id_dropped() {
        let mut map = json!({
            "id": "#/definitions/Foo",
            "type": "object"
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_id(&mut map);
        assert!(map.get("id").is_none());
        assert!(map.get("$id").is_none());
    }

    #[test]
    fn fragment_only_dollar_id_dropped() {
        let mut map = json!({
            "$id": "#fragment",
            "type": "object"
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_id(&mut map);
        assert!(map.get("$id").is_none());
    }

    #[test]
    fn string_booleans_converted() {
        let mut map = json!({
            "additionalProperties": "false"
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_string_booleans(&mut map);
        assert_eq!(map["additionalProperties"], false);
    }

    #[test]
    fn dependencies_split_with_schema_keywords() {
        let mut map = json!({
            "type": "object",
            "dependencies": {
                "bar": {"properties": {"baz": {"type": "string"}}},
                "quux": ["foo", "bar"]
            }
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_dependencies(&mut map);
        assert!(map.get("dependencies").is_none());
        assert!(map["dependentSchemas"]["bar"].get("properties").is_some());
        assert_eq!(map["dependentRequired"]["quux"], json!(["foo", "bar"]));
    }

    #[test]
    fn dependencies_not_split_without_schema_keywords() {
        let mut map = json!({
            "dependencies": {
                "type": "object",
                "additionalProperties": {"type": "string"}
            },
            "dev-dependencies": {"type": "object"}
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_dependencies(&mut map);
        // No schema keywords as siblings → must not transform
        assert!(map.get("dependencies").is_some());
        assert!(map.get("dependentSchemas").is_none());
    }

    #[test]
    fn exclusive_minimum_bool_true() {
        let mut map = json!({
            "minimum": 5,
            "exclusiveMinimum": true
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_exclusive_bound(&mut map, "exclusiveMinimum", "minimum");
        assert_eq!(map["exclusiveMinimum"], 5);
        assert!(map.get("minimum").is_none());
    }

    #[test]
    fn exclusive_minimum_bool_false() {
        let mut map = json!({
            "minimum": 5,
            "exclusiveMinimum": false
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_exclusive_bound(&mut map, "exclusiveMinimum", "minimum");
        assert!(map.get("exclusiveMinimum").is_none());
        assert_eq!(map["minimum"], 5);
    }

    #[test]
    fn exclusive_minimum_numeric_unchanged() {
        let mut map = json!({
            "exclusiveMinimum": 5
        })
        .as_object()
        .expect("json object")
        .clone();
        migrate_exclusive_bound(&mut map, "exclusiveMinimum", "minimum");
        assert_eq!(map["exclusiveMinimum"], 5);
    }

    #[test]
    fn ref_definitions_rewritten() {
        let mut value = json!({"$ref": "#/definitions/Foo"});
        rewrite_definition_refs(&mut value);
        assert_eq!(value["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn ref_external_definitions_rewritten() {
        let mut value = json!({"$ref": "https://x.com/foo.json#/definitions/Bar"});
        rewrite_definition_refs(&mut value);
        assert_eq!(value["$ref"], "https://x.com/foo.json#/$defs/Bar");
    }

    #[test]
    fn dollar_id_definitions_rewritten() {
        let mut value = json!({"$id": "https://example.com/s#/definitions/nested"});
        rewrite_definition_refs(&mut value);
        assert_eq!(value["$id"], "https://example.com/s#/$defs/nested");
    }

    #[test]
    fn ref_defs_unchanged() {
        let mut value = json!({"$ref": "#/$defs/Foo"});
        rewrite_definition_refs(&mut value);
        assert_eq!(value["$ref"], "#/$defs/Foo");
    }
}
