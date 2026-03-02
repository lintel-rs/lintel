use jsonschema_schema::{Schema, SchemaValue, ref_name};

use crate::fmt::{Fmt, format_type};

/// Resolve a `$ref` within the same schema document.
pub fn resolve_ref<'a>(sv: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
    let Some(schema) = sv.as_schema() else {
        return sv;
    };
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix('#')
        && let Ok(resolved) = jsonschema_schema::navigate_pointer(root, root, path)
    {
        return resolved;
    }
    sv
}

/// Walk a JSON Pointer path through a schema, resolving `$ref` at each step.
///
/// # Errors
///
/// Returns an error if a segment in the pointer cannot be resolved.
pub fn navigate_pointer<'a>(
    schema: &'a SchemaValue,
    root: &'a SchemaValue,
    pointer: &str,
) -> Result<&'a SchemaValue, String> {
    jsonschema_schema::navigate_pointer(schema, root, pointer)
}

/// Extract the `required` array from a schema as a list of strings.
pub(crate) fn required_set(schema: &Schema) -> Vec<String> {
    schema.required_set().to_vec()
}

/// Produce a short human-readable type string for a schema.
pub(crate) fn schema_type_str(schema: &Schema) -> Option<String> {
    schema.type_str()
}

/// Get the best description text from a schema, preferring `markdownDescription`.
pub(crate) fn get_description(schema: &Schema) -> Option<&str> {
    schema.description()
}

/// Produce a one-line summary of a variant schema for `oneOf`/`anyOf`/`allOf` listings.
pub(crate) fn variant_summary(variant: &SchemaValue, root: &SchemaValue, f: &Fmt<'_>) -> String {
    let resolved_sv = resolve_ref(variant, root);
    let Some(resolved) = resolved_sv.as_schema() else {
        return format!("{}(schema){}", f.dim, f.reset);
    };

    let dep = if resolved.is_deprecated() {
        format!(" {}[DEPRECATED]{}", f.dim, f.reset)
    } else {
        String::new()
    };

    // Title first — best label for any variant.
    if let Some(title) = resolved.title.as_deref() {
        let ty = schema_type_str(resolved).unwrap_or_default();
        if ty.is_empty() {
            return format!("{}{title}{}{dep}", f.bold, f.reset);
        }
        return format!(
            "{}{title}{}{dep} ({})",
            f.bold,
            f.reset,
            format_type(&ty, f)
        );
    }

    // $ref variants without a title: show the ref name — DEFINITIONS has details.
    if let Some(schema) = variant.as_schema()
        && let Some(ref r) = schema.ref_
    {
        if r.starts_with("#/") {
            return format!("{}{}{}{dep}", f.cyan, ref_name(r), f.reset);
        }
        return format!("{}(see: {r}){}{dep}", f.dim, f.reset);
    }

    if let Some(desc) = get_description(resolved) {
        let first_line = first_sentence(desc);
        let ty = schema_type_str(resolved).unwrap_or_default();
        let rendered = if f.is_color() {
            markdown_to_ansi::render_inline(first_line, &f.md_opts(None))
        } else {
            first_line.to_string()
        };
        if ty.is_empty() {
            return format!("{rendered}{dep}");
        }
        return format!("{} - {rendered}{dep}", format_type(&ty, f));
    }

    if let Some(ty) = schema_type_str(resolved) {
        return format!("{}{dep}", format_type(&ty, f));
    }

    format!("{}(schema){}{dep}", f.dim, f.reset)
}

/// Extract the first sentence or line from a description for one-line summaries.
fn first_sentence(desc: &str) -> &str {
    // Use the first line break (paragraph boundary) if present.
    let trimmed = desc.trim();
    if let Some(pos) = trimmed.find("\n\n") {
        let first = trimmed[..pos].trim();
        if !first.is_empty() {
            return first;
        }
    }
    if let Some(pos) = trimmed.find('\n') {
        let first = trimmed[..pos].trim();
        if !first.is_empty() {
            return first;
        }
    }
    trimmed
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Parse with migration so tests work with older JSON Schema drafts.
    fn sv(mut val: serde_json::Value) -> SchemaValue {
        jsonschema_migrate::migrate_to_2020_12(&mut val);
        serde_json::from_value(val).unwrap()
    }

    // --- navigate_pointer ---

    #[test]
    fn navigate_empty_pointer_returns_schema() {
        let schema = sv(json!({"type": "object"}));
        let result = navigate_pointer(&schema, &schema, "").unwrap();
        assert!(result.as_schema().is_some());
    }

    #[test]
    fn navigate_root_slash_returns_schema() {
        let schema = sv(json!({"type": "object"}));
        let result = navigate_pointer(&schema, &schema, "/").unwrap();
        assert!(result.as_schema().is_some());
    }

    #[test]
    fn navigate_single_segment() {
        let schema = sv(json!({
            "properties": {
                "name": { "type": "string" }
            }
        }));
        let result = navigate_pointer(&schema, &schema, "/properties/name").unwrap();
        let s = result.as_schema().unwrap();
        assert!(s.type_str().as_deref() == Some("string"));
    }

    #[test]
    fn navigate_nested_segments() {
        let schema = sv(json!({
            "properties": {
                "name": { "type": "string", "description": "The name" }
            }
        }));
        let result = navigate_pointer(&schema, &schema, "/properties/name").unwrap();
        let s = result.as_schema().unwrap();
        assert_eq!(s.description.as_deref(), Some("The name"));
    }

    #[test]
    fn navigate_resolves_ref_at_each_step() {
        let schema = sv(json!({
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "description": "An item"
                }
            }
        }));
        let result = navigate_pointer(&schema, &schema, "/properties/item").unwrap();
        let s = result.as_schema().unwrap();
        assert_eq!(s.description.as_deref(), Some("An item"));
    }

    #[test]
    fn navigate_through_ref_then_deeper() {
        let schema = sv(json!({
            "properties": {
                "config": { "$ref": "#/$defs/Config" }
            },
            "$defs": {
                "Config": {
                    "type": "object",
                    "properties": {
                        "debug": { "type": "boolean" }
                    }
                }
            }
        }));
        let result =
            navigate_pointer(&schema, &schema, "/properties/config/properties/debug").unwrap();
        let s = result.as_schema().unwrap();
        assert!(s.type_str().as_deref() == Some("boolean"));
    }

    #[test]
    fn navigate_array_index() {
        let schema = sv(json!({
            "oneOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        }));
        let result = navigate_pointer(&schema, &schema, "/oneOf/1").unwrap();
        let s = result.as_schema().unwrap();
        assert!(s.type_str().as_deref() == Some("integer"));
    }

    #[test]
    fn navigate_missing_segment_errors() {
        let schema = sv(json!({"type": "object"}));
        let err = navigate_pointer(&schema, &schema, "/nonexistent").unwrap_err();
        assert!(err.contains("nonexistent"), "error was: {err}");
    }

    #[test]
    fn navigate_defs_directly() {
        let schema = sv(json!({
            "$defs": {
                "Foo": { "type": "string" }
            }
        }));
        let result = navigate_pointer(&schema, &schema, "/$defs/Foo").unwrap();
        let s = result.as_schema().unwrap();
        assert!(s.type_str().as_deref() == Some("string"));
    }

    // --- resolve_ref ---

    #[test]
    fn resolve_ref_no_ref_returns_self() {
        let schema = sv(json!({"type": "string"}));
        let result = resolve_ref(&schema, &schema);
        assert!(result.as_schema().is_some());
    }

    #[test]
    fn resolve_ref_follows_local_ref() {
        let root = sv(json!({
            "$defs": {
                "Name": { "type": "string" }
            }
        }));
        let schema = sv(json!({"$ref": "#/$defs/Name"}));
        let resolved = resolve_ref(&schema, &root);
        let s = resolved.as_schema().unwrap();
        assert!(s.type_str().as_deref() == Some("string"));
    }

    #[test]
    fn resolve_ref_missing_target_returns_self() {
        let root = sv(json!({"$defs": {}}));
        let schema = sv(json!({"$ref": "#/$defs/Missing"}));
        let resolved = resolve_ref(&schema, &root);
        let s = resolved.as_schema().unwrap();
        assert!(s.ref_.is_some());
    }

    #[test]
    fn resolve_ref_external_ref_returns_self() {
        let root = sv(json!({}));
        let schema = sv(json!({"$ref": "https://example.com/schema.json"}));
        let resolved = resolve_ref(&schema, &root);
        let s = resolved.as_schema().unwrap();
        assert!(s.ref_.is_some());
    }
}
