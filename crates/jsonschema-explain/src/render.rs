use core::fmt::Write;

use serde_json::Value;

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type, format_type_suffix, format_value};
use crate::schema::{
    get_description, ref_name, required_set, resolve_ref, schema_type_str, variant_summary,
};
use crate::write_description;

/// Maximum nesting depth for recursive property rendering.
pub(crate) const MAX_DEPTH: usize = 3;

/// Render a variant block for `oneOf`/`anyOf`/`allOf`.
///
/// If the resolved variant has properties or a description, expand
/// them inline. Otherwise, render a single summary line.
pub(crate) fn render_variant_block(
    out: &mut String,
    resolved: &Value,
    original: &Value,
    root: &Value,
    f: &Fmt<'_>,
    index: usize,
) {
    let label = if let Some(title) = resolved.get("title").and_then(Value::as_str) {
        title.to_string()
    } else if let Some(r) = original.get("$ref").and_then(Value::as_str) {
        ref_name(r).to_string()
    } else if let Some(ty) = schema_type_str(resolved) {
        ty
    } else {
        format!("variant {index}")
    };

    let has_properties = resolved
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());
    let desc = get_description(resolved);

    if has_properties || desc.is_some() {
        // Expanded block
        let ty = schema_type_str(resolved).unwrap_or_default();
        let suffix = format_type_suffix(&ty, f);
        let _ = writeln!(
            out,
            "    {}({index}){} {}{label}{}{suffix}",
            f.dim, f.reset, f.green, f.reset
        );
        if let Some(desc) = desc {
            write_description(out, desc, f, "        ");
        }
        if let Some(props) = resolved.get("properties").and_then(Value::as_object) {
            let req = required_set(resolved);
            render_properties(out, props, &req, root, f, 2);
        }
    } else {
        // Single-line summary
        let summary = variant_summary(original, root, f);
        let _ = writeln!(out, "    {}({index}){} {summary}", f.dim, f.reset);
    }
}

/// Render properties at a given indentation depth.
pub(crate) fn render_properties(
    out: &mut String,
    props: &serde_json::Map<String, Value>,
    required: &[String],
    root: &Value,
    f: &Fmt<'_>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let desc_indent = format!("{indent}    ");

    // Sort required fields first, preserving relative order within each group.
    let mut sorted_props: Vec<_> = props.iter().collect();
    sorted_props.sort_by_key(|(name, _)| i32::from(!required.contains(name)));

    for (prop_name, prop_schema) in sorted_props {
        let prop_schema = resolve_ref(prop_schema, root);
        let ty = schema_type_str(prop_schema).unwrap_or_default();
        let is_required = required.contains(prop_name);
        let type_display = format_type(&ty, f);
        let req_tag = if is_required {
            format!(", {}*required{}", f.red, f.reset)
        } else {
            String::new()
        };

        let _ = writeln!(
            out,
            "{indent}{}{prop_name}{} ({type_display}{req_tag})",
            f.green, f.reset
        );

        render_property_details(out, prop_schema, root, f, depth, &desc_indent);
        out.push('\n');
    }
}

/// Render details for a single property: description, default, enum, const,
/// variant lists, nested properties, and array item types.
fn render_property_details(
    out: &mut String,
    prop_schema: &Value,
    root: &Value,
    f: &Fmt<'_>,
    depth: usize,
    desc_indent: &str,
) {
    if let Some(desc) = get_description(prop_schema) {
        write_description(out, desc, f, desc_indent);
    }

    if let Some(default) = prop_schema.get("default") {
        let _ = writeln!(
            out,
            "{desc_indent}{}Default:{} {}{}{}",
            f.dim,
            f.reset,
            f.magenta,
            format_value(default),
            f.reset
        );
    }

    if let Some(values) = prop_schema.get("enum").and_then(Value::as_array) {
        let joined: String = values
            .iter()
            .map(|v| {
                let display = v.as_str().map_or_else(|| v.to_string(), str::to_string);
                format!("{}{display}{}", f.magenta, f.reset)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "{desc_indent}{}Values:{} {joined}", f.dim, f.reset);
    }

    if let Some(c) = prop_schema.get("const") {
        let _ = writeln!(
            out,
            "{desc_indent}{}Constant:{} {}{c}{}",
            f.dim, f.reset, f.magenta, f.reset
        );
    }

    for keyword in COMPOSITION_KEYWORDS {
        if let Some(variants) = prop_schema.get(*keyword).and_then(Value::as_array) {
            let label = match *keyword {
                "oneOf" => "One of",
                "anyOf" => "Any of",
                "allOf" => "All of",
                _ => keyword,
            };
            let _ = writeln!(out, "{desc_indent}{}{label}:{}", f.dim, f.reset);
            for variant in variants {
                let summary = variant_summary(variant, root, f);
                let _ = writeln!(out, "{desc_indent}  - {summary}");
            }
        }
    }

    if depth < MAX_DEPTH
        && let Some(nested_props) = prop_schema.get("properties").and_then(Value::as_object)
    {
        let nested_required = required_set(prop_schema);
        out.push('\n');
        render_properties(out, nested_props, &nested_required, root, f, depth + 1);
    }

    if prop_schema.get("type").and_then(Value::as_str) == Some("array")
        && let Some(items) = prop_schema.get("items")
    {
        let items = resolve_ref(items, root);
        let item_ty = schema_type_str(items).unwrap_or_default();
        if !item_ty.is_empty() {
            let _ = writeln!(
                out,
                "{desc_indent}{}Items:{} {}",
                f.dim,
                f.reset,
                format_type(&item_ty, f)
            );
        }
    }
}

/// Render a sub-schema summary at a given depth.
pub(crate) fn render_subschema(
    out: &mut String,
    schema: &Value,
    root: &Value,
    f: &Fmt<'_>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let schema = resolve_ref(schema, root);
    let ty = schema_type_str(schema).unwrap_or_default();

    if !ty.is_empty() {
        let _ = writeln!(
            out,
            "{indent}{}Type:{} {}",
            f.dim,
            f.reset,
            format_type(&ty, f)
        );
    }

    if let Some(desc) = get_description(schema) {
        write_description(out, desc, f, &indent);
    }

    if depth < MAX_DEPTH {
        let required = required_set(schema);
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            render_properties(out, props, &required, root, f, depth + 1);
        }
    }
}
