use core::fmt::Write;

use jsonschema_schema::{Schema, SchemaValue};
use serde_json::Value;

use crate::fmt::{COMPOSITION_KEYWORDS, Fmt, format_type_suffix, format_value};
use crate::man::{write_description, write_label, write_section};
use crate::render::{render_properties, render_variant_block};
use crate::schema::{get_description, required_set, resolve_ref, schema_type_str};

/// Render the SCHEMA section with URL and source information.
pub(crate) fn render_schema_section(out: &mut String, schema: &Schema, f: &Fmt<'_>) {
    let id = schema.id.as_deref();
    let source = schema.x_lintel.as_ref().and_then(|xl| xl.source.as_deref());

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
///
/// All composition keywords use the same rendering style:
/// - `$ref` entries show label, type, description, and URL (not expanded inline)
/// - Inline entries are expanded normally with properties
pub(crate) fn render_variants_section(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
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
                "oneOf" => "ONE OF",
                "anyOf" => "ANY OF",
                "allOf" => "ALL OF",
                _ => keyword,
            };
            write_section(out, label, f);
            for variant in variants {
                let resolved_sv = resolve_ref(variant, root);
                if let Some(resolved) = resolved_sv.as_schema() {
                    render_variant_block(out, resolved, variant, root, f);
                }
            }
            out.push('\n');
        }
    }
}

/// Render an EXAMPLES section when the schema has top-level `examples`.
pub(crate) fn render_examples_section(out: &mut String, schema: &Schema, f: &Fmt<'_>) {
    let examples = match schema.examples.as_ref() {
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
pub(crate) fn render_definitions_section(
    out: &mut String,
    schema: &Schema,
    root: &SchemaValue,
    f: &Fmt<'_>,
) {
    if let Some(ref defs) = schema.defs
        && !defs.is_empty()
    {
        render_defs_block(out, defs.iter(), root, f);
    }

    // Also check "definitions" in extra (pre-2020-12 schemas might have it)
    if let Some(defs) = schema.extra.get("definitions").and_then(|v| {
        serde_json::from_value::<indexmap::IndexMap<String, SchemaValue>>(v.clone()).ok()
    }) && !defs.is_empty()
    {
        render_defs_block(out, defs.iter(), root, f);
    }
}

fn render_defs_block<'a>(
    out: &mut String,
    defs: impl Iterator<Item = (&'a String, &'a SchemaValue)>,
    root: &SchemaValue,
    f: &Fmt<'_>,
) {
    write_section(out, "DEFINITIONS", f);
    // Sort deprecated definitions to the end.
    let mut sorted_defs: Vec<_> = defs.collect();
    sorted_defs.sort_by_key(|(_, sv)| i32::from(sv.as_schema().is_some_and(Schema::is_deprecated)));
    for (def_name, def_sv) in sorted_defs {
        let Some(def_schema) = def_sv.as_schema() else {
            let _ = writeln!(out, "    {}{def_name}{}", f.green, f.reset);
            out.push('\n');
            continue;
        };
        let ty = schema_type_str(def_schema).unwrap_or_default();
        let suffix = format_type_suffix(&ty, f);
        let dep_tag = if def_schema.is_deprecated() {
            format!(" {}[DEPRECATED]{}", f.dim, f.reset)
        } else {
            String::new()
        };
        let _ = writeln!(out, "    {}{def_name}{}{dep_tag}{suffix}", f.green, f.reset);
        if let Some(src) = def_schema
            .x_lintel
            .as_ref()
            .and_then(|xl| xl.source.as_deref())
        {
            write_label(out, "        ", "Source", src);
        }
        if let Some(desc) = get_description(def_schema) {
            write_description(out, desc, f, "        ");
        }
        if let Some(ref props) = def_schema.properties {
            let req = required_set(def_schema);
            render_properties(out, props, &req, root, f, 2);
        }
        out.push('\n');
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::fmt::Fmt;
    use serde_json::json;

    /// Parse with migration so tests work with older JSON Schema drafts.
    fn parse_schema(mut val: serde_json::Value) -> Schema {
        jsonschema_migrate::migrate_to_2020_12(&mut val);
        serde_json::from_value(val).unwrap()
    }

    fn parse_sv(mut val: serde_json::Value) -> SchemaValue {
        jsonschema_migrate::migrate_to_2020_12(&mut val);
        serde_json::from_value(val).unwrap()
    }

    // --- SCHEMA section ---

    #[test]
    fn schema_section_with_id() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = parse_schema(json!({
            "$id": "https://json.schemastore.org/cargo.json",
            "type": "object"
        }));

        render_schema_section(&mut out, &schema, &f);
        assert!(out.contains("SCHEMA"));
        assert!(out.contains("URL: https://json.schemastore.org/cargo.json"));
    }

    #[test]
    fn schema_section_with_x_lintel_source() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = parse_schema(json!({
            "$id": "https://json.schemastore.org/cargo.json",
            "x-lintel": {
                "source": "https://raw.githubusercontent.com/nickel-org/cargo.json",
                "sourceSha256": "abc123"
            },
            "type": "object"
        }));

        render_schema_section(&mut out, &schema, &f);
        assert!(out.contains("SCHEMA"));
        assert!(out.contains("URL: https://json.schemastore.org/cargo.json"));
        assert!(out.contains("Source: https://raw.githubusercontent.com/nickel-org/cargo.json"));
    }

    #[test]
    fn schema_section_not_shown_without_id_or_x_lintel() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = parse_schema(json!({ "type": "object" }));

        render_schema_section(&mut out, &schema, &f);
        assert!(out.is_empty());
    }

    #[test]
    fn schema_section_with_only_x_lintel() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = parse_schema(json!({
            "x-lintel": {
                "source": "https://example.com/schema.json",
                "sourceSha256": "def456"
            },
            "type": "object"
        }));

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
        let schema = parse_schema(json!({
            "type": "object",
            "examples": [
                { "name": "test", "value": 42 }
            ]
        }));

        render_examples_section(&mut out, &schema, &f);
        assert!(out.contains("EXAMPLES"));
        assert!(out.contains("\"name\": \"test\""));
    }

    #[test]
    fn empty_examples_not_shown() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let schema = parse_schema(json!({ "examples": [] }));

        render_examples_section(&mut out, &schema, &f);
        assert!(out.is_empty());
    }

    // --- DEFINITIONS section ---

    #[test]
    fn definitions_not_truncated() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let val = json!({
            "definitions": {
                "myDef": {
                    "type": "object",
                    "description": "This is a very long description that should not be truncated at all because we want to show the full text to users who are reading the documentation"
                }
            }
        });
        let schema = parse_schema(val.clone());
        let root = parse_sv(val);

        render_definitions_section(&mut out, &schema, &root, &f);
        assert!(out.contains("reading the documentation"));
        assert!(!out.contains("..."));
    }

    #[test]
    fn definitions_show_x_lintel_source() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let val = json!({
            "$defs": {
                "Core vocabulary meta-schema": {
                    "type": "object",
                    "x-lintel": {
                        "source": "https://json-schema.org/draft/2020-12/meta/core"
                    },
                    "properties": {
                        "$id": { "type": "string" }
                    }
                }
            }
        });
        let schema = parse_schema(val.clone());
        let root = parse_sv(val);

        render_definitions_section(&mut out, &schema, &root, &f);
        assert!(out.contains("DEFINITIONS"));
        assert!(out.contains("Core vocabulary meta-schema"));
        assert!(out.contains("Source: https://json-schema.org/draft/2020-12/meta/core"));
    }

    // --- VARIANTS section ---

    #[test]
    fn any_of_variants_listed() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let val = json!({
            "anyOf": [
                { "type": "string", "description": "A string value" },
                { "type": "integer", "description": "An integer value" }
            ]
        });
        let schema = parse_schema(val.clone());
        let root = parse_sv(val);

        render_variants_section(&mut out, &schema, &root, &f);
        assert!(out.contains("ANY OF"));
        assert!(out.contains("A string value"));
        assert!(out.contains("An integer value"));
    }

    #[test]
    fn allof_refs_show_description_and_url() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let val = json!({
            "allOf": [
                { "$ref": "#/$defs/base" }
            ],
            "$defs": {
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
        let schema = parse_schema(val.clone());
        let root = parse_sv(val);

        render_variants_section(&mut out, &schema, &root, &f);
        assert!(out.contains("ALL OF"));
        assert!(out.contains("base"));
        // Description is shown for $ref entries
        assert!(out.contains("Base configuration"));
        // $ref URL is shown
        assert!(out.contains("#/$defs/base"));
        // Properties are NOT expanded inline for $ref entries
        assert!(!out.contains("The name"));
        // No numbered indexes
        assert!(!out.contains("(1)"));
    }

    #[test]
    fn allof_uses_same_style_as_oneof_anyof() {
        let mut out = String::new();
        let f = Fmt::plain(80);
        let val = json!({
            "allOf": [
                { "$ref": "#/$defs/First" }
            ],
            "$defs": {
                "First": { "type": "object", "description": "First schema" }
            }
        });
        let schema = parse_schema(val.clone());
        let root = parse_sv(val);

        render_variants_section(&mut out, &schema, &root, &f);
        assert!(out.contains("ALL OF"));
        assert!(out.contains("First"));
        assert!(out.contains("First schema"));
        assert!(out.contains("#/$defs/First"));
        // No numbered indexes
        assert!(!out.contains("(1)"));
    }
}
