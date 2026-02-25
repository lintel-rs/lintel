use serde_json::Value;

use crate::fmt::{Fmt, format_type};

/// Extract the trailing name from a `$ref` path (e.g. `"#/$defs/Foo"` -> `"Foo"`).
pub(crate) fn ref_name(ref_str: &str) -> &str {
    ref_str.rsplit('/').next().unwrap_or(ref_str)
}

/// Resolve a `$ref` within the same schema document.
pub fn resolve_ref<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str)
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        let mut current = root;
        for segment in path.split('/') {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            match current {
                Value::Object(map) => {
                    if let Some(next) = map.get(&decoded) {
                        current = next;
                    } else {
                        return schema;
                    }
                }
                _ => return schema,
            }
        }
        return current;
    }
    schema
}

/// Walk a JSON Pointer path through a schema, resolving `$ref` at each step.
///
/// Segments are decoded per RFC 6901 (`~1` → `/`, `~0` → `~`).
/// Returns the sub-schema at the given pointer, or an error describing
/// which segment could not be resolved.
///
/// # Errors
///
/// Returns an error if a segment in the pointer cannot be resolved within the
/// schema (i.e. the key does not exist or the array index is out of bounds).
pub fn navigate_pointer<'a>(
    schema: &'a Value,
    root: &'a Value,
    pointer: &str,
) -> Result<&'a Value, String> {
    let path = pointer.strip_prefix('/').unwrap_or(pointer);
    if path.is_empty() {
        return Ok(schema);
    }

    let mut current = resolve_ref(schema, root);

    for segment in path.split('/') {
        let decoded = segment.replace("~1", "/").replace("~0", "~");
        current = resolve_ref(current, root);

        // Try direct object key lookup first
        if let Some(next) = current.get(&decoded) {
            current = next;
            continue;
        }

        // Try as an array index
        if let Value::Array(arr) = current
            && let Ok(idx) = decoded.parse::<usize>()
            && let Some(next) = arr.get(idx)
        {
            current = next;
            continue;
        }

        return Err(format!(
            "cannot resolve segment '{decoded}' in pointer '{pointer}'"
        ));
    }

    Ok(resolve_ref(current, root))
}

/// Extract the `required` array from a schema as a list of strings.
pub(crate) fn required_set(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Produce a short human-readable type string for a schema.
pub(crate) fn schema_type_str(schema: &Value) -> Option<String> {
    // Explicit type field
    if let Some(ty) = schema.get("type") {
        return match ty {
            Value::String(s) if s == "array" => match schema.get("items").and_then(schema_type_str)
            {
                Some(item_ty) => Some(format!("{item_ty}[]")),
                None => Some("array".to_string()),
            },
            Value::String(s) => Some(s.clone()),
            Value::Array(arr) => {
                let types: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
                Some(types.join(" | "))
            }
            _ => None,
        };
    }

    // $ref
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        return Some(ref_name(r).to_string());
    }

    // oneOf/anyOf
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let types: Vec<String> = variants
                .iter()
                .filter_map(|v| {
                    schema_type_str(v).or_else(|| {
                        v.get("$ref")
                            .and_then(Value::as_str)
                            .map(|r| ref_name(r).to_string())
                    })
                })
                .collect();
            if !types.is_empty() {
                return Some(types.join(" | "));
            }
        }
    }

    // const
    if let Some(c) = schema.get("const") {
        return Some(format!("const: {c}"));
    }

    // enum
    if schema.get("enum").is_some() {
        return Some("enum".to_string());
    }

    None
}

/// Get the best description text from a schema, preferring `markdownDescription`.
pub(crate) fn get_description(schema: &Value) -> Option<&str> {
    schema
        .get("markdownDescription")
        .and_then(Value::as_str)
        .or_else(|| schema.get("description").and_then(Value::as_str))
}

/// Produce a one-line summary of a variant schema for `oneOf`/`anyOf`/`allOf` listings.
pub(crate) fn variant_summary(variant: &Value, root: &Value, f: &Fmt<'_>) -> String {
    let resolved = resolve_ref(variant, root);
    let dep = if resolved
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        format!(" {}[DEPRECATED]{}", f.dim, f.reset)
    } else {
        String::new()
    };

    // Title first — best label for any variant.
    if let Some(title) = resolved.get("title").and_then(Value::as_str) {
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
    if let Some(r) = variant.get("$ref").and_then(Value::as_str) {
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

    // --- navigate_pointer ---

    #[test]
    fn navigate_empty_pointer_returns_schema() {
        let schema = json!({"type": "object"});
        let result = navigate_pointer(&schema, &schema, "").unwrap();
        assert_eq!(result, &schema);
    }

    #[test]
    fn navigate_root_slash_returns_schema() {
        let schema = json!({"type": "object"});
        let result = navigate_pointer(&schema, &schema, "/").unwrap();
        assert_eq!(result, &schema);
    }

    #[test]
    fn navigate_single_segment() {
        let schema = json!({
            "properties": {
                "name": { "type": "string" }
            }
        });
        let result = navigate_pointer(&schema, &schema, "/properties").unwrap();
        assert_eq!(result, &json!({"name": {"type": "string"}}));
    }

    #[test]
    fn navigate_nested_segments() {
        let schema = json!({
            "properties": {
                "name": { "type": "string", "description": "The name" }
            }
        });
        let result = navigate_pointer(&schema, &schema, "/properties/name").unwrap();
        assert_eq!(
            result,
            &json!({"type": "string", "description": "The name"})
        );
    }

    #[test]
    fn navigate_resolves_ref_at_each_step() {
        let schema = json!({
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "description": "An item"
                }
            }
        });
        let result = navigate_pointer(&schema, &schema, "/properties/item").unwrap();
        assert_eq!(result, &json!({"type": "object", "description": "An item"}));
    }

    #[test]
    fn navigate_through_ref_then_deeper() {
        let schema = json!({
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
        });
        let result =
            navigate_pointer(&schema, &schema, "/properties/config/properties/debug").unwrap();
        assert_eq!(result, &json!({"type": "boolean"}));
    }

    #[test]
    fn navigate_array_index() {
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        let result = navigate_pointer(&schema, &schema, "/oneOf/1").unwrap();
        assert_eq!(result, &json!({"type": "integer"}));
    }

    #[test]
    fn navigate_missing_segment_errors() {
        let schema = json!({"type": "object"});
        let err = navigate_pointer(&schema, &schema, "/nonexistent").unwrap_err();
        assert!(err.contains("nonexistent"), "error was: {err}");
    }

    #[test]
    fn navigate_tilde_decoding() {
        // RFC 6901: ~0 -> ~, ~1 -> /
        let schema = json!({
            "properties": {
                "a/b": { "type": "string" },
                "c~d": { "type": "integer" }
            }
        });
        let result = navigate_pointer(&schema, &schema, "/properties/a~1b").unwrap();
        assert_eq!(result, &json!({"type": "string"}));

        let result = navigate_pointer(&schema, &schema, "/properties/c~0d").unwrap();
        assert_eq!(result, &json!({"type": "integer"}));
    }

    #[test]
    fn navigate_defs_directly() {
        let schema = json!({
            "$defs": {
                "Foo": { "type": "string" }
            }
        });
        let result = navigate_pointer(&schema, &schema, "/$defs/Foo").unwrap();
        assert_eq!(result, &json!({"type": "string"}));
    }

    // --- resolve_ref ---

    #[test]
    fn resolve_ref_no_ref_returns_self() {
        let schema = json!({"type": "string"});
        let root = json!({"type": "string"});
        assert_eq!(resolve_ref(&schema, &root), &schema);
    }

    #[test]
    fn resolve_ref_follows_local_ref() {
        let root = json!({
            "$defs": {
                "Name": { "type": "string" }
            }
        });
        let schema = json!({"$ref": "#/$defs/Name"});
        let resolved = resolve_ref(&schema, &root);
        assert_eq!(resolved, &json!({"type": "string"}));
    }

    #[test]
    fn resolve_ref_missing_target_returns_self() {
        let root = json!({"$defs": {}});
        let schema = json!({"$ref": "#/$defs/Missing"});
        let resolved = resolve_ref(&schema, &root);
        assert_eq!(resolved, &schema);
    }

    #[test]
    fn resolve_ref_external_ref_returns_self() {
        let root = json!({});
        let schema = json!({"$ref": "https://example.com/schema.json"});
        // External refs (no #/ prefix) are not resolved locally
        assert_eq!(resolve_ref(&schema, &root), &schema);
    }
}
