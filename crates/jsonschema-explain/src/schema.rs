use serde_json::Value;

use crate::fmt::{Fmt, format_type};

/// Extract the trailing name from a `$ref` path (e.g. `"#/$defs/Foo"` -> `"Foo"`).
pub(crate) fn ref_name(ref_str: &str) -> &str {
    ref_str.rsplit('/').next().unwrap_or(ref_str)
}

/// Resolve a `$ref` within the same schema document.
pub(crate) fn resolve_ref<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
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
                Some(item_ty) => Some(format!("array of {item_ty}")),
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

    if let Some(title) = resolved.get("title").and_then(Value::as_str) {
        let ty = schema_type_str(resolved).unwrap_or_default();
        if ty.is_empty() {
            return format!("{}{title}{}", f.bold, f.reset);
        }
        return format!("{}{title}{} ({})", f.bold, f.reset, format_type(&ty, f));
    }

    if let Some(desc) = get_description(resolved) {
        let ty = schema_type_str(resolved).unwrap_or_default();
        let rendered = if f.is_color() {
            markdown_to_ansi::render_inline(desc, &f.md_opts(None))
        } else {
            desc.to_string()
        };
        if ty.is_empty() {
            return rendered;
        }
        return format!("{} - {rendered}", format_type(&ty, f));
    }

    if let Some(r) = variant.get("$ref").and_then(Value::as_str) {
        if r.starts_with("#/") {
            return format!("{}{}{}", f.cyan, ref_name(r), f.reset);
        }
        return format!("{}(see: {r}){}", f.dim, f.reset);
    }

    if let Some(ty) = schema_type_str(resolved) {
        return format_type(&ty, f);
    }

    format!("{}(schema){}", f.dim, f.reset)
}
