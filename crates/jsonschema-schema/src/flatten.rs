use crate::schema::{Schema, SchemaValue, navigate_pointer};

/// A schema with its `allOf` entries merged into the root, plus provenance info.
pub struct FlattenedSchema {
    /// The merged schema (allOf entries folded in).
    pub schema: Schema,
    /// Metadata about each sub-schema that was merged.
    pub includes: Vec<IncludedSchema>,
}

/// Metadata about a sub-schema that was merged via `allOf`.
pub struct IncludedSchema {
    /// The title of the sub-schema (or a fallback label).
    pub title: String,
    /// Short type string (e.g. "object | boolean").
    pub type_str: Option<String>,
    /// Source URL if available (from `x-lintel.source` or `$id`).
    pub source: Option<String>,
}

/// Flatten `allOf` entries into the root schema.
///
/// Clones the schema, takes the `all_of` entries out, resolves any `$ref`
/// pointers, extracts provenance metadata, and merges each entry into the
/// root using `Schema`'s `Add` implementation (left-bias).
///
/// Returns the merged schema and a list of included sub-schemas.
/// If the schema has no `allOf`, returns it unchanged with an empty includes list.
pub fn flatten_all_of(schema: &Schema, root: &SchemaValue) -> FlattenedSchema {
    let mut merged = schema.clone();
    let Some(all_of) = merged.all_of.take() else {
        return FlattenedSchema {
            schema: merged,
            includes: Vec::new(),
        };
    };

    let mut includes = Vec::new();

    for entry in all_of {
        // Resolve $ref if present
        let resolved_sv = resolve_entry(&entry, root);
        let Some(resolved) = resolved_sv.as_schema() else {
            continue;
        };

        // Extract provenance metadata
        let title = resolved
            .title
            .clone()
            .or_else(|| {
                // Fall back to $ref name if the entry was a $ref
                entry
                    .as_schema()
                    .and_then(|s| s.ref_.as_ref())
                    .map(|r| crate::schema::ref_name(r).to_string())
            })
            .unwrap_or_else(|| "(anonymous)".to_string());

        let type_str = resolved.type_str();
        let source = resolved
            .x_lintel
            .as_ref()
            .and_then(|xl| xl.source.clone())
            .or_else(|| resolved.id.clone());

        includes.push(IncludedSchema {
            title,
            type_str,
            source,
        });

        // Merge: root (left) wins over the allOf entry (right)
        let entry_schema = resolved.clone();
        // Strip fields we don't want to merge from the entry
        let mut clean = entry_schema;
        // Don't carry over the entry's allOf/anyOf/oneOf into the merged result
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

    FlattenedSchema {
        schema: merged,
        includes,
    }
}

/// Resolve a `$ref` entry against the root schema.
fn resolve_entry<'a>(entry: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
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
        assert!(result.includes.is_empty());
        assert_eq!(result.schema.title.as_deref(), Some("Root"));
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

        assert_eq!(result.includes.len(), 1);
        assert_eq!(result.includes[0].title, "Extra");

        let props = result.schema.properties.unwrap();
        assert!(props.contains_key("a"));
        assert!(props.contains_key("b"));
        // allOf should be cleared
        assert!(result.schema.all_of.is_none());
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

        assert_eq!(result.includes.len(), 1);
        assert_eq!(result.includes[0].title, "Base Schema");
        let props = result.schema.properties.unwrap();
        assert!(props.contains_key("name"));
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

        let props = result.schema.properties.unwrap();
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

        let req = result.schema.required.unwrap();
        assert!(req.contains(&"a".to_string()));
        assert!(req.contains(&"b".to_string()));
        assert!(req.contains(&"c".to_string()));
        assert_eq!(req.len(), 3);
    }

    #[test]
    fn includes_provenance_with_source() {
        let val = json!({
            "allOf": [
                { "$ref": "#/$defs/Core" }
            ],
            "$defs": {
                "Core": {
                    "title": "Core vocabulary",
                    "type": ["object", "boolean"],
                    "x-lintel": {
                        "source": "https://json-schema.org/draft/2020-12/meta/core"
                    }
                }
            }
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        assert_eq!(result.includes.len(), 1);
        assert_eq!(result.includes[0].title, "Core vocabulary");
        assert_eq!(
            result.includes[0].source.as_deref(),
            Some("https://json-schema.org/draft/2020-12/meta/core")
        );
    }
}
