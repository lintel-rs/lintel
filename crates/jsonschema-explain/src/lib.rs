#![doc = include_str!("../README.md")]

mod fmt;
mod man;
mod render;
mod schema;

use core::fmt::Write;

use serde_json::Value;

use fmt::{COMPOSITION_KEYWORDS, Fmt, format_header, format_type, format_type_suffix};
use man::{write_description, write_section};
use render::{render_properties, render_subschema, render_variant_block};
use schema::{get_description, required_set, resolve_ref, schema_type_str};

pub use schema::{navigate_pointer, resolve_ref as resolve_schema_ref};

/// A validation error to display in the VALIDATION ERRORS section.
pub struct ExplainError {
    /// JSON Pointer to the failing instance (e.g. `/badges/appveyor`).
    pub instance_path: String,
    /// Human-readable error message.
    pub message: String,
}

/// Display options for rendering schema documentation.
pub struct ExplainOptions {
    /// Use ANSI color codes in output.
    pub color: bool,
    /// Syntax-highlight fenced code blocks in descriptions.
    pub syntax_highlight: bool,
    /// Terminal width in columns for layout.
    pub width: usize,
    /// Validation errors to show before the schema documentation.
    pub validation_errors: Vec<ExplainError>,
}

/// Render a JSON Schema as human-readable terminal documentation.
///
/// `schema` is a parsed JSON Schema value. `name` is a display name
/// (e.g. from a catalog entry). `opts` controls color, syntax highlighting,
/// and terminal width.
pub fn explain(schema: &Value, name: &str, opts: &ExplainOptions) -> String {
    let mut out = String::new();
    let f = Fmt::from_opts(opts);

    let upper = name.to_uppercase();
    let header = format_header(&upper, "JSON Schema", opts.width);
    let _ = writeln!(out, "{}{header}{}\n", f.bold, f.reset);

    if !opts.validation_errors.is_empty() {
        write_section(&mut out, "VALIDATION ERRORS", &f);
        for err in &opts.validation_errors {
            let path = if err.instance_path.is_empty() {
                "(root)"
            } else {
                &err.instance_path
            };
            let _ = writeln!(out, "    {}{path}{}: {}", f.red, f.reset, err.message);
        }
        out.push('\n');
    }

    let title = schema.get("title").and_then(Value::as_str).unwrap_or(name);
    let description = get_description(schema);

    write_section(&mut out, "NAME", &f);
    let _ = writeln!(out, "    {}{title}{}", f.bold, f.reset);
    out.push('\n');

    if let Some(desc) = description {
        write_section(&mut out, "DESCRIPTION", &f);
        write_description(&mut out, desc, &f, "    ");
        out.push('\n');
    }

    let type_str = schema_type_str(schema);
    if let Some(ref ty) = type_str {
        write_section(&mut out, "TYPE", &f);
        let _ = writeln!(out, "    {}", format_type(ty, &f));
        out.push('\n');
    }

    let required = required_set(schema);
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        write_section(&mut out, "PROPERTIES", &f);
        render_properties(&mut out, props, &required, schema, &f, 1);
        out.push('\n');
    }

    if type_str.as_deref() == Some("array")
        && let Some(items) = schema.get("items")
    {
        write_section(&mut out, "ITEMS", &f);
        render_subschema(&mut out, items, schema, &f, 1);
        out.push('\n');
    }

    render_variants_section(&mut out, schema, &f);
    render_definitions_section(&mut out, schema, &f);

    out
}

/// Render a sub-schema at a given JSON Pointer path.
///
/// Navigates `pointer` within `schema`, then renders the sub-schema the same
/// way [`explain`] renders the root. `name` is used in the header.
///
/// # Errors
///
/// Returns an error if the pointer cannot be resolved within the schema.
pub fn explain_at_path(
    schema: &Value,
    pointer: &str,
    name: &str,
    opts: &ExplainOptions,
) -> Result<String, String> {
    let sub = navigate_pointer(schema, schema, pointer)?;
    Ok(explain(sub, name, opts))
}

/// Render `oneOf`/`anyOf`/`allOf` variant sections.
fn render_variants_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
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

/// Render the DEFINITIONS section (`$defs`/`definitions`).
fn render_definitions_section(out: &mut String, schema: &Value, f: &Fmt<'_>) {
    for defs_key in &["$defs", "definitions"] {
        if let Some(defs) = schema.get(*defs_key).and_then(Value::as_object)
            && !defs.is_empty()
        {
            write_section(out, "DEFINITIONS", f);
            for (def_name, def_schema) in defs {
                let ty = schema_type_str(def_schema).unwrap_or_default();
                let suffix = format_type_suffix(&ty, f);
                let _ = writeln!(out, "    {}{def_name}{}{suffix}", f.green, f.reset);
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
    use crate::fmt::{BLUE, BOLD, CYAN, GREEN, RESET};
    use serde_json::json;

    fn plain() -> ExplainOptions {
        ExplainOptions {
            color: false,
            syntax_highlight: false,
            width: 80,
            validation_errors: vec![],
        }
    }

    fn colored() -> ExplainOptions {
        ExplainOptions {
            color: true,
            syntax_highlight: true,
            width: 80,
            validation_errors: vec![],
        }
    }

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

        let output = explain(&schema, "test", &plain());
        assert!(output.contains("NAME"));
        assert!(output.contains("Test"));
        assert!(!output.contains("Test - A test schema"));
        assert!(output.contains("DESCRIPTION"));
        assert!(output.contains("A test schema"));
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

        let output = explain(&schema, "nested", &plain());
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

        let output = explain(&schema, "enum-test", &plain());
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

        let output = explain(&schema, "required-test", &plain());
        assert!(output.contains("name (string, *required)"));
        assert!(output.contains("optional (string)"));
        assert!(!output.contains("optional (string, *required)"));

        // Required fields should appear before optional fields
        let name_pos = output
            .find("name (string")
            .expect("name field should be present");
        let optional_pos = output
            .find("optional (string")
            .expect("optional field should be present");
        assert!(
            name_pos < optional_pos,
            "required field 'name' should appear before optional field"
        );
    }

    #[test]
    fn schema_with_no_properties_handled() {
        let schema = json!({
            "type": "string",
            "description": "A plain string type"
        });

        let output = explain(&schema, "simple", &plain());
        assert!(output.contains("NAME"));
        assert!(output.contains("DESCRIPTION"));
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

        let colored_out = explain(&schema, "colored", &colored());
        let plain_out = explain(&schema, "colored", &plain());

        assert!(colored_out.contains(BOLD));
        assert!(colored_out.contains(RESET));
        assert!(colored_out.contains(CYAN));
        assert!(colored_out.contains(GREEN));
        assert!(!plain_out.contains(BOLD));
        assert!(!plain_out.contains(RESET));
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

        let output = explain(&schema, "defaults", &plain());
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

        let output = explain(&schema, "ref-test", &plain());
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

        let output = explain(&schema, "union", &plain());
        assert!(output.contains("ANY OF"));
        assert!(output.contains("A string value"));
        assert!(output.contains("An integer value"));
    }

    #[test]
    fn format_header_centers() {
        let h = format_header("TEST", "JSON Schema", 76);
        assert!(h.starts_with("TEST"));
        assert!(h.ends_with("TEST"));
        assert!(h.contains("JSON Schema"));
        assert_eq!(h.len(), 76);
    }

    #[test]
    fn format_header_uses_full_width() {
        let h = format_header("CARGO MANIFEST", "JSON Schema", 120);
        assert_eq!(h.len(), 120);
        assert!(h.starts_with("CARGO MANIFEST"));
        assert!(h.ends_with("CARGO MANIFEST"));
    }

    #[test]
    fn explain_output_uses_width() {
        let schema = json!({"type": "object", "title": "Test"});
        let opts_120 = ExplainOptions {
            width: 120,
            ..plain()
        };
        let output_80 = explain(&schema, "test", &plain());
        let output_120 = explain(&schema, "test", &opts_120);
        let header_80 = output_80.lines().next().unwrap();
        let header_120 = output_120.lines().next().unwrap();
        assert_eq!(header_80.len(), 80);
        assert_eq!(header_120.len(), 120);
    }

    #[test]
    fn inline_backtick_colorization() {
        let f = Fmt::color(80);
        let result = markdown_to_ansi::render_inline("Use `foo` and `bar`", &f.md_opts(None));
        assert!(result.contains(BLUE));
        assert!(result.contains("foo"));
        assert!(result.contains("bar"));
        assert!(!result.contains('`'));
    }

    #[test]
    fn inline_bold_rendering() {
        let f = Fmt::color(80);
        let result =
            markdown_to_ansi::render_inline("This is **important** text", &f.md_opts(None));
        assert!(result.contains(BOLD));
        assert!(result.contains("important"));
        assert!(!result.contains("**"));
    }

    #[test]
    fn inline_markdown_link() {
        let f = Fmt::color(80);
        let result = markdown_to_ansi::render_inline(
            "See [docs](https://example.com) here",
            &f.md_opts(None),
        );
        assert!(result.contains("docs"));
        assert!(result.contains("https://example.com"));
        assert!(result.contains("\x1b]8;;"));
    }

    #[test]
    fn inline_raw_url() {
        let f = Fmt::color(80);
        let result =
            markdown_to_ansi::render_inline("See more: https://example.com/foo", &f.md_opts(None));
        assert!(result.contains("https://example.com/foo"));
    }

    #[test]
    fn type_formatting_union() {
        let f = Fmt::plain(80);
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

        let output = explain(&schema, "test", &plain());
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

        let output = explain(&schema, "test", &plain());
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

        let output = explain(&schema, "test", &plain());
        assert!(output.contains("Rich **markdown** description"));
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

        let output = explain(&schema, "test", &plain());
        let desc_line = output
            .lines()
            .find(|l| l.contains("This is a very long"))
            .expect("description line should be present");
        assert!(desc_line.contains("terminal width instead"));
    }

    // --- explain_at_path ---

    #[test]
    fn explain_at_path_shows_sub_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name field"
                },
                "config": {
                    "type": "object",
                    "title": "Config",
                    "description": "Configuration settings",
                    "properties": {
                        "debug": { "type": "boolean" }
                    }
                }
            }
        });

        let output = explain_at_path(&schema, "/properties/config", "test", &plain()).unwrap();
        assert!(output.contains("Config"));
        assert!(output.contains("Configuration settings"));
        assert!(output.contains("debug (boolean)"));
        // Should NOT contain the sibling "name" property
        assert!(!output.contains("The name field"));
    }

    #[test]
    fn explain_at_path_root_pointer_shows_full_schema() {
        let schema = json!({
            "type": "object",
            "title": "Root",
            "properties": {
                "a": { "type": "string" }
            }
        });

        let output = explain_at_path(&schema, "", "test", &plain()).unwrap();
        assert!(output.contains("Root"));
        assert!(output.contains("a (string)"));
    }

    #[test]
    fn explain_at_path_resolves_ref() {
        let schema = json!({
            "type": "object",
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "title": "Item",
                    "description": "An item",
                    "properties": {
                        "id": { "type": "integer" }
                    }
                }
            }
        });

        let output = explain_at_path(&schema, "/properties/item", "test", &plain()).unwrap();
        assert!(output.contains("Item"));
        assert!(output.contains("An item"));
        assert!(output.contains("id (integer)"));
    }

    #[test]
    fn explain_at_path_bad_pointer_errors() {
        let schema = json!({"type": "object"});
        let err = explain_at_path(&schema, "/nonexistent/path", "test", &plain());
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("nonexistent"));
    }

    #[test]
    fn explain_at_path_deep_nesting() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {
                    "type": "object",
                    "properties": {
                        "b": {
                            "type": "object",
                            "title": "Deep",
                            "properties": {
                                "c": { "type": "string", "description": "Deeply nested" }
                            }
                        }
                    }
                }
            }
        });

        let output =
            explain_at_path(&schema, "/properties/a/properties/b", "test", &plain()).unwrap();
        assert!(output.contains("Deep"));
        assert!(output.contains("c (string)"));
        assert!(output.contains("Deeply nested"));
    }
}
