use serde_json::{Map, Value};

use super::string_to_bool;
use crate::regex::normalize_ecma_regex;

/// Convert string `deprecated` to bool `true`.
pub fn migrate_deprecated(obj: &mut Map<String, Value>) {
    if matches!(obj.get("deprecated"), Some(Value::String(_))) {
        obj.insert("deprecated".to_string(), Value::Bool(true));
    }
}

/// Convert string `"true"`/`"false"` to boolean for schema-value keys.
pub fn migrate_string_booleans(obj: &mut Map<String, Value>) {
    for key in [
        "additionalProperties",
        "unevaluatedProperties",
        "unevaluatedItems",
    ] {
        if let Some(val) = obj.remove(key) {
            obj.insert(key.to_string(), string_to_bool(val));
        }
    }
}

/// Remove null values for annotation keys.
pub fn remove_nulls(obj: &mut Map<String, Value>) {
    for key in ["description", "title", "$comment"] {
        if matches!(obj.get(key), Some(Value::Null)) {
            obj.remove(key);
        }
    }
}

/// Normalize ECMA regex in `pattern` value.
pub fn normalize_pattern(obj: &mut Map<String, Value>) {
    if let Some(Value::String(p)) = obj.get_mut("pattern") {
        *p = normalize_ecma_regex(p);
    }
}

/// Normalize ECMA regex in `patternProperties` keys.
pub fn normalize_pattern_property_keys(obj: &mut Map<String, Value>) {
    if let Some(Value::Object(pp)) = obj.remove("patternProperties") {
        let normalized: Map<String, Value> = pp
            .into_iter()
            .map(|(k, v)| (normalize_ecma_regex(&k), v))
            .collect();
        obj.insert("patternProperties".to_string(), Value::Object(normalized));
    }
}

/// Infer `type: "object"` when `properties` is present but `type` is missing.
pub fn infer_type(obj: &mut Map<String, Value>) {
    if !obj.contains_key("type") && matches!(obj.get("properties"), Some(Value::Object(_))) {
        obj.insert("type".to_string(), Value::String("object".to_string()));
    }
}

/// Migrate draft-03 `"required": true` from child properties to a `"required"` array
/// on the parent object.
///
/// In draft-03, `required` was a boolean on each property schema. In draft-04+,
/// it became an array of property names on the parent. This function scans
/// `properties` for entries with `"required": true`, collects those names,
/// removes the boolean from each child, and merges them into the parent's
/// `"required"` array.
pub fn migrate_required(obj: &mut Map<String, Value>) {
    let Some(Value::Object(props)) = obj.get_mut("properties") else {
        return;
    };

    let mut required_names: Vec<String> = Vec::new();
    for (name, schema) in props.iter_mut() {
        if let Value::Object(prop_obj) = schema {
            if matches!(prop_obj.get("required"), Some(Value::Bool(true))) {
                required_names.push(name.clone());
                prop_obj.remove("required");
            } else if matches!(prop_obj.get("required"), Some(v) if !v.is_array()) {
                // Remove other non-array required values (e.g. `required: false`)
                prop_obj.remove("required");
            }
        }
    }

    if required_names.is_empty() {
        return;
    }

    // Merge with any existing required array on the parent
    if let Some(Value::Array(existing)) = obj.get_mut("required") {
        for name in required_names {
            let val = Value::String(name);
            if !existing.contains(&val) {
                existing.push(val);
            }
        }
    } else {
        obj.insert(
            "required".to_string(),
            Value::Array(required_names.into_iter().map(Value::String).collect()),
        );
    }
}

/// Normalize `type` by removing invalid values (not a string or array, e.g. `"type": {}`).
pub fn normalize_type(obj: &mut Map<String, Value>) {
    match obj.get("type") {
        Some(Value::String(_) | Value::Array(_)) | None => {}
        Some(_) => {
            obj.remove("type");
        }
    }
}

/// Normalize non-array `enum` values.
///
/// When `enum` is an object with a `$ref` key (e.g. `"enum": {"$ref": "#/$defs/Foo"}`),
/// the author intended the enum values to come from a referenced definition.
/// We promote the `$ref` to the parent schema level (wrapping the current schema
/// in an `allOf` if it has other keywords) and remove the invalid `enum`.
/// Other non-array `enum` values are simply removed.
pub fn normalize_enum(obj: &mut Map<String, Value>) {
    let Some(val) = obj.get("enum") else { return };
    if val.is_array() {
        return;
    }

    // If it's {"$ref": "..."}, promote the $ref
    if let Value::Object(enum_obj) = val
        && let Some(Value::String(ref_val)) = enum_obj.get("$ref")
    {
        let ref_val = ref_val.clone();
        obj.remove("enum");
        if !obj.contains_key("$ref") {
            obj.insert("$ref".to_string(), Value::String(ref_val));
        }
        return;
    }

    obj.remove("enum");
}

/// Deduplicate array values for keywords that require unique elements.
///
/// JSON Schema requires `enum` and `required` to have unique elements.
/// Some schemas have duplicate entries (e.g. `["implementation", "implementation"]`).
pub fn deduplicate_arrays(obj: &mut Map<String, Value>) {
    for key in ["enum", "required"] {
        if let Some(Value::Array(arr)) = obj.get_mut(key) {
            let mut seen = Vec::with_capacity(arr.len());
            arr.retain(|v| {
                if seen.contains(v) {
                    false
                } else {
                    seen.push(v.clone());
                    true
                }
            });
        }
    }
}

/// Wrap bare non-array `examples` in an array.
///
/// `examples` should be `[value, ...]` but some schemas use a bare value
/// like `"examples": "Present"`. Wrap it in an array.
pub fn migrate_examples(obj: &mut Map<String, Value>) {
    if matches!(obj.get("examples"), Some(v) if !v.is_array())
        && let Some(val) = obj.remove("examples")
    {
        obj.insert("examples".to_string(), Value::Array(vec![val]));
    }
}

/// Sanitize `$defs`/`definitions` maps.
///
/// Applies two transforms:
///
/// 1. **Promote annotations**: Entries like `"description"` or `"$comment"` with
///    string values are promoted to the parent object (the author intended them
///    as annotations on the `$defs` block).
///
/// 2. **Remove non-schema entries**: Any remaining non-object/non-bool values
///    are removed.
pub fn flatten_defs(obj: &mut Map<String, Value>) {
    const ANNOTATION_KEYS: &[&str] = &["description", "title", "$comment"];

    for defs_key in ["$defs", "definitions"] {
        let Some(Value::Object(defs)) = obj.get_mut(defs_key) else {
            continue;
        };

        // 1. Promote known annotation strings to the parent
        //    Only promote if the value is a string — object/bool values are
        //    real schema definitions that happen to share an annotation key name.
        let mut promoted = Vec::new();
        for anno in ANNOTATION_KEYS {
            if matches!(defs.get(*anno), Some(Value::String(_)))
                && let Some(Value::String(s)) = defs.remove(*anno)
            {
                promoted.push((*anno, s));
            }
        }
        for (key, val) in promoted {
            obj.entry(key.to_string()).or_insert(Value::String(val));
        }

        // 2. Remove any remaining non-schema entries
        let Some(Value::Object(defs)) = obj.get_mut(defs_key) else {
            continue;
        };
        defs.retain(|_, v| matches!(v, Value::Object(_) | Value::Bool(_)));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(value: serde_json::Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn string_deprecated_becomes_bool() {
        let mut m = obj(json!({"deprecated": "Use new_field"}));
        migrate_deprecated(&mut m);
        assert_eq!(m["deprecated"], true);
    }

    #[test]
    fn bool_deprecated_unchanged() {
        let mut m = obj(json!({"deprecated": true}));
        migrate_deprecated(&mut m);
        assert_eq!(m["deprecated"], true);
    }

    #[test]
    fn string_booleans_converted() {
        let mut m = obj(json!({"additionalProperties": "false"}));
        migrate_string_booleans(&mut m);
        assert_eq!(m["additionalProperties"], false);
    }

    #[test]
    fn nulls_removed() {
        let mut m = obj(json!({"description": null, "title": null, "$comment": null}));
        remove_nulls(&mut m);
        assert!(!m.contains_key("description"));
        assert!(!m.contains_key("title"));
        assert!(!m.contains_key("$comment"));
    }

    #[test]
    fn pattern_normalized() {
        let mut m = obj(json!({"pattern": r"^{?[a-z]+}?$"}));
        normalize_pattern(&mut m);
        assert_eq!(m["pattern"], r"^\{?[a-z]+\}?$");
    }

    #[test]
    fn pattern_property_keys_normalized() {
        let mut m = obj(json!({"patternProperties": {"^{[a-z]+}$": {"type": "string"}}}));
        normalize_pattern_property_keys(&mut m);
        assert!(m["patternProperties"].get(r"^\{[a-z]+\}$").is_some());
    }

    #[test]
    fn infer_type_adds_object() {
        let mut m = obj(json!({"properties": {"x": {"type": "string"}}}));
        infer_type(&mut m);
        assert_eq!(m["type"], "object");
    }

    #[test]
    fn infer_type_skips_when_type_present() {
        let mut m = obj(json!({"type": "array", "properties": {"x": {}}}));
        infer_type(&mut m);
        assert_eq!(m["type"], "array");
    }

    #[test]
    fn infer_type_skips_non_object_properties() {
        let mut m = obj(json!({"properties": "not-an-object"}));
        infer_type(&mut m);
        assert!(!m.contains_key("type"));
    }

    #[test]
    fn migrate_required_collects_from_properties() {
        let mut m = obj(json!({
            "properties": {
                "uri": {"type": "string", "required": true},
                "name": {"type": "string", "required": false},
                "age": {"type": "number"}
            }
        }));
        migrate_required(&mut m);
        assert_eq!(m["required"], json!(["uri"]));
        // required: true removed from child
        assert!(m["properties"]["uri"].get("required").is_none());
        // required: false also removed from child
        assert!(m["properties"]["name"].get("required").is_none());
    }

    #[test]
    fn migrate_required_merges_with_existing() {
        let mut m = obj(json!({
            "required": ["existing"],
            "properties": {
                "uri": {"type": "string", "required": true}
            }
        }));
        migrate_required(&mut m);
        let req = m["required"].as_array().unwrap();
        assert!(req.contains(&json!("existing")));
        assert!(req.contains(&json!("uri")));
    }

    #[test]
    fn migrate_required_no_properties_is_noop() {
        let mut m = obj(json!({"type": "object"}));
        migrate_required(&mut m);
        assert!(!m.contains_key("required"));
    }

    #[test]
    fn normalize_type_removes_empty_object() {
        let mut m = obj(json!({"type": {}}));
        normalize_type(&mut m);
        assert!(!m.contains_key("type"));
    }

    #[test]
    fn normalize_type_keeps_string() {
        let mut m = obj(json!({"type": "object"}));
        normalize_type(&mut m);
        assert_eq!(m["type"], "object");
    }

    #[test]
    fn normalize_type_keeps_array() {
        let mut m = obj(json!({"type": ["string", "null"]}));
        normalize_type(&mut m);
        assert_eq!(m["type"], json!(["string", "null"]));
    }

    #[test]
    fn normalize_enum_promotes_ref() {
        let mut m = obj(json!({"enum": {"$ref": "#/$defs/Foo"}}));
        normalize_enum(&mut m);
        assert!(!m.contains_key("enum"));
        assert_eq!(m["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn normalize_enum_ref_no_overwrite() {
        let mut m = obj(json!({"$ref": "#/$defs/Bar", "enum": {"$ref": "#/$defs/Foo"}}));
        normalize_enum(&mut m);
        assert!(!m.contains_key("enum"));
        // Existing $ref is preserved, not overwritten
        assert_eq!(m["$ref"], "#/$defs/Bar");
    }

    #[test]
    fn normalize_enum_removes_non_ref_object() {
        let mut m = obj(json!({"enum": {"foo": "bar"}}));
        normalize_enum(&mut m);
        assert!(!m.contains_key("enum"));
        assert!(!m.contains_key("$ref"));
    }

    #[test]
    fn normalize_enum_keeps_array() {
        let mut m = obj(json!({"enum": ["a", "b"]}));
        normalize_enum(&mut m);
        assert_eq!(m["enum"], json!(["a", "b"]));
    }

    #[test]
    fn flatten_defs_promotes_annotations() {
        let mut m = obj(json!({"$defs": {
            "Foo": {"type": "string"},
            "$comment": "This is a comment",
            "description": "A description"
        }}));
        flatten_defs(&mut m);
        let defs = m["$defs"].as_object().unwrap();
        assert!(defs.contains_key("Foo"));
        assert!(!defs.contains_key("$comment"));
        assert!(!defs.contains_key("description"));
        // Annotations promoted to parent
        assert_eq!(m["$comment"], "This is a comment");
        assert_eq!(m["description"], "A description");
    }

    #[test]
    fn flatten_defs_no_overwrite_existing_annotations() {
        let mut m = obj(json!({
            "description": "Parent description",
            "$defs": {
                "description": "Defs description"
            }
        }));
        flatten_defs(&mut m);
        // Parent's existing description is preserved
        assert_eq!(m["description"], "Parent description");
    }

    #[test]
    fn flatten_defs_keeps_bool_schemas() {
        let mut m = obj(json!({"$defs": {
            "Anything": true,
            "Nothing": false
        }}));
        flatten_defs(&mut m);
        let defs = m["$defs"].as_object().unwrap();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn migrate_examples_wraps_string() {
        let mut m = obj(json!({"examples": "Present"}));
        migrate_examples(&mut m);
        assert_eq!(m["examples"], json!(["Present"]));
    }

    #[test]
    fn migrate_examples_keeps_array() {
        let mut m = obj(json!({"examples": ["a", "b"]}));
        migrate_examples(&mut m);
        assert_eq!(m["examples"], json!(["a", "b"]));
    }

    #[test]
    fn migrate_examples_wraps_number() {
        let mut m = obj(json!({"examples": 42}));
        migrate_examples(&mut m);
        assert_eq!(m["examples"], json!([42]));
    }
}
