use serde_json::{Map, Value};

/// Migrate `id` → `$id`.
///
/// - Removes fragment-only `id` (starts with `#`)
/// - Renames `id` → `$id` if the object looks like a schema and no `$id` exists
pub fn migrate_id(obj: &mut Map<String, Value>, looks_like_schema: bool) {
    // Remove fragment-only "id"
    if matches!(obj.get("id"), Some(Value::String(s)) if s.starts_with('#')) {
        obj.remove("id");
    }

    // Rename "id" → "$id" if schema-like and no $id exists
    if !obj.contains_key("$id")
        && matches!(obj.get("id"), Some(Value::String(s)) if !s.starts_with('#'))
        && looks_like_schema
    {
        let val = obj.remove("id").expect("id key confirmed present");
        obj.insert("$id".to_string(), val);
    }
}

/// Drop fragment-only `$id` (starts with `#`).
pub fn drop_fragment_only_id(obj: &mut Map<String, Value>) {
    if matches!(obj.get("$id"), Some(Value::String(s)) if s.starts_with('#')) {
        obj.remove("$id");
    }
}

/// Rewrite `/definitions/` → `/$defs/` in a string value at the given key.
///
/// Replaces all occurrences, including nested paths like
/// `#/$defs/Outer/definitions/Inner` → `#/$defs/Outer/$defs/Inner`.
pub fn rewrite_ref_value(obj: &mut Map<String, Value>, key: &str) {
    if let Some(Value::String(s)) = obj.get_mut(key)
        && s.contains("/definitions/")
    {
        *s = s.replace("/definitions/", "/$defs/");
    }
}

/// Rename `definitions` → `$defs`. If both exist, drop `definitions`.
pub fn migrate_defs(obj: &mut Map<String, Value>) {
    if obj.contains_key("$defs") {
        obj.remove("definitions");
    } else if let Some(defs) = obj.remove("definitions") {
        obj.insert("$defs".to_string(), defs);
    }
}

/// Strip trailing `#` fragment from `$schema` value.
pub fn strip_schema_fragment(obj: &mut Map<String, Value>) {
    if let Some(Value::String(s)) = obj.get_mut("$schema")
        && let Some(pos) = s.find('#')
    {
        s.truncate(pos);
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
    fn renames_id_to_dollar_id() {
        let mut m = obj(json!({"id": "https://example.com", "type": "object"}));
        migrate_id(&mut m, true);
        assert!(!m.contains_key("id"));
        assert_eq!(m["$id"], "https://example.com");
    }

    #[test]
    fn skips_rename_when_not_schema_like() {
        let mut m = obj(json!({"id": "https://example.com"}));
        migrate_id(&mut m, false);
        assert!(m.contains_key("id"));
        assert!(!m.contains_key("$id"));
    }

    #[test]
    fn drops_fragment_only_id() {
        let mut m = obj(json!({"id": "#foo", "type": "object"}));
        migrate_id(&mut m, true);
        assert!(!m.contains_key("id"));
        assert!(!m.contains_key("$id"));
    }

    #[test]
    fn drops_fragment_only_dollar_id() {
        let mut m = obj(json!({"$id": "#fragment"}));
        drop_fragment_only_id(&mut m);
        assert!(!m.contains_key("$id"));
    }

    #[test]
    fn preserves_normal_dollar_id() {
        let mut m = obj(json!({"$id": "https://example.com"}));
        drop_fragment_only_id(&mut m);
        assert_eq!(m["$id"], "https://example.com");
    }

    #[test]
    fn rewrites_definitions_in_ref() {
        let mut m = obj(json!({"$ref": "#/definitions/Foo"}));
        rewrite_ref_value(&mut m, "$ref");
        assert_eq!(m["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn preserves_defs_in_ref() {
        let mut m = obj(json!({"$ref": "#/$defs/Foo"}));
        rewrite_ref_value(&mut m, "$ref");
        assert_eq!(m["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn migrate_defs_renames() {
        let mut m = obj(json!({"definitions": {"Foo": {"type": "string"}}}));
        migrate_defs(&mut m);
        assert!(!m.contains_key("definitions"));
        assert!(m.contains_key("$defs"));
    }

    #[test]
    fn migrate_defs_prefers_existing_dollar_defs() {
        let mut m = obj(json!({
            "$defs": {"A": {}},
            "definitions": {"B": {}}
        }));
        migrate_defs(&mut m);
        assert!(!m.contains_key("definitions"));
        assert!(m["$defs"].get("A").is_some());
    }

    #[test]
    fn strips_schema_fragment() {
        let mut m = obj(json!({"$schema": "http://json-schema.org/draft-07/schema#"}));
        strip_schema_fragment(&mut m);
        assert_eq!(m["$schema"], "http://json-schema.org/draft-07/schema");
    }
}
