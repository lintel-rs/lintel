#![doc = include_str!("../README.md")]

mod fmt;
mod man;
mod render;
mod schema;
mod sections;

use core::fmt::Write;

use jsonschema_schema::{Schema, SchemaValue};

use fmt::{Fmt, format_header, format_type};
use man::{write_description, write_section};
use render::{
    render_additional_properties, render_pattern_properties, render_properties, render_subschema,
};
use schema::{get_description, required_set, schema_type_str};
use sections::{
    render_definitions_section, render_examples_section, render_schema_section,
    render_variants_section,
};

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
    /// Show extended details like `$comment` annotations.
    pub extended: bool,
}

/// Render a JSON Schema as human-readable terminal documentation.
///
/// `schema` is a parsed `SchemaValue`. `name` is a display name
/// (e.g. from a catalog entry). `opts` controls color, syntax highlighting,
/// and terminal width.
pub fn explain(schema: &SchemaValue, name: &str, opts: &ExplainOptions) -> String {
    let Some(s) = schema.as_schema() else {
        // Bool schema — just show header
        let mut out = String::new();
        let f = Fmt::from_opts(opts);
        let upper = name.to_uppercase();
        let header = format_header(&upper, name, opts.width);
        let _ = writeln!(out, "{}{header}{}\n", f.bold, f.reset);
        return out;
    };
    explain_schema(s, schema, name, opts)
}

/// Render a `Schema` as human-readable terminal documentation.
fn explain_schema(s: &Schema, root: &SchemaValue, name: &str, opts: &ExplainOptions) -> String {
    let mut out = String::new();
    let f = Fmt::from_opts(opts);

    // In extended mode, show raw schema structure; otherwise flatten allOf.
    // absolute() rewrites local $refs to absolute URLs using the schema's $id.
    let s = if f.extended {
        s.clone()
    } else {
        s.absolute().flatten(root)
    };
    let render_root = SchemaValue::Schema(Box::new(s.clone()));

    let title = s.title.as_deref();
    let description = get_description(&s);

    let label = std::path::Path::new(name)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(name);
    let center = title.unwrap_or(label);
    let header = format_header(label, center, opts.width);
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

    if let Some(t) = title {
        write_section(&mut out, "TITLE", &f);
        let _ = writeln!(out, "    {}{t}{}", f.bold, f.reset);
        out.push('\n');
    }

    if let Some(desc) = description {
        write_section(&mut out, "DESCRIPTION", &f);
        write_description(&mut out, desc, &f, "    ");
        out.push('\n');
    }

    if f.extended
        && let Some(ref comment) = s.comment
    {
        write_section(&mut out, "COMMENT", &f);
        write_description(&mut out, comment, &f, "    ");
        out.push('\n');
    }

    render_schema_section(&mut out, &s, &f);

    let type_str = schema_type_str(&s);
    if let Some(ref ty) = type_str {
        write_section(&mut out, "TYPE", &f);
        let _ = writeln!(out, "    {}", format_type(ty, &f));
        out.push('\n');
    }

    let required = required_set(&s);
    if !s.properties.is_empty() {
        write_section(&mut out, "PROPERTIES", &f);
        render_properties(&mut out, &s.properties, &required, &render_root, &f, 1);
        out.push('\n');
    }

    render_pattern_properties(&mut out, &s, root, &f, 0, "    ");
    render_additional_properties(&mut out, &s, root, &f, 0, "    ");

    // Root-level if/then/else
    if s.if_.is_some() {
        use crate::schema::variant_summary;
        write_section(&mut out, "CONDITIONAL", &f);
        if let Some(ref if_sv) = s.if_ {
            let summary = variant_summary(if_sv, root, &f);
            let _ = writeln!(out, "    If: {summary}");
        }
        if let Some(ref then_sv) = s.then_ {
            let summary = variant_summary(then_sv, root, &f);
            let _ = writeln!(out, "    Then: {summary}");
        }
        if let Some(ref else_sv) = s.else_ {
            let summary = variant_summary(else_sv, root, &f);
            let _ = writeln!(out, "    Else: {summary}");
        }
        out.push('\n');
    }

    if type_str.as_deref() == Some("array")
        && let Some(ref items) = s.items
    {
        write_section(&mut out, "ITEMS", &f);
        render_subschema(&mut out, items, &render_root, &f, 1);
        out.push('\n');
    }

    render_examples_section(&mut out, &s, &f);
    // Resolve allOf/oneOf/anyOf $refs against the original root — the flattened
    // schema may have pruned merged $defs entries.
    render_variants_section(&mut out, &s, root, &f);
    render_definitions_section(&mut out, &s, &render_root, &f);

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
    schema: &SchemaValue,
    pointer: &str,
    name: &str,
    opts: &ExplainOptions,
) -> Result<String, String> {
    let sub = navigate_pointer(schema, schema, pointer)?;
    Ok(explain(sub, name, opts))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::fmt::{BLUE, BOLD, CYAN, GREEN, RESET, format_header, format_type};
    use serde_json::json;

    /// Parse a JSON value into a `SchemaValue`, running migration first
    /// to ensure compatibility with older JSON Schema drafts.
    fn sv(val: serde_json::Value) -> SchemaValue {
        SchemaValue::Schema(Box::new(jsonschema_migrate::migrate(val).unwrap()))
    }

    fn plain() -> ExplainOptions {
        ExplainOptions {
            color: false,
            syntax_highlight: false,
            width: 80,
            validation_errors: vec![],
            extended: false,
        }
    }

    fn colored() -> ExplainOptions {
        ExplainOptions {
            color: true,
            syntax_highlight: true,
            width: 80,
            validation_errors: vec![],
            extended: false,
        }
    }

    #[test]
    fn simple_object_schema() {
        let schema = sv(json!({
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
        }));

        let output = explain(&schema, "test", &plain());
        assert!(output.contains("TITLE"));
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
        let schema = sv(json!({
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
        }));

        let output = explain(&schema, "nested", &plain());
        assert!(output.contains("config (object)"));
        assert!(output.contains("debug (boolean)"));
        assert!(output.contains("Enable debug mode"));
    }

    #[test]
    fn enum_values_listed() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["low", "medium", "high"]
                }
            }
        }));

        let output = explain(&schema, "enum-test", &plain());
        assert!(output.contains("Values: low, medium, high"));
    }

    #[test]
    fn required_properties_marked() {
        let schema = sv(json!({
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
        }));

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
        let schema = sv(json!({
            "type": "string",
            "description": "A plain string type"
        }));

        let output = explain(&schema, "simple", &plain());
        assert!(!output.contains("TITLE"));
        assert!(output.contains("DESCRIPTION"));
        assert!(output.contains("A plain string type"));
        assert!(!output.contains("PROPERTIES"));
    }

    #[test]
    fn color_output_contains_ansi() {
        let schema = sv(json!({
            "title": "Colored",
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            }
        }));

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
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "default": 8080
                }
            }
        }));

        let output = explain(&schema, "defaults", &plain());
        assert!(output.contains("Default: 8080"));
    }

    #[test]
    fn long_default_wraps() {
        let long_val = "First of: `tsconfig.json` rootDir if specified, directory containing `tsconfig.json`, or cwd if no `tsconfig.json` is loaded.";
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "declarationDir": {
                    "type": "string",
                    "default": long_val
                }
            }
        }));

        let output = explain(&schema, "wrap-test", &plain());
        // Short defaults stay on one line, but this is too long — the label
        // should appear on its own line with the value wrapped below.
        assert!(
            output.contains("Default:\n"),
            "long default should wrap onto next line\n{output}"
        );
        assert!(
            output.contains(long_val),
            "full default value should appear in output\n{output}"
        );
    }

    #[test]
    fn ref_resolution() {
        let schema = sv(json!({
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
        }));

        let output = explain(&schema, "ref-test", &plain());
        assert!(output.contains("item (object)"));
        assert!(output.contains("An item definition"));
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
        let schema = sv(json!({"type": "object", "title": "Test"}));
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
    fn prefers_markdown_description() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Plain description",
                    "markdownDescription": "Rich **markdown** description"
                }
            }
        }));

        let output = explain(&schema, "test", &plain());
        assert!(output.contains("Rich **markdown** description"));
        assert!(!output.contains("Plain description"));
    }

    #[test]
    fn no_premature_wrapping() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "string",
                    "description": "This is a very long description that should not be wrapped at 72 characters because we want the pager to handle wrapping at the terminal width instead"
                }
            }
        }));

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
        let schema = sv(json!({
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
        }));

        let output = explain_at_path(&schema, "/properties/config", "test", &plain()).unwrap();
        assert!(output.contains("Config"));
        assert!(output.contains("Configuration settings"));
        assert!(output.contains("debug (boolean)"));
        // Should NOT contain the sibling "name" property
        assert!(!output.contains("The name field"));
    }

    #[test]
    fn explain_at_path_root_pointer_shows_full_schema() {
        let schema = sv(json!({
            "type": "object",
            "title": "Root",
            "properties": {
                "a": { "type": "string" }
            }
        }));

        let output = explain_at_path(&schema, "", "test", &plain()).unwrap();
        assert!(output.contains("Root"));
        assert!(output.contains("a (string)"));
    }

    #[test]
    fn explain_at_path_resolves_ref() {
        let schema = sv(json!({
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
        }));

        let output = explain_at_path(&schema, "/properties/item", "test", &plain()).unwrap();
        assert!(output.contains("Item"));
        assert!(output.contains("An item"));
        assert!(output.contains("id (integer)"));
    }

    #[test]
    fn explain_at_path_bad_pointer_errors() {
        let schema = sv(json!({"type": "object"}));
        let err = explain_at_path(&schema, "/nonexistent/path", "test", &plain());
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("nonexistent"));
    }

    #[test]
    fn property_examples_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "examples": ["TAG-ID", "DUNS"]
                }
            }
        }));

        let output = explain(&schema, "examples-test", &plain());
        assert!(output.contains("Examples: \"TAG-ID\", \"DUNS\""));
    }

    #[test]
    fn explain_at_path_deep_nesting() {
        let schema = sv(json!({
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
        }));

        let output =
            explain_at_path(&schema, "/properties/a/properties/b", "test", &plain()).unwrap();
        assert!(output.contains("Deep"));
        assert!(output.contains("c (string)"));
        assert!(output.contains("Deeply nested"));
    }

    #[test]
    fn numeric_constraints_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535
                }
            }
        }));

        let output = explain(&schema, "constraints", &plain());
        assert!(output.contains("Constraints:"));
        assert!(output.contains("min=1"));
        assert!(output.contains("max=65535"));
    }

    #[test]
    fn string_constraints_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "format": "email",
                    "minLength": 5,
                    "maxLength": 255,
                    "pattern": "^[^@]+@[^@]+$"
                }
            }
        }));

        let output = explain(&schema, "constraints", &plain());
        assert!(output.contains("format=email"));
        assert!(output.contains("minLength=5"));
        assert!(output.contains("maxLength=255"));
        assert!(output.contains("pattern=^[^@]+@[^@]+$"));
    }

    #[test]
    fn array_constraints_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "maxItems": 10,
                    "uniqueItems": true
                }
            }
        }));

        let output = explain(&schema, "constraints", &plain());
        assert!(output.contains("minItems=1"));
        assert!(output.contains("maxItems=10"));
        assert!(output.contains("unique"));
    }

    #[test]
    fn exclusive_bounds_and_multiple_of_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "score": {
                    "type": "number",
                    "exclusiveMinimum": 0,
                    "exclusiveMaximum": 100,
                    "multipleOf": 0.5
                }
            }
        }));

        let output = explain(&schema, "constraints", &plain());
        assert!(output.contains("exclusiveMin=0"));
        assert!(output.contains("exclusiveMax=100"));
        assert!(output.contains("multipleOf=0.5"));
    }

    #[test]
    fn no_constraints_line_when_none() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Just a name"
                }
            }
        }));

        let output = explain(&schema, "no-constraints", &plain());
        assert!(!output.contains("Constraints:"));
    }

    // --- TITLE section ---

    #[test]
    fn title_section_shows_schema_title() {
        let schema = sv(json!({
            "title": "My Schema",
            "type": "object"
        }));

        let output = explain(&schema, "display-name", &plain());
        assert!(output.contains("TITLE"));
        assert!(output.contains("My Schema"));
    }

    #[test]
    fn title_section_hidden_without_schema_title() {
        let schema = sv(json!({ "type": "object" }));

        let output = explain(&schema, "fallback-name", &plain());
        assert!(!output.contains("TITLE"));
        // display name still appears in the header banner (not uppercased)
        assert!(output.contains("fallback-name"));
    }

    #[test]
    fn schema_section_appears_after_description() {
        let schema = sv(json!({
            "$id": "https://json.schemastore.org/cargo.json",
            "type": "object",
            "title": "Cargo",
            "description": "Cargo manifest schema"
        }));

        let output = explain(&schema, "cargo", &plain());
        let desc_pos = output.find("DESCRIPTION").unwrap();
        let schema_pos = output.find("SCHEMA").unwrap();
        let type_pos = output.find("TYPE").unwrap();
        assert!(
            desc_pos < schema_pos,
            "SCHEMA should appear after DESCRIPTION"
        );
        assert!(schema_pos < type_pos, "SCHEMA should appear before TYPE");
    }

    // --- New keyword rendering ---

    #[test]
    fn comment_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "x": {
                    "type": "string",
                    "$comment": "See https://example.com for details"
                }
            }
        }));

        let extended = ExplainOptions {
            extended: true,
            ..plain()
        };
        let output = explain(&schema, "comment-test", &extended);
        assert!(output.contains("Comment:"));
        assert!(output.contains("See https://example.com for details"));
    }

    #[test]
    fn comment_hidden_by_default() {
        let schema = sv(json!({
            "type": "object",
            "$comment": "Hidden comment",
            "properties": {
                "x": {
                    "type": "string",
                    "$comment": "Also hidden"
                }
            }
        }));

        let output = explain(&schema, "comment-test", &plain());
        assert!(!output.contains("Comment"));
        assert!(!output.contains("Hidden comment"));
        assert!(!output.contains("Also hidden"));
    }

    #[test]
    fn root_comment_shown() {
        let schema = sv(json!({
            "$comment": "Root level comment",
            "type": "object"
        }));

        let extended = ExplainOptions {
            extended: true,
            ..plain()
        };
        let output = explain(&schema, "comment-test", &extended);
        assert!(output.contains("COMMENT"));
        assert!(output.contains("Root level comment"));
        // No double blank lines — only one blank line between sections
        assert!(
            !output.contains("\n\n\n"),
            "should not have triple newlines (double blank lines)\n{output}"
        );
    }

    #[test]
    fn additional_properties_false() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        }));

        let output = explain(&schema, "ap-test", &plain());
        assert!(output.contains("Additional properties: not allowed"));
    }

    #[test]
    fn additional_properties_schema() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": { "type": "string" }
        }));

        let output = explain(&schema, "ap-test", &plain());
        assert!(output.contains("Additional properties: string"));
    }

    #[test]
    fn additional_properties_true_not_shown() {
        let schema = sv(json!({
            "type": "object",
            "additionalProperties": true
        }));

        let output = explain(&schema, "ap-test", &plain());
        assert!(!output.contains("Additional properties"));
    }

    #[test]
    fn pattern_properties_shown() {
        let schema = sv(json!({
            "type": "object",
            "patternProperties": {
                "^x-": { "type": "object", "description": "Extension properties" }
            }
        }));

        let output = explain(&schema, "pp-test", &plain());
        assert!(output.contains("Pattern properties:"));
        assert!(output.contains("^x-"));
        assert!(output.contains("Extension properties"));
    }

    #[test]
    fn if_then_else_shown() {
        let schema = sv(json!({
            "type": "object",
            "if": { "properties": { "type": { "const": "a" } } },
            "then": { "properties": { "value": { "type": "string" } } },
            "else": { "properties": { "value": { "type": "integer" } } }
        }));

        let output = explain(&schema, "cond-test", &plain());
        assert!(output.contains("CONDITIONAL"));
        assert!(output.contains("If:"));
        assert!(output.contains("Then:"));
        assert!(output.contains("Else:"));
    }

    #[test]
    fn not_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "x": {
                    "not": { "type": "string" }
                }
            }
        }));

        let output = explain(&schema, "not-test", &plain());
        assert!(output.contains("Not: string"));
    }

    #[test]
    fn dependent_required_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "dependentRequired": {
                        "bar": ["foo"]
                    }
                }
            }
        }));

        let output = explain(&schema, "dr-test", &plain());
        assert!(output.contains("Dependent required:"));
        assert!(output.contains("\"bar\""));
        assert!(output.contains("\"foo\""));
    }

    #[test]
    fn property_names_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "propertyNames": { "pattern": "^[a-z]+$" }
                }
            }
        }));

        let output = explain(&schema, "pn-test", &plain());
        assert!(output.contains("Property names: pattern=^[a-z]+$"));
    }

    #[test]
    fn prefix_items_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "tuple": {
                    "type": "array",
                    "prefixItems": [
                        { "type": "string" },
                        { "type": "integer" }
                    ]
                }
            }
        }));

        let output = explain(&schema, "prefix-test", &plain());
        assert!(output.contains("Tuple items:"));
        assert!(output.contains("[0]: string"));
        assert!(output.contains("[1]: integer"));
    }

    #[test]
    fn contains_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "arr": {
                    "type": "array",
                    "contains": { "type": "string" },
                    "minContains": 1
                }
            }
        }));

        let output = explain(&schema, "contains-test", &plain());
        assert!(output.contains("Contains: string"));
        assert!(output.contains("minContains=1"));
    }

    #[test]
    fn read_only_tag_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "readOnly": true
                }
            }
        }));

        let output = explain(&schema, "ro-test", &plain());
        assert!(output.contains("[READ-ONLY]"));
    }

    #[test]
    fn write_only_tag_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "password": {
                    "type": "string",
                    "writeOnly": true
                }
            }
        }));

        let output = explain(&schema, "wo-test", &plain());
        assert!(output.contains("[WRITE-ONLY]"));
    }

    #[test]
    fn content_media_type_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "string",
                    "contentMediaType": "application/json",
                    "contentEncoding": "base64"
                }
            }
        }));

        let output = explain(&schema, "content-test", &plain());
        assert!(output.contains("Content: application/json (base64)"));
    }

    #[test]
    fn markdown_enum_descriptions_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["fast", "safe", "auto"],
                    "markdownEnumDescriptions": [
                        "Optimizes for speed",
                        "Optimizes for safety",
                        "Automatically chooses"
                    ]
                }
            }
        }));

        let output = explain(&schema, "enum-desc-test", &plain());
        assert!(output.contains("Values:"));
        assert!(output.contains("fast"));
        assert!(output.contains("Optimizes for speed"));
        assert!(output.contains("—"));
    }

    #[test]
    fn min_max_contains_in_constraints() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "arr": {
                    "type": "array",
                    "minContains": 2,
                    "maxContains": 5
                }
            }
        }));

        let output = explain(&schema, "contains-constraints", &plain());
        assert!(output.contains("minContains=2"));
        assert!(output.contains("maxContains=5"));
    }

    #[test]
    fn dependent_schemas_shown() {
        let schema = sv(json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "dependentSchemas": {
                        "credit_card": {
                            "properties": {
                                "billing_address": { "type": "string" }
                            }
                        }
                    }
                }
            }
        }));

        let output = explain(&schema, "ds-test", &plain());
        assert!(output.contains("Dependent schemas:"));
        assert!(output.contains("\"credit_card\""));
    }
}
