mod bounds;
mod cleanup;
mod deps;
mod id;
mod items;

use serde_json::{Map, Value};

use crate::draft::Draft;

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

/// Keys that hold a single nested schema value.
const SINGLE_SCHEMA_KEYS: &[&str] = &[
    "items",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
    "not",
    "if",
    "then",
    "else",
    "contentSchema",
];

/// Keys that hold a map of named schemas.
const MAP_SCHEMA_KEYS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "dependentSchemas",
];

/// Keys that hold an array of schemas.
const ARRAY_SCHEMA_KEYS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];

/// Migrate a JSON Schema map to draft 2020-12 in place.
///
/// Applies keyword transformations recursively through all nested schema
/// positions. After this call, the map is ready for `serde_json::from_value`.
pub fn migrate_in_place(obj: &mut Map<String, Value>, draft: Option<Draft>) {
    let needs_migration = draft != Some(Draft::Draft2020_12);
    let looks_like_schema = is_schema_like(obj);

    if needs_migration {
        id::migrate_id(obj, looks_like_schema);
        id::rewrite_ref_value(obj, "$id");
        id::rewrite_ref_value(obj, "$ref");
        id::migrate_defs(obj);
        items::migrate_items(obj);
        bounds::migrate_numeric_bounds(obj);
        deps::migrate_dependencies(obj, looks_like_schema);
    }

    // Always apply — cleanup operations valid for any draft
    id::strip_schema_fragment(obj);
    id::drop_fragment_only_id(obj);
    cleanup::migrate_deprecated(obj);
    cleanup::migrate_string_booleans(obj);
    cleanup::remove_nulls(obj);
    cleanup::normalize_pattern(obj);
    cleanup::normalize_pattern_property_keys(obj);
    cleanup::infer_type(obj);
    cleanup::migrate_required(obj);
    cleanup::sanitize_type(obj);
    cleanup::sanitize_enum(obj);
    cleanup::deduplicate_arrays(obj);
    cleanup::migrate_examples(obj);
    cleanup::flatten_defs(obj);

    // Recurse into nested schema positions
    for key in SINGLE_SCHEMA_KEYS {
        if let Some(v) = obj.get_mut(*key) {
            migrate_value_in_place(v, draft);
        }
    }
    for key in MAP_SCHEMA_KEYS {
        if let Some(Value::Object(map)) = obj.get_mut(*key) {
            // Remove non-schema entries (strings, numbers, nulls)
            map.retain(|_, v| matches!(v, Value::Object(_) | Value::Bool(_) | Value::Array(_)));
            for v in map.values_mut() {
                migrate_value_in_place(v, draft);
            }
        }
    }
    for key in ARRAY_SCHEMA_KEYS {
        if let Some(Value::Array(arr)) = obj.get_mut(*key) {
            for v in arr.iter_mut() {
                migrate_value_in_place(v, draft);
            }
        }
    }
}

fn migrate_value_in_place(value: &mut Value, draft: Option<Draft>) {
    match value {
        Value::Object(obj) => migrate_in_place(obj, draft),
        // Bare arrays in schema positions → wrap as `{"enum": [...]}`
        Value::Array(_) => {
            let arr = core::mem::replace(value, Value::Null);
            let mut obj = Map::new();
            obj.insert("enum".to_string(), arr);
            *value = Value::Object(obj);
        }
        _ => {}
    }
}

fn is_schema_like(obj: &Map<String, Value>) -> bool {
    SCHEMA_KEYWORDS.iter().any(|kw| obj.contains_key(*kw))
}

/// Convert string `"false"`/`"true"` to boolean values.
pub(crate) fn string_to_bool(value: Value) -> Value {
    if let Value::String(ref s) = value {
        match s.as_str() {
            "false" => return Value::Bool(false),
            "true" => return Value::Bool(true),
            _ => {}
        }
    }
    value
}
