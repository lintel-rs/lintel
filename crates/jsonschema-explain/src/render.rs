use core::fmt::Write;

use indexmap::IndexMap;
use jsonschema_schema::{Schema, SchemaValue, ref_name};

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type, format_type_suffix, format_value};
use crate::man::{write_description, write_label, write_label_wrapped};
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

    let has_properties = !resolved.properties.is_empty();
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
        if !resolved.properties.is_empty() {
            let req = required_set(resolved);
            render_properties(out, &resolved.properties, &req, root, f, 2);
        }
    } else if resolved.enum_.is_some() {
        if resolved.markdown_enum_descriptions.is_some() {
            render_enum_with_descriptions(out, resolved, f, "        ");
        } else if let Some(ref values) = resolved.enum_ {
            let prefix = format!("    {}({index}){} ", f.dim, f.reset);
            render_enum_values(out, values, f, &prefix);
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
        let readonly_tag = if prop_schema.read_only {
            format!(" {}[READ-ONLY]{}", f.dim, f.reset)
        } else {
            String::new()
        };
        let writeonly_tag = if prop_schema.write_only {
            format!(" {}[WRITE-ONLY]{}", f.dim, f.reset)
        } else {
            String::new()
        };

        let _ = writeln!(
            out,
            "{indent}{}{prop_name}{}{deprecated_tag}{readonly_tag}{writeonly_tag} ({type_display}{req_tag})",
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

    if f.extended
        && let Some(ref comment) = prop_schema.comment
    {
        let _ = writeln!(out, "{desc_indent}{}Comment:{}", f.dim, f.reset);
        write_description(out, comment, f, &format!("{desc_indent}  "));
    }

    if let Some(ref default) = prop_schema.default {
        write_label_wrapped(out, desc_indent, "Default", &format_value(default), f);
    }

    render_enum_with_descriptions(out, prop_schema, f, desc_indent);

    if let Some(ref c) = prop_schema.const_ {
        write_label_wrapped(out, desc_indent, "Constant", &c.to_string(), f);
    }

    if let Some(ref examples) = prop_schema.examples
        && !examples.is_empty()
    {
        let joined: String = examples
            .iter()
            .map(format_value)
            .collect::<Vec<_>>()
            .join(", ");
        write_label_wrapped(out, desc_indent, "Examples", &joined, f);
    }

    render_constraints(out, prop_schema, f, desc_indent);

    // Content type/encoding
    render_content_info(out, prop_schema, f, desc_indent);

    // Composition keywords (oneOf/anyOf/allOf)
    render_composition(out, prop_schema, root, f, depth, desc_indent);

    // not
    if let Some(ref not_sv) = prop_schema.not {
        let summary = variant_summary(not_sv, root, f);
        write_label(out, desc_indent, "Not", &summary);
    }

    // if/then/else
    render_conditional(out, prop_schema, root, f, depth, desc_indent);

    // dependentRequired
    render_dependent_required(out, prop_schema, f, desc_indent);

    // dependentSchemas
    render_dependent_schemas(out, prop_schema, root, f, depth, desc_indent);

    // Nested properties
    if depth < MAX_DEPTH && !prop_schema.properties.is_empty() {
        let nested_required = required_set(prop_schema);
        out.push('\n');
        render_properties(
            out,
            &prop_schema.properties,
            &nested_required,
            root,
            f,
            depth + 1,
        );
    }

    // patternProperties
    render_pattern_properties(out, prop_schema, root, f, depth, desc_indent);

    // propertyNames
    render_property_names(out, prop_schema, f, desc_indent);

    // prefixItems / contains
    render_prefix_items(out, prop_schema, root, f, desc_indent);
    render_contains(out, prop_schema, root, f, desc_indent);

    // additionalProperties
    render_additional_properties(out, prop_schema, root, f, depth, desc_indent);
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
    let has_properties = !resolved.properties.is_empty();

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
        if !resolved.properties.is_empty() {
            let req = required_set(resolved);
            render_properties(out, &resolved.properties, &req, root, f, depth + 2);
        }
    } else if resolved.enum_.is_some() {
        if resolved.markdown_enum_descriptions.is_some() {
            let nested_indent = format!("{desc_indent}  ");
            render_enum_with_descriptions(out, resolved, f, &nested_indent);
        } else if let Some(ref values) = resolved.enum_ {
            let prefix = format!("{desc_indent}  - ");
            render_enum_values(out, values, f, &prefix);
        }
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
    if schema.unique_items {
        parts.push(format!("{}unique{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.min_contains {
        parts.push(format!("minContains={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.max_contains {
        parts.push(format!("maxContains={}{v}{}", f.magenta, f.reset));
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

/// Render enum values, optionally with `markdownEnumDescriptions`.
fn render_enum_with_descriptions(out: &mut String, schema: &Schema, f: &Fmt<'_>, indent: &str) {
    let Some(ref values) = schema.enum_ else {
        return;
    };
    if let Some(ref descs) = schema.markdown_enum_descriptions
        && descs.iter().any(Option::is_some)
    {
        let _ = writeln!(out, "{indent}{}Values:{}", f.dim, f.reset);
        for (i, val) in values.iter().enumerate() {
            let display = val.as_str().map_or_else(|| val.to_string(), str::to_string);
            let desc = descs.get(i).and_then(|d| d.as_deref()).unwrap_or_default();
            if desc.is_empty() {
                let _ = writeln!(out, "{indent}    {}{display}{}", f.magenta, f.reset);
            } else {
                let rendered_desc = if f.is_color() {
                    markdown_to_ansi::render_inline(desc, &f.md_opts(None))
                } else {
                    desc.to_string()
                };
                let _ = writeln!(
                    out,
                    "{indent}    {}{display}{} — {rendered_desc}",
                    f.magenta, f.reset
                );
            }
        }
    } else {
        let prefix = format!("{indent}Values: ");
        render_enum_values(out, values, f, &prefix);
    }
}

/// Render `contentMediaType` / `contentEncoding`.
fn render_content_info(out: &mut String, schema: &Schema, f: &Fmt<'_>, indent: &str) {
    let media = schema.content_media_type.as_deref();
    let encoding = schema.content_encoding.as_deref();
    if media.is_none() && encoding.is_none() {
        return;
    }
    let value = match (media, encoding) {
        (Some(m), Some(e)) => format!("{}{m}{} ({e})", f.magenta, f.reset),
        (Some(m), None) => format!("{}{m}{}", f.magenta, f.reset),
        (None, Some(e)) => format!("{}{e}{}", f.magenta, f.reset),
        (None, None) => unreachable!(),
    };
    write_label(out, indent, "Content", &value);
}

/// Render composition keywords (oneOf/anyOf/allOf) for a property.
#[allow(clippy::too_many_arguments)]
fn render_composition(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    desc_indent: &str,
) {
    for keyword in COMPOSITION_KEYWORDS {
        let variants = match *keyword {
            "oneOf" => schema.one_of.as_ref(),
            "anyOf" => schema.any_of.as_ref(),
            "allOf" => schema.all_of.as_ref(),
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
}

/// Render `if`/`then`/`else` conditional.
#[allow(clippy::too_many_arguments)]
fn render_conditional(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    indent: &str,
) {
    if schema.if_.is_none() {
        return;
    }
    let _ = writeln!(out, "{indent}{}Conditional:{}", f.dim, f.reset);
    if let Some(ref if_sv) = schema.if_ {
        let summary = variant_summary(if_sv, root, f);
        let _ = writeln!(out, "{indent}  If: {summary}");
        if depth < MAX_DEPTH {
            render_conditional_subschema(out, if_sv, root, f, depth);
        }
    }
    if let Some(ref then_sv) = schema.then_ {
        let summary = variant_summary(then_sv, root, f);
        let _ = writeln!(out, "{indent}  Then: {summary}");
        if depth < MAX_DEPTH {
            render_conditional_subschema(out, then_sv, root, f, depth);
        }
    }
    if let Some(ref else_sv) = schema.else_ {
        let summary = variant_summary(else_sv, root, f);
        let _ = writeln!(out, "{indent}  Else: {summary}");
        if depth < MAX_DEPTH {
            render_conditional_subschema(out, else_sv, root, f, depth);
        }
    }
}

/// Render nested properties within a conditional subschema.
#[allow(clippy::too_many_arguments)]
fn render_conditional_subschema(
    out: &mut String,
    sv: &SchemaValue,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
) {
    let resolved_sv = resolve_ref(sv, root);
    if let Some(resolved) = resolved_sv.as_schema()
        && !resolved.properties.is_empty()
    {
        let req = required_set(resolved);
        render_properties(out, &resolved.properties, &req, root, f, depth + 2);
    }
}

/// Render `dependentRequired`.
fn render_dependent_required(out: &mut String, schema: &Schema, f: &Fmt<'_>, indent: &str) {
    let Some(ref deps) = schema.dependent_required else {
        return;
    };
    if deps.is_empty() {
        return;
    }
    let _ = writeln!(out, "{indent}{}Dependent required:{}", f.dim, f.reset);
    for (key, required) in deps {
        let values = required
            .iter()
            .map(|r| format!("{}\"{r}\"{}", f.magenta, f.reset))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "{indent}  {}\"{key}\"{} requires: {values}",
            f.green, f.reset
        );
    }
}

/// Render `dependentSchemas`.
#[allow(clippy::too_many_arguments)]
fn render_dependent_schemas(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    indent: &str,
) {
    if schema.dependent_schemas.is_empty() {
        return;
    }
    let _ = writeln!(out, "{indent}{}Dependent schemas:{}", f.dim, f.reset);
    for (key, dep_sv) in &schema.dependent_schemas {
        let summary = variant_summary(dep_sv, root, f);
        let _ = writeln!(out, "{indent}  {}\"{key}\"{}: {summary}", f.green, f.reset);
        if depth < MAX_DEPTH {
            let resolved_sv = resolve_ref(dep_sv, root);
            if let Some(resolved) = resolved_sv.as_schema()
                && !resolved.properties.is_empty()
            {
                let req = required_set(resolved);
                render_properties(out, &resolved.properties, &req, root, f, depth + 2);
            }
        }
    }
}

/// Render `patternProperties`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_pattern_properties(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    indent: &str,
) {
    if schema.pattern_properties.is_empty() {
        return;
    }
    let _ = writeln!(out, "{indent}{}Pattern properties:{}", f.dim, f.reset);
    for (pattern, sv) in &schema.pattern_properties {
        let resolved_sv = resolve_ref(sv, root);
        let ty = resolved_sv
            .as_schema()
            .and_then(schema_type_str)
            .unwrap_or_default();
        let type_display = format_type(&ty, f);
        if ty.is_empty() {
            let _ = writeln!(out, "{indent}  {}{pattern}{}", f.green, f.reset);
        } else {
            let _ = writeln!(
                out,
                "{indent}  {}{pattern}{} ({type_display})",
                f.green, f.reset
            );
        }
        if let Some(resolved) = resolved_sv.as_schema() {
            if let Some(desc) = get_description(resolved) {
                let nested_indent = format!("{indent}      ");
                write_description(out, desc, f, &nested_indent);
            }
            if depth < MAX_DEPTH && !resolved.properties.is_empty() {
                let req = required_set(resolved);
                render_properties(out, &resolved.properties, &req, root, f, depth + 2);
            }
        }
    }
}

/// Render `propertyNames` constraints.
fn render_property_names(out: &mut String, schema: &Schema, f: &Fmt<'_>, indent: &str) {
    let Some(ref pn_sv) = schema.property_names else {
        return;
    };
    let Some(pn) = pn_sv.as_schema() else {
        return;
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref pat) = pn.pattern {
        parts.push(format!("pattern={}{pat}{}", f.magenta, f.reset));
    }
    if let Some(ref fmt_val) = pn.format {
        parts.push(format!("format={}{fmt_val}{}", f.magenta, f.reset));
    }
    if let Some(ref values) = pn.enum_ {
        let joined = values
            .iter()
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(joined);
    }
    if let Some(v) = pn.min_length {
        parts.push(format!("minLength={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = pn.max_length {
        parts.push(format!("maxLength={}{v}{}", f.magenta, f.reset));
    }
    if !parts.is_empty() {
        write_label(out, indent, "Property names", &parts.join(", "));
    }
}

/// Render `prefixItems` (tuple validation).
#[allow(clippy::too_many_arguments)]
fn render_prefix_items(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    indent: &str,
) {
    let Some(ref items) = schema.prefix_items else {
        return;
    };
    if items.is_empty() {
        return;
    }
    let _ = writeln!(out, "{indent}{}Tuple items:{}", f.dim, f.reset);
    for (i, item) in items.iter().enumerate() {
        let summary = variant_summary(item, root, f);
        let _ = writeln!(out, "{indent}  [{i}]: {summary}");
    }
}

/// Render `contains`.
#[allow(clippy::too_many_arguments)]
fn render_contains(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    indent: &str,
) {
    let Some(ref contains_sv) = schema.contains else {
        return;
    };
    let summary = variant_summary(contains_sv, root, f);
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = schema.min_contains {
        parts.push(format!("minContains={}{v}{}", f.magenta, f.reset));
    }
    if let Some(v) = schema.max_contains {
        parts.push(format!("maxContains={}{v}{}", f.magenta, f.reset));
    }
    if parts.is_empty() {
        write_label(out, indent, "Contains", &summary);
    } else {
        write_label(
            out,
            indent,
            "Contains",
            &format!("{summary} ({})", parts.join(", ")),
        );
    }
}

/// Render `additionalProperties`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_additional_properties(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
    depth: usize,
    indent: &str,
) {
    let Some(ref ap) = schema.additional_properties else {
        return;
    };
    match ap.as_ref() {
        SchemaValue::Bool(false) => {
            let _ = writeln!(
                out,
                "{indent}{}Additional properties:{} not allowed",
                f.dim, f.reset
            );
        }
        SchemaValue::Bool(true) => {} // default behavior, skip
        SchemaValue::Schema(s) => {
            let ty = schema_type_str(s).unwrap_or_default();
            let type_display = format_type(&ty, f);
            if ty.is_empty() {
                let _ = writeln!(
                    out,
                    "{indent}{}Additional properties:{} allowed",
                    f.dim, f.reset
                );
            } else {
                let _ = writeln!(
                    out,
                    "{indent}{}Additional properties:{} {type_display}",
                    f.dim, f.reset
                );
            }
            if let Some(desc) = get_description(s) {
                let nested_indent = format!("{indent}    ");
                write_description(out, desc, f, &nested_indent);
            }
            if depth < MAX_DEPTH && !s.properties.is_empty() {
                let req = required_set(s);
                render_properties(out, &s.properties, &req, root, f, depth + 2);
            }
        }
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
        if !resolved.properties.is_empty() {
            render_properties(out, &resolved.properties, &required, root, f, depth + 1);
        }
    }
}
