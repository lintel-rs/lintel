use core::fmt::Write;

use serde_json::Value;

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type_suffix, format_value};
use crate::man::{write_description, write_label, write_section};
use crate::render::{render_properties, render_variant_block};
use crate::schema::{get_description, required_set, resolve_ref, schema_type_str};

/// Render the SCHEMA section with URL and source information.
pub(crate) fn render_schema_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
    let id = schema.get("$id").and_then(Value::as_str);
    let x_lintel = schema.get("x-lintel").and_then(Value::as_object);
    let source = x_lintel.and_then(|xl| xl.get("source").and_then(Value::as_str));

    if id.is_none() && source.is_none() {
        return;
    }

    write_section(out, "SCHEMA", f);
    if let Some(url) = id {
        write_label(out, "    ", "URL", url);
    }
    if let Some(src) = source {
        write_label(out, "    ", "Source", src);
    }
    out.push('\n');
}

/// Render `oneOf`/`anyOf`/`allOf` variant sections.
pub(crate) fn render_variants_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
    for keyword in COMPOSITION_KEYWORDS {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let label = match *keyword {
                "oneOf" => "ONE OF",
                "anyOf" => "ANY OF",
                "allOf" => "ALL OF",
                _ => keyword,
            };
            write_section(out, label, f);
            for (i, variant) in variants.iter().enumerate() {
                let resolved = resolve_ref(variant, schema);
                render_variant_block(out, resolved, variant, schema, f, i + 1);
            }
            out.push('\n');
        }
    }
}

/// Render an EXAMPLES section when the schema has top-level `examples`.
pub(crate) fn render_examples_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
    let examples = match schema.get("examples").and_then(Value::as_array) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return,
    };

    write_section(out, "EXAMPLES", f);
    for (i, example) in examples.iter().enumerate() {
        if examples.len() > 1 {
            let _ = writeln!(out, "    {}({}){}:", f.dim, i + 1, f.reset);
        }
        match example {
            Value::Object(_) | Value::Array(_) => {
                let json = serde_json::to_string_pretty(example).unwrap_or_default();
                let lang_hint = if f.syntax_highlight { "json" } else { "" };
                let block = format!("```{lang_hint}\n{json}\n```");
                write_description(out, &block, f, "    ");
            }
            _ => {
                let _ = writeln!(out, "    {}{}{}", f.magenta, format_value(example), f.reset);
            }
        }
    }
    out.push('\n');
}

/// Render the DEFINITIONS section (`$defs`/`definitions`).
pub(crate) fn render_definitions_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
    for defs_key in &["$defs", "definitions"] {
        if let Some(defs) = schema.get(*defs_key).and_then(Value::as_object)
            && !defs.is_empty()
        {
            write_section(out, "DEFINITIONS", f);
            // Sort deprecated definitions to the end.
            let mut sorted_defs: Vec<_> = defs.iter().collect();
            sorted_defs.sort_by_key(|(_, s)| {
                i32::from(
                    s.get("deprecated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
            });
            for (def_name, def_schema) in sorted_defs {
                let ty = schema_type_str(def_schema).unwrap_or_default();
                let suffix = format_type_suffix(&ty, f);
                let is_deprecated = def_schema
                    .get("deprecated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let dep_tag = if is_deprecated {
                    format!(" {}[DEPRECATED]{}", f.dim, f.reset)
                } else {
                    String::new()
                };
                let _ = writeln!(out, "    {}{def_name}{}{dep_tag}{suffix}", f.green, f.reset);
                if let Some(desc) = get_description(def_schema) {
                    write_description(out, desc, f, "        ");
                }
                if let Some(props) = def_schema.get("properties").and_then(Value::as_object) {
                    let req = required_set(def_schema);
                    render_properties(out, props, &req, schema, f, 2);
                }
                out.push('\n');
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::fmt::Fmt;
    use serde_json::json;

    // --- SCHEMA section ---

    #[test]
    fn schema_section_with_id() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "$id": "https://json.schemastore.org/cargo.json",
            "type": "object"
        });

        render_schema_section(&mut out, &schema, &f);
        assert!(out.contains("SCHEMA"));
        assert!(out.contains("URL: https://json.schemastore.org/cargo.json"));
    }

    #[test]
    fn schema_section_with_x_lintel_source() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "$id": "https://json.schemastore.org/cargo.json",
            "x-lintel": {
                "source": "https://raw.githubusercontent.com/nickel-org/cargo.json",
                "sourceSha256": "abc123"
            },
            "type": "object"
        });

        render_schema_section(&mut out, &schema, &f);
        assert!(out.contains("SCHEMA"));
        assert!(out.contains("URL: https://json.schemastore.org/cargo.json"));
        assert!(out.contains("Source: https://raw.githubusercontent.com/nickel-org/cargo.json"));
    }

    #[test]
    fn schema_section_not_shown_without_id_or_x_lintel() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({ "type": "object" });

        render_schema_section(&mut out, &schema, &f);
        assert!(out.is_empty());
    }

    #[test]
    fn schema_section_with_only_x_lintel() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "x-lintel": {
                "source": "https://example.com/schema.json",
                "sourceSha256": "def456"
            },
            "type": "object"
        });

        render_schema_section(&mut out, &schema, &f);
        assert!(out.contains("SCHEMA"));
        assert!(!out.contains("URL:"));
        assert!(out.contains("Source: https://example.com/schema.json"));
    }

    // --- EXAMPLES section ---

    #[test]
    fn examples_section_renders() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "type": "object",
            "examples": [
                { "name": "test", "value": 42 }
            ]
        });

        render_examples_section(&mut out, &schema, &f);
        assert!(out.contains("EXAMPLES"));
        assert!(out.contains("\"name\": \"test\""));
    }

    #[test]
    fn empty_examples_not_shown() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({ "examples": [] });

        render_examples_section(&mut out, &schema, &f);
        assert!(out.is_empty());
    }

    // --- DEFINITIONS section ---

    #[test]
    fn definitions_not_truncated() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "definitions": {
                "myDef": {
                    "type": "object",
                    "description": "This is a very long description that should not be truncated at all because we want to show the full text to users who are reading the documentation"
                }
            }
        });

        render_definitions_section(&mut out, &schema, &f);
        assert!(out.contains("reading the documentation"));
        assert!(!out.contains("..."));
    }

    // --- VARIANTS section ---

    #[test]
    fn any_of_variants_listed() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = json!({
            "anyOf": [
                { "type": "string", "description": "A string value" },
                { "type": "integer", "description": "An integer value" }
            ]
        });

        render_variants_section(&mut out, &schema, &f);
        assert!(out.contains("ANY OF"));
        assert!(out.contains("A string value"));
        assert!(out.contains("An integer value"));
    }

    #[test]
    fn allof_refs_expanded() {
        let mut out = String::new();
        let f = Fmt::plain(80);
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

        render_variants_section(&mut out, &schema, &f);
        assert!(out.contains("ALL OF"));
        assert!(out.contains("base"));
        assert!(out.contains("Base configuration"));
        assert!(out.contains("name (string)"));
    }
}
