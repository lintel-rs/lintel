use serde_json::{Map, Value};

/// Split `dependencies` → `dependentRequired` + `dependentSchemas`.
///
/// Array values become `dependentRequired`, object values become
/// `dependentSchemas`. Merges into any existing 2020-12 keywords.
/// Only runs when the object looks like a schema (has standard keywords).
pub fn migrate_dependencies(obj: &mut Map<String, Value>, looks_like_schema: bool) {
    if !looks_like_schema {
        return;
    }

    let Some(Value::Object(deps)) = obj.remove("dependencies") else {
        return;
    };

    let mut required_map: Map<String, Value> = Map::new();
    let mut schemas_map: Map<String, Value> = Map::new();

    for (key, val) in deps {
        if val.is_array() {
            required_map.insert(key, val);
        } else {
            schemas_map.insert(key, val);
        }
    }

    if !required_map.is_empty() {
        if let Some(Value::Object(existing)) = obj.get_mut("dependentRequired") {
            existing.extend(required_map);
        } else {
            obj.insert("dependentRequired".to_string(), Value::Object(required_map));
        }
    }

    if !schemas_map.is_empty() {
        if let Some(Value::Object(existing)) = obj.get_mut("dependentSchemas") {
            existing.extend(schemas_map);
        } else {
            obj.insert("dependentSchemas".to_string(), Value::Object(schemas_map));
        }
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
    fn splits_array_and_schema_deps() {
        let mut m = obj(json!({
            "type": "object",
            "dependencies": {
                "bar": {"properties": {"baz": {"type": "string"}}},
                "quux": ["foo", "bar"]
            }
        }));
        migrate_dependencies(&mut m, true);
        assert!(!m.contains_key("dependencies"));
        assert_eq!(m["dependentRequired"]["quux"], json!(["foo", "bar"]));
        assert!(m["dependentSchemas"]["bar"].get("properties").is_some());
    }

    #[test]
    fn skips_when_not_schema_like() {
        let mut m = obj(json!({
            "dependencies": {"a": ["b"]}
        }));
        migrate_dependencies(&mut m, false);
        assert!(m.contains_key("dependencies"));
    }

    #[test]
    fn merges_into_existing() {
        let mut m = obj(json!({
            "type": "object",
            "dependentRequired": {"existing": ["x"]},
            "dependencies": {"new": ["y"]}
        }));
        migrate_dependencies(&mut m, true);
        assert_eq!(m["dependentRequired"]["existing"], json!(["x"]));
        assert_eq!(m["dependentRequired"]["new"], json!(["y"]));
    }
}
