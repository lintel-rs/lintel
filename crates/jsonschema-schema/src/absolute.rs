use crate::schema::{Schema, SchemaValue};

/// Rewrite all local `$ref` pointers (`#/…`) to absolute URLs using the
/// schema's `$id` as base.  Returns the schema unchanged if `$id` is absent.
pub fn make_absolute(schema: &Schema) -> Schema {
    let Some(ref id) = schema.id else {
        return schema.clone();
    };
    let base = id.trim_end_matches('#');
    let mut out = schema.clone();
    rewrite_schema(&mut out, base);
    out
}

fn rewrite_schema(schema: &mut Schema, base: &str) {
    if let Some(ref mut r) = schema.ref_
        && r.starts_with('#')
    {
        *r = format!("{base}{r}");
    }

    // Map fields
    if let Some(ref mut props) = schema.properties {
        for sv in props.values_mut() {
            rewrite_value(sv, base);
        }
    }
    if let Some(ref mut props) = schema.pattern_properties {
        for sv in props.values_mut() {
            rewrite_value(sv, base);
        }
    }
    if let Some(ref mut deps) = schema.dependent_schemas {
        for sv in deps.values_mut() {
            rewrite_value(sv, base);
        }
    }

    // $defs
    if let Some(ref mut defs) = schema.defs {
        for sv in defs.values_mut() {
            rewrite_value(sv, base);
        }
    }

    // Array fields
    for arr in [
        schema.all_of.as_mut(),
        schema.any_of.as_mut(),
        schema.one_of.as_mut(),
        schema.prefix_items.as_mut(),
    ]
    .into_iter()
    .flatten()
    {
        for sv in arr.iter_mut() {
            rewrite_value(sv, base);
        }
    }

    // Single fields
    for sv in [
        schema.items.as_deref_mut(),
        schema.contains.as_deref_mut(),
        schema.additional_properties.as_deref_mut(),
        schema.property_names.as_deref_mut(),
        schema.unevaluated_properties.as_deref_mut(),
        schema.unevaluated_items.as_deref_mut(),
        schema.not.as_deref_mut(),
        schema.if_.as_deref_mut(),
        schema.then_.as_deref_mut(),
        schema.else_.as_deref_mut(),
        schema.content_schema.as_deref_mut(),
    ]
    .into_iter()
    .flatten()
    {
        rewrite_value(sv, base);
    }
}

fn rewrite_value(sv: &mut SchemaValue, base: &str) {
    if let SchemaValue::Schema(s) = sv {
        rewrite_schema(s, base);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(val: serde_json::Value) -> Schema {
        serde_json::from_value(val).unwrap()
    }

    #[test]
    fn no_id_returns_unchanged() {
        let s = schema(json!({
            "type": "object",
            "properties": {
                "x": { "$ref": "#/$defs/Foo" }
            }
        }));
        let result = make_absolute(&s);
        let prop = result.properties.unwrap();
        let r = prop["x"].as_schema().unwrap().ref_.as_deref();
        assert_eq!(r, Some("#/$defs/Foo"));
    }

    #[test]
    fn rewrites_property_refs() {
        let s = schema(json!({
            "$id": "https://example.com/schema.json",
            "type": "object",
            "properties": {
                "x": { "$ref": "#/$defs/Foo" }
            }
        }));
        let result = make_absolute(&s);
        let prop = result.properties.unwrap();
        let r = prop["x"].as_schema().unwrap().ref_.as_deref();
        assert_eq!(r, Some("https://example.com/schema.json#/$defs/Foo"));
    }

    #[test]
    fn rewrites_allof_refs() {
        let s = schema(json!({
            "$id": "https://example.com/s.json",
            "allOf": [
                { "$ref": "#/$defs/A" },
                { "$ref": "#/$defs/B" }
            ]
        }));
        let result = make_absolute(&s);
        let all_of = result.all_of.unwrap();
        assert_eq!(
            all_of[0].as_schema().unwrap().ref_.as_deref(),
            Some("https://example.com/s.json#/$defs/A")
        );
        assert_eq!(
            all_of[1].as_schema().unwrap().ref_.as_deref(),
            Some("https://example.com/s.json#/$defs/B")
        );
    }

    #[test]
    fn external_refs_unchanged() {
        let s = schema(json!({
            "$id": "https://example.com/s.json",
            "properties": {
                "x": { "$ref": "https://other.com/schema.json#/foo" }
            }
        }));
        let result = make_absolute(&s);
        let prop = result.properties.unwrap();
        let r = prop["x"].as_schema().unwrap().ref_.as_deref();
        assert_eq!(r, Some("https://other.com/schema.json#/foo"));
    }

    #[test]
    fn rewrites_nested_refs() {
        let s = schema(json!({
            "$id": "https://example.com/s.json",
            "properties": {
                "outer": {
                    "type": "object",
                    "properties": {
                        "inner": { "$ref": "#/$defs/Deep" }
                    }
                }
            }
        }));
        let result = make_absolute(&s);
        let props = result.properties.unwrap();
        let outer = props["outer"].as_schema().unwrap();
        let outer_props = outer.properties.as_ref().unwrap();
        let inner_ref = outer_props["inner"].as_schema().unwrap().ref_.as_deref();
        assert_eq!(inner_ref, Some("https://example.com/s.json#/$defs/Deep"));
    }
}
