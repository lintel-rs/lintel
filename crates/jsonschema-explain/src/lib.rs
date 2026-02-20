//! Render JSON Schema as human-readable terminal documentation.
//!
//! Produces man-page-style output from a `serde_json::Value` containing a
//! JSON Schema, with optional ANSI color formatting for terminal display.

#![allow(clippy::format_push_string)] // Text rendering naturally uses format!() + push_str

use std::fmt::Write;

use serde_json::Value;

/// Maximum nesting depth for recursive property rendering.
const MAX_DEPTH: usize = 3;

// ANSI escape codes
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const BLUE: &str = "\x1b[34m";

/// Formatting context passed through the rendering functions.
struct Fmt<'a> {
    bold: &'a str,
    dim: &'a str,
    reset: &'a str,
    /// Cyan — used for type annotations.
    cyan: &'a str,
    /// Green — used for property names.
    green: &'a str,
    /// Yellow — used for section headers.
    yellow: &'a str,
    /// Magenta — used for values (defaults, enums, constants).
    magenta: &'a str,
    /// Blue — used for inline code (backtick-wrapped text).
    blue: &'a str,
}

impl Fmt<'_> {
    fn color() -> Self {
        Fmt {
            bold: BOLD,
            dim: DIM,
            reset: RESET,
            cyan: CYAN,
            green: GREEN,
            yellow: YELLOW,
            magenta: MAGENTA,
            blue: BLUE,
        }
    }

    fn plain() -> Self {
        Fmt {
            bold: "",
            dim: "",
            reset: "",
            cyan: "",
            green: "",
            yellow: "",
            magenta: "",
            blue: "",
        }
    }
}

/// Render a JSON Schema as human-readable terminal documentation.
///
/// `schema` is a parsed JSON Schema value. `name` is a display name
/// (e.g. from a catalog entry). When `color` is true, ANSI escape
/// codes are used for formatting.
#[allow(clippy::too_many_lines)]
pub fn explain(schema: &Value, name: &str, color: bool) -> String {
    let mut out = String::new();
    let f = if color { Fmt::color() } else { Fmt::plain() };

    // Header line
    let upper = name.to_uppercase();
    let header = format_header(&upper, "JSON Schema");
    let _ = writeln!(out, "{}{header}{}\n", f.bold, f.reset);

    // NAME section
    let title = schema.get("title").and_then(Value::as_str).unwrap_or(name);
    let description = get_description(schema);

    let _ = writeln!(out, "{}NAME{}", f.yellow, f.reset);
    if let Some(desc) = description {
        let _ = writeln!(
            out,
            "    {}{title}{} - {}",
            f.bold,
            f.reset,
            render_inline(desc, &f)
        );
    } else {
        let _ = writeln!(out, "    {}{title}{}", f.bold, f.reset);
    }
    out.push('\n');

    // DESCRIPTION section (if there's a longer description separate from title)
    if let Some(desc) = description
        && schema.get("title").and_then(Value::as_str).is_some()
    {
        let _ = writeln!(out, "{}DESCRIPTION{}", f.yellow, f.reset);
        write_description(&mut out, desc, &f, "    ");
        out.push('\n');
    }

    // TYPE section
    if let Some(ty) = schema_type_str(schema) {
        let _ = writeln!(out, "{}TYPE{}", f.yellow, f.reset);
        let _ = writeln!(out, "    {}", format_type(&ty, &f));
        out.push('\n');
    }

    // Collect required fields
    let required = required_set(schema);

    // PROPERTIES section
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        let _ = writeln!(out, "{}PROPERTIES{}", f.yellow, f.reset);
        render_properties(&mut out, props, &required, schema, &f, 1);
        out.push('\n');
    }

    // ITEMS section (for array schemas)
    if schema_type_str(schema).as_deref() == Some("array")
        && let Some(items) = schema.get("items")
    {
        let _ = writeln!(out, "{}ITEMS{}", f.yellow, f.reset);
        render_subschema(&mut out, items, schema, &f, 1);
        out.push('\n');
    }

    // oneOf / anyOf / allOf — resolve $ref variants and expand if they have properties
    for keyword in &["oneOf", "anyOf", "allOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let label = match *keyword {
                "oneOf" => "ONE OF",
                "anyOf" => "ANY OF",
                "allOf" => "ALL OF",
                _ => keyword,
            };
            let _ = writeln!(out, "{}{label}{}", f.yellow, f.reset);
            for (i, variant) in variants.iter().enumerate() {
                let resolved = resolve_ref(variant, schema);
                render_variant_block(&mut out, resolved, variant, schema, &f, i + 1);
            }
            out.push('\n');
        }
    }

    // DEFINITIONS section — expanded with full descriptions and properties
    for defs_key in &["$defs", "definitions"] {
        if let Some(defs) = schema.get(*defs_key).and_then(Value::as_object)
            && !defs.is_empty()
        {
            let _ = writeln!(out, "{}DEFINITIONS{}", f.yellow, f.reset);
            for (def_name, def_schema) in defs {
                let ty = schema_type_str(def_schema).unwrap_or_default();
                let type_display = if ty.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", format_type(&ty, &f))
                };
                let _ = writeln!(out, "    {}{def_name}{}{type_display}", f.green, f.reset);
                if let Some(desc) = get_description(def_schema) {
                    write_description(&mut out, desc, &f, "        ");
                }
                // Show properties of definitions
                if let Some(props) = def_schema.get("properties").and_then(Value::as_object) {
                    let req = required_set(def_schema);
                    render_properties(&mut out, props, &req, schema, &f, 2);
                }
                out.push('\n');
            }
        }
    }

    out
}

/// Render a variant block for `oneOf`/`anyOf`/`allOf`.
///
/// If the resolved variant has properties or a description, expand
/// them inline. Otherwise, render a single summary line.
fn render_variant_block(
    out: &mut String,
    resolved: &Value,
    original: &Value,
    root: &Value,
    f: &Fmt<'_>,
    index: usize,
) {
    // Get a label for this variant
    let label = if let Some(title) = resolved.get("title").and_then(Value::as_str) {
        title.to_string()
    } else if let Some(r) = original.get("$ref").and_then(Value::as_str) {
        r.rsplit('/').next().unwrap_or(r).to_string()
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
        let type_display = if ty.is_empty() {
            String::new()
        } else {
            format!(" ({})", format_type(&ty, f))
        };
        let _ = writeln!(
            out,
            "    {}({index}){} {}{label}{}{type_display}",
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
#[allow(clippy::too_many_lines)]
fn render_properties(
    out: &mut String,
    props: &serde_json::Map<String, Value>,
    required: &[String],
    root: &Value,
    f: &Fmt<'_>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let desc_indent = format!("{indent}    ");

    for (prop_name, prop_schema) in props {
        let prop_schema = resolve_ref(prop_schema, root);
        let ty = schema_type_str(prop_schema).unwrap_or_default();
        let is_required = required.contains(prop_name);
        let type_display = format_type(&ty, f);
        let req_tag = if is_required {
            format!(", {}required{}", f.yellow, f.reset)
        } else {
            String::new()
        };

        let _ = writeln!(
            out,
            "{indent}{}{prop_name}{} ({type_display}{req_tag})",
            f.green, f.reset
        );

        // Description
        if let Some(desc) = get_description(prop_schema) {
            write_description(out, desc, f, &desc_indent);
        }

        // Default value
        if let Some(default) = prop_schema.get("default") {
            let _ = writeln!(
                out,
                "{desc_indent}{}Default:{} {}{}{}",
                f.dim,
                f.reset,
                f.magenta,
                format_json_value(default),
                f.reset
            );
        }

        // Enum values
        if let Some(values) = prop_schema.get("enum").and_then(Value::as_array) {
            let vals: Vec<String> = values
                .iter()
                .map(|v| match v {
                    Value::String(s) => format!("{}{s}{}", f.magenta, f.reset),
                    other => format!("{}{other}{}", f.magenta, f.reset),
                })
                .collect();
            let joined = vals.join(", ");
            let _ = writeln!(out, "{desc_indent}{}Values:{} {joined}", f.dim, f.reset);
        }

        // Const value
        if let Some(c) = prop_schema.get("const") {
            let _ = writeln!(
                out,
                "{desc_indent}{}Constant:{} {}{c}{}",
                f.dim, f.reset, f.magenta, f.reset
            );
        }

        // oneOf / anyOf / allOf within property
        for keyword in &["oneOf", "anyOf", "allOf"] {
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

        // Nested object properties (recurse if within depth limit)
        if depth < MAX_DEPTH
            && let Some(nested_props) = prop_schema.get("properties").and_then(Value::as_object)
        {
            let nested_required = required_set(prop_schema);
            out.push('\n');
            render_properties(out, nested_props, &nested_required, root, f, depth + 1);
        }

        // Array items type hint
        if prop_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "array")
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

        out.push('\n');
    }
}

/// Render a sub-schema summary at a given depth.
fn render_subschema(out: &mut String, schema: &Value, root: &Value, f: &Fmt<'_>, depth: usize) {
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

// ---------------------------------------------------------------------------
// Markdown / inline rendering
// ---------------------------------------------------------------------------

/// Get the best description text from a schema, preferring `markdownDescription`.
fn get_description(schema: &Value) -> Option<&str> {
    schema
        .get("markdownDescription")
        .and_then(Value::as_str)
        .or_else(|| schema.get("description").and_then(Value::as_str))
}

/// Write a multi-line description to the output buffer.
///
/// Each line from the description is output with the given `indent` prefix.
/// No wrapping is applied — the pager or terminal handles line wrapping.
/// Inline markdown (backticks, bold, links) is rendered.
fn write_description(out: &mut String, text: &str, f: &Fmt<'_>, indent: &str) {
    let rendered = render_markdown(text, f);
    for line in rendered.split('\n') {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}

/// Render block-level markdown: headers become bold, other lines get
/// inline rendering.
fn render_markdown(text: &str, f: &Fmt<'_>) -> String {
    text.split('\n')
        .map(|line| {
            // Markdown headers → bold
            let heading = line
                .strip_prefix("### ")
                .or_else(|| line.strip_prefix("## "))
                .or_else(|| line.strip_prefix("# "));
            if let Some(h) = heading {
                format!("{}{}{}", f.bold, render_inline(h.trim(), f), f.reset)
            } else {
                render_inline(line, f)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render inline markdown: `` `code` ``, `**bold**`, `[text](url)`, and raw URLs.
fn render_inline(text: &str, f: &Fmt<'_>) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 4);
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Backtick code spans
            b'`' => {
                if let Some(end) = text[i + 1..].find('`')
                    && end > 0
                {
                    let code = &text[i + 1..i + 1 + end];
                    let _ = write!(out, "{}{code}{}", f.blue, f.reset);
                    i += end + 2;
                    continue;
                }
                out.push('`');
                i += 1;
            }
            // **bold**
            b'*' if i + 2 < len && bytes[i + 1] == b'*' => {
                if let Some(end) = text[i + 2..].find("**")
                    && end > 0
                {
                    let content = &text[i + 2..i + 2 + end];
                    // Recurse for nested inline formatting inside bold
                    let _ = write!(out, "{}{}{}", f.bold, render_inline(content, f), f.reset);
                    i += end + 4;
                    continue;
                }
                out.push('*');
                i += 1;
            }
            // [text](url) markdown links
            b'[' => {
                if let Some(bracket_end) = text[i + 1..].find(']')
                    && let after = i + 1 + bracket_end + 1
                    && after < len
                    && bytes[after] == b'('
                    && let Some(paren_end) = text[after + 1..].find(')')
                {
                    let link_text = &text[i + 1..i + 1 + bracket_end];
                    let url = &text[after + 1..after + 1 + paren_end];
                    let _ = write!(
                        out,
                        "{} ({}{}{})",
                        render_inline(link_text, f),
                        f.dim,
                        url,
                        f.reset
                    );
                    i = after + 1 + paren_end + 1;
                    continue;
                }
                out.push('[');
                i += 1;
            }
            // Raw URLs
            b'h' if text[i..].starts_with("https://") || text[i..].starts_with("http://") => {
                let rest = &text[i..];
                let url_end = rest
                    .find(|c: char| c.is_whitespace() || matches!(c, ')' | '>' | ']'))
                    .unwrap_or(rest.len());
                let url = &rest[..url_end];
                let _ = write!(out, "{}{url}{}", f.dim, f.reset);
                i += url_end;
            }
            _ => {
                // Advance one full UTF-8 character
                let ch = text[i..].chars().next().unwrap_or('?');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    out
}

/// Format a type string with color.
///
/// Splits on ` | ` to colorize each alternative, and handles
/// compound types like `array of string`.
fn format_type(ty: &str, f: &Fmt<'_>) -> String {
    if ty.is_empty() {
        return String::new();
    }
    if ty.contains(" | ") {
        let parts: Vec<&str> = ty.split(" | ").collect();
        let colored: Vec<String> = parts
            .iter()
            .map(|p| format!("{}{p}{}", f.cyan, f.reset))
            .collect();
        colored.join(&format!(" {}|{} ", f.dim, f.reset))
    } else if let Some(rest) = ty.strip_prefix("array of ") {
        format!(
            "{}array{} {}of{} {}",
            f.cyan,
            f.reset,
            f.dim,
            f.reset,
            format_type(rest, f)
        )
    } else {
        format!("{}{ty}{}", f.cyan, f.reset)
    }
}

/// Format a JSON value for display.
fn format_json_value(val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{s}\""),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Schema helpers
// ---------------------------------------------------------------------------

/// Resolve a `$ref` within the same schema document.
fn resolve_ref<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str) {
        // Only resolve local references (#/...)
        if let Some(path) = ref_str.strip_prefix("#/") {
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
    }
    schema
}

/// Extract the `required` array from a schema as a list of strings.
fn required_set(schema: &Value) -> Vec<String> {
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
fn schema_type_str(schema: &Value) -> Option<String> {
    // Explicit type field
    if let Some(ty) = schema.get("type") {
        return match ty {
            Value::String(s) => {
                if s == "array" {
                    if let Some(items) = schema.get("items") {
                        let item_ty = schema_type_str(items).unwrap_or("any".to_string());
                        Some(format!("array of {item_ty}"))
                    } else {
                        Some("array".to_string())
                    }
                } else {
                    Some(s.clone())
                }
            }
            Value::Array(arr) => {
                let types: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
                Some(types.join(" | "))
            }
            _ => None,
        };
    }

    // $ref
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        return Some(name.to_string());
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
                            .map(|r| r.rsplit('/').next().unwrap_or(r).to_string())
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

/// Produce a one-line summary of a variant schema for `oneOf`/`anyOf`/`allOf` listings.
fn variant_summary(variant: &Value, root: &Value, f: &Fmt<'_>) -> String {
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
        let rendered = render_inline(desc, f);
        if ty.is_empty() {
            return rendered;
        }
        return format!("{} - {rendered}", format_type(&ty, f));
    }

    if let Some(r) = variant.get("$ref").and_then(Value::as_str) {
        if r.starts_with("#/") {
            let name = r.rsplit('/').next().unwrap_or(r);
            return format!("{}{name}{}", f.cyan, f.reset);
        }
        return format!("{}(see: {r}){}", f.dim, f.reset);
    }

    if let Some(ty) = schema_type_str(resolved) {
        return format_type(&ty, f);
    }

    format!("{}(schema){}", f.dim, f.reset)
}

/// Format a centered header line: `LEFT      CENTER      LEFT`
fn format_header(left: &str, center: &str) -> String {
    let width = 76;
    let total_content = left.len() * 2 + center.len();
    if total_content >= width {
        return format!("{left}  {center}  {left}");
    }
    let total_space = width - total_content;
    let pad = total_space / 2;
    format!(
        "{left}{}{center}{}{left}",
        " ".repeat(pad),
        " ".repeat(total_space - pad)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_object_schema() {
        let schema = json!({
            "title": "Test",
            "description": "A test schema",
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name field"
                },
                "age": {
                    "type": "integer",
                    "description": "The age field"
                }
            }
        });

        let output = explain(&schema, "test", false);
        assert!(output.contains("NAME"));
        assert!(output.contains("Test - A test schema"));
        assert!(output.contains("PROPERTIES"));
        assert!(output.contains("name (string)"));
        assert!(output.contains("The name field"));
        assert!(output.contains("age (integer)"));
    }

    #[test]
    fn nested_object_renders_with_indentation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "description": "Configuration block",
                    "properties": {
                        "debug": {
                            "type": "boolean",
                            "description": "Enable debug mode"
                        }
                    }
                }
            }
        });

        let output = explain(&schema, "nested", false);
        assert!(output.contains("config (object)"));
        assert!(output.contains("debug (boolean)"));
        assert!(output.contains("Enable debug mode"));
    }

    #[test]
    fn enum_values_listed() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["low", "medium", "high"]
                }
            }
        });

        let output = explain(&schema, "enum-test", false);
        assert!(output.contains("Values: low, medium, high"));
    }

    #[test]
    fn required_properties_marked() {
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string"
                },
                "optional": {
                    "type": "string"
                }
            }
        });

        let output = explain(&schema, "required-test", false);
        assert!(output.contains("name (string, required)"));
        assert!(output.contains("optional (string)"));
        assert!(!output.contains("optional (string, required)"));
    }

    #[test]
    fn schema_with_no_properties_handled() {
        let schema = json!({
            "type": "string",
            "description": "A plain string type"
        });

        let output = explain(&schema, "simple", false);
        assert!(output.contains("NAME"));
        assert!(output.contains("A plain string type"));
        assert!(!output.contains("PROPERTIES"));
    }

    #[test]
    fn color_output_contains_ansi() {
        let schema = json!({
            "title": "Colored",
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            }
        });

        let colored = explain(&schema, "colored", true);
        let plain = explain(&schema, "colored", false);

        assert!(colored.contains(BOLD));
        assert!(colored.contains(RESET));
        assert!(colored.contains(CYAN));
        assert!(colored.contains(GREEN));
        assert!(!plain.contains(BOLD));
        assert!(!plain.contains(RESET));
    }

    #[test]
    fn default_value_shown() {
        let schema = json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "default": 8080
                }
            }
        });

        let output = explain(&schema, "defaults", false);
        assert!(output.contains("Default: 8080"));
    }

    #[test]
    fn ref_resolution() {
        let schema = json!({
            "type": "object",
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "description": "An item definition"
                }
            }
        });

        let output = explain(&schema, "ref-test", false);
        assert!(output.contains("item (object)"));
        assert!(output.contains("An item definition"));
    }

    #[test]
    fn any_of_variants_listed() {
        let schema = json!({
            "anyOf": [
                { "type": "string", "description": "A string value" },
                { "type": "integer", "description": "An integer value" }
            ]
        });

        let output = explain(&schema, "union", false);
        assert!(output.contains("ANY OF"));
        assert!(output.contains("A string value"));
        assert!(output.contains("An integer value"));
    }

    #[test]
    fn format_header_centers() {
        let h = format_header("TEST", "JSON Schema");
        assert!(h.starts_with("TEST"));
        assert!(h.ends_with("TEST"));
        assert!(h.contains("JSON Schema"));
    }

    #[test]
    fn inline_backtick_colorization() {
        let f = Fmt::color();
        let result = render_inline("Use `foo` and `bar`", &f);
        assert!(result.contains(BLUE));
        assert!(result.contains("foo"));
        assert!(result.contains("bar"));
        assert!(!result.contains('`'));
    }

    #[test]
    fn inline_bold_rendering() {
        let f = Fmt::color();
        let result = render_inline("This is **important** text", &f);
        assert!(result.contains(BOLD));
        assert!(result.contains("important"));
        assert!(!result.contains("**"));
    }

    #[test]
    fn inline_markdown_link() {
        // Test without color to avoid ANSI codes containing '[' or '('
        let f = Fmt::plain();
        let result = render_inline("See [docs](https://example.com) here", &f);
        assert!(result.contains("docs"));
        assert!(result.contains("https://example.com"));
        // Markdown link syntax should be stripped
        assert!(!result.contains("]("));
    }

    #[test]
    fn inline_raw_url() {
        let f = Fmt::color();
        let result = render_inline("See more: https://example.com/foo", &f);
        assert!(result.contains(DIM));
        assert!(result.contains("https://example.com/foo"));
    }

    #[test]
    fn type_formatting_union() {
        let f = Fmt::plain();
        let result = format_type("object | null", &f);
        assert!(result.contains("object"));
        assert!(result.contains("null"));
        assert!(result.contains('|'));
    }

    #[test]
    fn definitions_not_truncated() {
        let schema = json!({
            "definitions": {
                "myDef": {
                    "type": "object",
                    "description": "This is a very long description that should not be truncated at all because we want to show the full text to users who are reading the documentation"
                }
            }
        });

        let output = explain(&schema, "test", false);
        assert!(output.contains("reading the documentation"));
        assert!(!output.contains("..."));
    }

    #[test]
    fn allof_refs_expanded() {
        let schema = json!({
            "allOf": [
                { "$ref": "#/definitions/base" }
            ],
            "definitions": {
                "base": {
                    "type": "object",
                    "description": "Base configuration",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name"
                        }
                    }
                }
            }
        });

        let output = explain(&schema, "test", false);
        assert!(output.contains("ALL OF"));
        assert!(output.contains("base"));
        assert!(output.contains("Base configuration"));
        assert!(output.contains("name (string)"));
    }

    #[test]
    fn prefers_markdown_description() {
        let schema = json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Plain description",
                    "markdownDescription": "Rich **markdown** description"
                }
            }
        });

        let output = explain(&schema, "test", false);
        assert!(output.contains("Rich markdown description"));
        assert!(!output.contains("Plain description"));
    }

    #[test]
    fn no_premature_wrapping() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "string",
                    "description": "This is a very long description that should not be wrapped at 72 characters because we want the pager to handle wrapping at the terminal width instead"
                }
            }
        });

        let output = explain(&schema, "test", false);
        // The entire description should be on one line (after indentation)
        let desc_line = output
            .lines()
            .find(|l| l.contains("This is a very long"))
            .expect("description line should be present");
        assert!(desc_line.contains("terminal width instead"));
    }

    #[test]
    fn markdown_headers_bolded() {
        let f = Fmt::color();
        let result = render_markdown("# My Header\nSome text", &f);
        assert!(result.contains(BOLD));
        assert!(result.contains("My Header"));
        // The # prefix should be stripped
        assert!(!result.contains("# "));
    }
}
