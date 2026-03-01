/// Preferred key order for the root object of a JSON Schema.
const SCHEMA_KEY_ORDER: &[&str] = &[
    "$schema",
    "$id",
    "title",
    "description",
    "x-lintel",
    "type",
    "properties",
];

/// Reorder the top-level keys of a JSON Schema object so that well-known
/// fields appear first (in [`SCHEMA_KEY_ORDER`]), followed by the rest in
/// their original order.
pub(super) fn reorder_schema_keys(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let mut ordered = serde_json::Map::with_capacity(obj.len());
    for &key in SCHEMA_KEY_ORDER {
        if let Some(v) = obj.remove(key) {
            ordered.insert(key.to_string(), v);
        }
    }
    // Append remaining keys in their original order
    ordered.extend(core::mem::take(obj));
    *obj = ordered;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_keys_come_first() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {},
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Test",
            "description": "A test schema",
            "additionalProperties": false
        });
        reorder_schema_keys(&mut schema);

        let keys: Vec<&String> = schema
            .as_object()
            .expect("test value is an object")
            .keys()
            .collect();
        assert_eq!(
            keys,
            &[
                "$schema",
                "title",
                "description",
                "type",
                "properties",
                "additionalProperties"
            ]
        );
    }

    #[test]
    fn non_object_is_noop() {
        let mut value = serde_json::json!("just a string");
        reorder_schema_keys(&mut value);
        assert_eq!(value, "just a string");
    }
}
