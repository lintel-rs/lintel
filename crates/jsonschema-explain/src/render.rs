use core::fmt::Write;

use indexmap::IndexMap;
use jsonschema_schema::{Schema, SchemaValue, ref_name};

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type, format_type_suffix, format_value};
use crate::man::write_description;
use crate::man::write_label;
use crate::schema::{get_description, required_set, resolve_ref, schema_type_str, variant_summary};

/// Maximum nesting depth for recursive property rendering.
pub(crate) const MAX_DEPTH: usize = 3;

/// Render a variant block for `oneOf`/`anyOf`/`allOf`.
///
/// If the resolved variant has properties or a description, expand
/// them inline. Otherwise, render a single summary line.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_variant_block(
    out: &mut String,
    resolved: &Schema,
    original: &SchemaValue,
    root: &SchemaValue,
    f: &Fmt<'_>,
    index: usize,
) {
    let label = if let Some(ref title) = resolved.title {
        title.clone()
    } else if let Some(orig_schema) = original.as_schema()
        && let Some(ref r) = orig_schema.ref_
    {
        ref_name(r).to_string()
    } else if let Some(ty) = schema_type_str(resolved) {
        ty
    } else {
        format!("variant {index}")
    };

    let has_properties = resolved.properties.as_ref().is_some_and(|p| !p.is_empty());
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
        if let Some(ref props) = resolved.properties {
            let req = required_set(resolved);
            render_properties(out, props, &req, root, f, 2);
        }
    } else if let Some(ref values) = resolved.enum_ {
        let prefix = format!("    {}({index}){} ", f.dim, f.reset);
        render_enum_values(out, values, f, &prefix);
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
    props: &IndexMap<String, SchemaValue>,
    required: &[String],
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let desc_indent = format!("{indent}    ");

    // Sort: required first, then normal, then deprecated — preserving
    // relative order within each group.
    let mut sorted_props: Vec<_> = props.iter().collect();
    sorted_props.sort_by_key(|(name, sv)| {
        let deprecated = resolve_ref(sv, root)
            .as_schema()
            .is_some_and(Schema::is_deprecated);
        // 0 = required, 1 = normal, 2 = deprecated
        i32::from(deprecated) * 2 + i32::from(!required.contains(name))
    });

    for (prop_name, prop_sv) in sorted_props {
        let resolved_sv = resolve_ref(prop_sv, root);
        let Some(prop_schema) = resolved_sv.as_schema() else {
            let _ = writeln!(out, "{indent}{}{prop_name}{}", f.green, f.reset);
            out.push('\n');
            continue;
        };
        let ty = schema_type_str(prop_schema).unwrap_or_default();
        let is_required = required.contains(prop_name);
        let is_deprecated = prop_schema.is_deprecated();
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
    prop_schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    desc_indent: &str,
) {
    if let Some(desc) = get_description(prop_schema) {
        write_description(out, desc, f, desc_indent);
    }

    if let Some(ref default) = prop_schema.default {
        write_label(
            out,
            desc_indent,
            "Default",
            &format!("{}{}{}", f.magenta, format_value(default), f.reset),
        );
    }

    if let Some(ref values) = prop_schema.enum_ {
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

    if let Some(ref c) = prop_schema.const_ {
        write_label(
            out,
            desc_indent,
            "Constant",
            &format!("{}{c}{}", f.magenta, f.reset),
        );
    }

    if let Some(ref examples) = prop_schema.examples
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
        let variants = match *keyword {
            "oneOf" => prop_schema.one_of.as_ref(),
            "anyOf" => prop_schema.any_of.as_ref(),
            "allOf" => prop_schema.all_of.as_ref(),
            _ => None,
        };
        if let Some(variants) = variants {
            let label = match *keyword {
                "oneOf" => "One of",
                "anyOf" => "Any of",
                "allOf" => "All of",
                _ => keyword,
            };
            let _ = writeln!(out, "{desc_indent}{}{label}:{}", f.dim, f.reset);
            for (i, variant) in variants.iter().enumerate() {
                let resolved_sv = resolve_ref(variant, root);
                let resolved = resolved_sv.as_schema();
                if let Some(resolved) = resolved {
                    render_inline_variant(
                        out,
                        resolved,
                        variant,
                        root,
                        f,
                        depth,
                        desc_indent,
                        i + 1,
                    );
                } else {
                    let summary = variant_summary(variant, root, f);
                    let _ = writeln!(out, "{desc_indent}  - {summary}");
                }
            }
        }
    }

    if depth < MAX_DEPTH
        && let Some(ref nested_props) = prop_schema.properties
    {
        let nested_required = required_set(prop_schema);
        out.push('\n');
        render_properties(out, nested_props, &nested_required, root, f, depth + 1);
    }
}

/// Render a variant inline within a property's composition list.
#[allow(clippy::too_many_arguments)]
fn render_inline_variant(
    out: &mut String,
    resolved: &Schema,
    original: &SchemaValue,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    desc_indent: &str,
    index: usize,
) {
    let is_ref = original.as_schema().is_some_and(|s| s.ref_.is_some());
    let has_properties = resolved.properties.as_ref().is_some_and(|p| !p.is_empty());

    if !is_ref && has_properties && depth < MAX_DEPTH {
        let deprecated_tag = deprecated_tag(resolved, f);
        let (label, label_is_type) = if let Some(ref title) = resolved.title {
            (title.clone(), false)
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
        if let Some(ref props) = resolved.properties {
            let req = required_set(resolved);
            render_properties(out, props, &req, root, f, depth + 2);
        }
    } else if let Some(ref values) = resolved.enum_ {
        let prefix = format!("{desc_indent}  - ");
        render_enum_values(out, values, f, &prefix);
    } else {
        let summary = variant_summary(original, root, f);
        let _ = writeln!(out, "{desc_indent}  - {summary}");
    }
}

/// Render enum values with line-wrapping when they exceed the terminal width.
///
/// `prefix` is what starts the first line (e.g. `"        - "`).
/// Continuation lines are indented to align after the prefix.
fn render_enum_values(out: &mut String, values: &[serde_json::Value], f: &Fmt<'_>, prefix: &str) {
    let items: Vec<String> = values
        .iter()
        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
        .collect();

    // Try single line first.
    let single_line = items.join(", ");
    let prefix_visual_len = prefix.chars().filter(|c| !c.is_control()).count();
    if prefix_visual_len + single_line.len() <= f.width {
        let colored = items
            .iter()
            .map(|s| format!("{}{s}{}", f.magenta, f.reset))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "{prefix}{colored}");
        return;
    }

    // Wrap: continuation lines align with the first value.
    let cont_indent = " ".repeat(prefix_visual_len);
    let mut line_len = prefix_visual_len;
    let mut first_on_line = true;
    out.push_str(prefix);

    for (i, item) in items.iter().enumerate() {
        let sep = if i > 0 { ", " } else { "" };
        let needed = sep.len() + item.len();

        if !first_on_line && line_len + needed > f.width {
            if i > 0 {
                out.push(',');
            }
            out.push('\n');
            out.push_str(&cont_indent);
            line_len = cont_indent.len();
            first_on_line = true;
        }

        if !first_on_line {
            out.push_str(sep);
            line_len += sep.len();
        }
        let _ = write!(out, "{}{item}{}", f.magenta, f.reset);
        line_len += item.len();
        first_on_line = false;
    }
    out.push('\n');
}

/// Return a `" [DEPRECATED]"` tag if the schema has `"deprecated": true`.
fn deprecated_tag(schema: &Schema, f: &Fmt<'_>) -> String {
    if schema.is_deprecated() {
        format!(" {}[DEPRECATED]{}", f.dim, f.reset)
    } else {
        String::new()
    }
}

/// Render JSON Schema validation constraints as a compact annotation line.
fn render_constraints(out: &mut String, schema: &Schema, f: &Fmt<'_>, indent: &str) {
    let mut parts: Vec<String> = Vec::new();

    if let Some(ref fmt_val) = schema.format {
        parts.push(format!("format={}{fmt_val}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.min_length {
        parts.push(format!("minLength={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.max_length {
        parts.push(format!("maxLength={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.pattern {
        parts.push(format!("pattern={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.minimum {
        parts.push(format!("min={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.maximum {
        parts.push(format!("max={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.exclusive_minimum {
        parts.push(format!("exclusiveMin={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.exclusive_maximum {
        parts.push(format!("exclusiveMax={}{v}{}", f.magenta, f.reset));
    }
    if let Some(ref v) = schema.multiple_of {
        parts.push(format!("multipleOf={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.min_items {
        parts.push(format!("minItems={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.max_items {
        parts.push(format!("maxItems={}{v}{}", f.magenta, f.reset));
    }
    if schema.unique_items.unwrap_or(false) {
        parts.push(format!("{}unique{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.min_properties {
        parts.push(format!("minProperties={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.max_properties {
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
    schema: &SchemaValue,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let resolved_sv = resolve_ref(schema, root);
    let Some(resolved) = resolved_sv.as_schema() else {
        return;
    };
    let ty = schema_type_str(resolved).unwrap_or_default();

    if !ty.is_empty() {
        write_label(out, &indent, "Type", &format_type(&ty, f));
    }

    if let Some(desc) = get_description(resolved) {
        write_description(out, desc, f, &indent);
    }

    if depth < MAX_DEPTH {
        let required = required_set(resolved);
        if let Some(ref props) = resolved.properties {
            render_properties(out, props, &required, root, f, depth + 1);
        }
    }
}
