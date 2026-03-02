use alloc::collections::BTreeMap;

use crate::schema::{Schema, SchemaValue, navigate_pointer};

/// Flatten `allOf` entries into the root schema.
///
/// Clones the schema, resolves each `allOf` entry, merges its properties into
/// the root using `Schema`'s `Add` implementation (left-bias), and replaces
/// inline entries with `$ref` pointers into `$defs`.
///
/// The returned schema keeps `allOf` (now all `$ref` entries) so the ALL OF
/// section shows what was composed, while PROPERTIES shows the merged view.
pub fn flatten_all_of(schema: &Schema, root: &SchemaValue) -> Schema {
    let mut merged = schema.clone();
    let Some(all_of) = merged.all_of.take() else {
        return merged;
    };

    let mut new_all_of = Vec::new();

    for (i, entry) in all_of.into_iter().enumerate() {
        let is_ref = entry.as_schema().is_some_and(|s| s.ref_.is_some());

        // Resolve $ref against the original root (before mutations)
        let resolved = if is_ref {
            let resolved_sv = resolve_entry_in_root(&entry, root);
            let Some(s) = resolved_sv.as_schema() else {
                new_all_of.push(entry);
                continue;
            };
            s.clone()
        } else {
            let Some(s) = entry.as_schema() else {
                new_all_of.push(entry);
                continue;
            };
            s.clone()
        };

        // Build the allOf entry: keep existing $ref or create one for inline schemas
        let ref_entry = if is_ref {
            entry
        } else {
            // Inline schema — move it to $defs and replace with $ref
            let def_name = resolved
                .title
                .clone()
                .unwrap_or_else(|| format!("allOf-{i}"));

            let defs = merged.defs.get_or_insert_with(BTreeMap::new);
            defs.entry(def_name.clone()).or_insert(entry);

            SchemaValue::Schema(Box::new(Schema {
                ref_: Some(format!("#/$defs/{def_name}")),
                ..Default::default()
            }))
        };
        new_all_of.push(ref_entry);

        // Merge resolved properties into root (left-bias)
        let mut clean = resolved;
        // Don't carry over composition keywords
        clean.all_of = None;
        clean.any_of = None;
        clean.one_of = None;
        // Don't carry over identity fields from sub-schemas
        clean.schema = None;
        clean.id = None;
        clean.title = None;
        clean.description = None;
        clean.markdown_description = None;
        clean.x_lintel = None;

        merged = merged + clean;
    }

    merged.all_of = Some(new_all_of);
    merged
}

/// Resolve a `$ref` entry against the root schema.
fn resolve_entry_in_root<'a>(entry: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
    let Some(schema) = entry.as_schema() else {
        return entry;
    };
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix('#')
        && let Ok(resolved) = navigate_pointer(root, root, path)
    {
        return resolved;
    }
    entry
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sv(val: serde_json::Value) -> SchemaValue {
        serde_json::from_value(val).unwrap()
    }

    fn schema(val: serde_json::Value) -> Schema {
        serde_json::from_value(val).unwrap()
    }

    #[test]
    fn no_allof_returns_unchanged() {
        let s = schema(json!({"type": "object", "title": "Root"}));
        let root = sv(json!({"type": "object", "title": "Root"}));
        let result = flatten_all_of(&s, &root);
        assert!(result.all_of.is_none());
        assert_eq!(result.title.as_deref(), Some("Root"));
    }

    #[test]
    fn merges_inline_allof_properties() {
        let val = json!({
            "type": "object",
            "properties": {
                "a": { "type": "string" }
            },
            "allOf": [
                {
                    "title": "Extra",
                    "properties": {
                        "b": { "type": "integer" }
                    }
                }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        // Properties merged
        let props = result.properties.unwrap();
        assert!(props.contains_key("a"));
        assert!(props.contains_key("b"));

        // allOf kept with $ref entry
        let all_of = result.all_of.unwrap();
        assert_eq!(all_of.len(), 1);
        let ref_str = all_of[0].as_schema().unwrap().ref_.as_deref();
        assert_eq!(ref_str, Some("#/$defs/Extra"));

        // Inline schema moved to $defs
        let defs = result.defs.unwrap();
        assert!(defs.contains_key("Extra"));
    }

    #[test]
    fn merges_ref_allof() {
        let val = json!({
            "type": "object",
            "allOf": [
                { "$ref": "#/$defs/Base" }
            ],
            "$defs": {
                "Base": {
                    "title": "Base Schema",
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let props = result.properties.unwrap();
        assert!(props.contains_key("name"));

        // allOf kept as original $ref
        let all_of = result.all_of.unwrap();
        let ref_str = all_of[0].as_schema().unwrap().ref_.as_deref();
        assert_eq!(ref_str, Some("#/$defs/Base"));
    }

    #[test]
    fn root_properties_win_over_allof() {
        let val = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            },
            "allOf": [
                {
                    "properties": {
                        "x": { "type": "integer" }
                    }
                }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let props = result.properties.unwrap();
        let x_schema = props["x"].as_schema().unwrap();
        assert!(
            matches!(x_schema.type_, Some(crate::schema::TypeValue::Single(ref t)) if t == "string")
        );
    }

    #[test]
    fn required_union() {
        let val = json!({
            "required": ["a"],
            "allOf": [
                { "required": ["b", "a"] },
                { "required": ["c"] }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let req = result.required.unwrap();
        assert!(req.contains(&"a".to_string()));
        assert!(req.contains(&"b".to_string()));
        assert!(req.contains(&"c".to_string()));
        assert_eq!(req.len(), 3);
    }

    #[test]
    fn inline_without_title_uses_index_name() {
        let val = json!({
            "allOf": [
                { "properties": { "x": { "type": "string" } } }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let all_of = result.all_of.unwrap();
        let ref_str = all_of[0].as_schema().unwrap().ref_.as_deref();
        assert_eq!(ref_str, Some("#/$defs/allOf-0"));

        let defs = result.defs.unwrap();
        assert!(defs.contains_key("allOf-0"));
    }
}
