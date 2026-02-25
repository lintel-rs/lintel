use core::fmt::Write;

use serde_json::Value;

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type, format_type_suffix, format_value};
use crate::man::write_description;
use crate::man::write_label;
use crate::schema::{
    get_description, ref_name, required_set, resolve_ref, schema_type_str, variant_summary,
};

/// Maximum nesting depth for recursive property rendering.
pub(crate) const MAX_DEPTH: usize = 3;

/// Render a variant block for `oneOf`/`anyOf`/`allOf`.
///
/// If the resolved variant has properties or a description, expand
/// them inline. Otherwise, render a single summary line.
#[allow(clippy::too_many_arguments)]
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

    let dep_tag = deprecated_tag(resolved, f);

    if has_properties || desc.is_some() {
        // Expanded block
        let ty = schema_type_str(resolved).unwrap_or_default();
        let suffix = format_type_suffix(&ty, f);
        let _ = writeln!(
            out,
            "    {}({index}){} {}{label}{}{dep_tag}{suffix}",
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
#[allow(clippy::too_many_arguments)]
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

    // Sort: required first, then normal, then deprecated — preserving
    // relative order within each group.
    let mut sorted_props: Vec<_> = props.iter().collect();
    sorted_props.sort_by_key(|(name, schema)| {
        let deprecated = resolve_ref(schema, root)
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // 0 = required, 1 = normal, 2 = deprecated
        i32::from(deprecated) * 2 + i32::from(!required.contains(name))
    });

    for (prop_name, prop_schema) in sorted_props {
        let prop_schema = resolve_ref(prop_schema, root);
        let ty = schema_type_str(prop_schema).unwrap_or_default();
        let is_required = required.contains(prop_name);
        let is_deprecated = prop_schema
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let type_display = format_type(&ty, f);
        let req_tag = if is_required {
            format!(", {}*required{}", f.red, f.reset)
        } else {
            String::new()
        };
        let deprecated_tag = if is_deprecated {
            format!(" {}[DEPRECATED]{}", f.dim, f.reset)
        } else {
            String::new()
        };

        let _ = writeln!(
            out,
            "{indent}{}{prop_name}{}{deprecated_tag} ({type_display}{req_tag})",
            f.green, f.reset
        );

        render_property_details(out, prop_schema, root, f, depth, &desc_indent);
        out.push('\n');
    }
}

/// Render details for a single property: description, default, enum, const,
/// variant lists, nested properties, and array item types.
#[allow(clippy::too_many_arguments)]
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
        write_label(
            out,
            desc_indent,
            "Default",
            &format!("{}{}{}", f.magenta, format_value(default), f.reset),
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
        write_label(out, desc_indent, "Values", &joined);
    }

    if let Some(c) = prop_schema.get("const") {
        write_label(
            out,
            desc_indent,
            "Constant",
            &format!("{}{c}{}", f.magenta, f.reset),
        );
    }

    if let Some(examples) = prop_schema.get("examples").and_then(Value::as_array)
        && !examples.is_empty()
    {
        let joined: String = examples
            .iter()
            .map(|v| {
                let display = format_value(v);
                format!("{}{display}{}", f.magenta, f.reset)
            })
            .collect::<Vec<_>>()
            .join(", ");
        write_label(out, desc_indent, "Examples", &joined);
    }

    render_constraints(out, prop_schema, f, desc_indent);

    for keyword in COMPOSITION_KEYWORDS {
        if let Some(variants) = prop_schema.get(*keyword).and_then(Value::as_array) {
            let label = match *keyword {
                "oneOf" => "One of",
                "anyOf" => "Any of",
                "allOf" => "All of",
                _ => keyword,
            };
            let _ = writeln!(out, "{desc_indent}{}{label}:{}", f.dim, f.reset);
            for (i, variant) in variants.iter().enumerate() {
                let resolved = resolve_ref(variant, root);
                render_inline_variant(out, resolved, variant, root, f, depth, desc_indent, i + 1);
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
}

/// Render a variant inline within a property's composition list.
///
/// `$ref` variants are always shown as one-line references (the DEFINITIONS
/// section has the full details). Non-ref variants with properties are
/// expanded inline when depth allows.
#[allow(clippy::too_many_arguments)]
fn render_inline_variant(
    out: &mut String,
    resolved: &Value,
    original: &Value,
    root: &Value,
    f: &Fmt<'_>,
    depth: usize,
    desc_indent: &str,
    index: usize,
) {
    let is_ref = original.get("$ref").is_some();
    let has_properties = resolved
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());

    // $ref variants are kept as one-line references; non-ref variants with
    // properties are expanded when depth allows.
    if !is_ref && has_properties && depth < MAX_DEPTH {
        let deprecated_tag = deprecated_tag(resolved, f);
        let (label, label_is_type) =
            if let Some(title) = resolved.get("title").and_then(Value::as_str) {
                (title.to_string(), false)
            } else if let Some(ty) = schema_type_str(resolved) {
                (ty, true)
            } else {
                (format!("variant {index}"), false)
            };
        let suffix = if label_is_type {
            String::new()
        } else {
            let ty = schema_type_str(resolved).unwrap_or_default();
            format_type_suffix(&ty, f)
        };
        let _ = writeln!(
            out,
            "{desc_indent}  {}({index}){} {}{label}{}{deprecated_tag}{suffix}",
            f.dim, f.reset, f.green, f.reset
        );
        if let Some(desc) = get_description(resolved) {
            let nested_indent = format!("{desc_indent}      ");
            write_description(out, desc, f, &nested_indent);
        }
        if let Some(props) = resolved.get("properties").and_then(Value::as_object) {
            let req = required_set(resolved);
            render_properties(out, props, &req, root, f, depth + 2);
        }
    } else {
        let summary = variant_summary(original, root, f);
        let _ = writeln!(out, "{desc_indent}  - {summary}");
    }
}

/// Return a `" [DEPRECATED]"` tag if the schema has `"deprecated": true`.
fn deprecated_tag(schema: &Value, f: &Fmt<'_>) -> String {
    if schema
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        format!(" {}[DEPRECATED]{}", f.dim, f.reset)
    } else {
        String::new()
    }
}

/// Render JSON Schema validation constraints (numeric bounds, string length,
/// pattern, array items, format, etc.) as a compact annotation line.
fn render_constraints(out: &mut String, schema: &Value, f: &Fmt<'_>, indent: &str) {
    let mut parts: Vec<String> = Vec::new();

    // Format
    if let Some(fmt_val) = schema.get("format").and_then(Value::as_str) {
        parts.push(format!("format={}{fmt_val}{}", f.magenta, f.reset));
    }

    // String constraints
    if let Some(v) = schema.get("minLength").and_then(Value::as_u64) {
        parts.push(format!("minLength={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("maxLength").and_then(Value::as_u64) {
        parts.push(format!("maxLength={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("pattern").and_then(Value::as_str) {
        parts.push(format!("pattern={}{v}{}", f.magenta, f.reset));
    }

    // Numeric constraints
    if let Some(v) = schema.get("minimum") {
        parts.push(format!("min={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("maximum") {
        parts.push(format!("max={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("exclusiveMinimum") {
        parts.push(format!("exclusiveMin={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("exclusiveMaximum") {
        parts.push(format!("exclusiveMax={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("multipleOf") {
        parts.push(format!("multipleOf={}{v}{}", f.magenta, f.reset));
    }

    // Array constraints
    if let Some(v) = schema.get("minItems").and_then(Value::as_u64) {
        parts.push(format!("minItems={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("maxItems").and_then(Value::as_u64) {
        parts.push(format!("maxItems={}{v}{}", f.magenta, f.reset));
    }
    if schema
        .get("uniqueItems")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        parts.push(format!("{}unique{}", f.magenta, f.reset));
    }

    // Object constraints
    if let Some(v) = schema.get("minProperties").and_then(Value::as_u64) {
        parts.push(format!("minProperties={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.get("maxProperties").and_then(Value::as_u64) {
        parts.push(format!("maxProperties={}{v}{}", f.magenta, f.reset));
    }

    if !parts.is_empty() {
        let _ = writeln!(
            out,
            "{indent}{}Constraints:{} {}",
            f.dim,
            f.reset,
            parts.join(", ")
        );
    }
}

/// Render a sub-schema summary at a given depth.
#[allow(clippy::too_many_arguments)]
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
        write_label(out, &indent, "Type", &format_type(&ty, f));
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
