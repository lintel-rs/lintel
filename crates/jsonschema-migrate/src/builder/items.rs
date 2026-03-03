use serde_json::{Map, Value};

use super::string_to_bool;

/// Migrate items/prefixItems/additionalItems for pre-2020-12 schemas.
///
/// If `items` is an array:
/// - `items` (array) → `prefixItems`
/// - `additionalItems` → `items` (with string-to-bool conversion)
pub fn migrate_items(obj: &mut Map<String, Value>) {
    if !obj.get("items").is_some_and(Value::is_array) {
        return;
    }

    if let Some(items) = obj.remove("items") {
        obj.insert("prefixItems".to_string(), items);
    }

    if let Some(ai) = obj.remove("additionalItems") {
        obj.insert("items".to_string(), string_to_bool(ai));
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
    fn array_items_becomes_prefix_items() {
        let mut m = obj(json!({
            "items": [{"type": "string"}, {"type": "number"}]
        }));
        migrate_items(&mut m);
        assert!(!m.contains_key("items"));
        assert!(m.contains_key("prefixItems"));
        assert_eq!(m["prefixItems"][0]["type"], "string");
    }

    #[test]
    fn additional_items_becomes_items() {
        let mut m = obj(json!({
            "items": [{"type": "string"}],
            "additionalItems": {"type": "number"}
        }));
        migrate_items(&mut m);
        assert_eq!(m["items"]["type"], "number");
        assert!(!m.contains_key("additionalItems"));
    }

    #[test]
    fn additional_items_string_bool_converted() {
        let mut m = obj(json!({
            "items": [{"type": "string"}],
            "additionalItems": "false"
        }));
        migrate_items(&mut m);
        assert_eq!(m["items"], false);
    }

    #[test]
    fn schema_items_unchanged() {
        let mut m = obj(json!({"items": {"type": "string"}}));
        migrate_items(&mut m);
        assert_eq!(m["items"]["type"], "string");
        assert!(!m.contains_key("prefixItems"));
    }
}
