use jsonschema_explain::{ExplainOptions, explain, explain_at_path};
use jsonschema_schema::SchemaValue;

fn plain() -> ExplainOptions {
    ExplainOptions {
        color: false,
        syntax_highlight: false,
        width: 80,
        validation_errors: vec![],
    }
}

fn load_fixture() -> SchemaValue {
    let json = include_str!("fixtures/tsconfig.json");
    let mut val: serde_json::Value = serde_json::from_str(json).expect("fixture should parse");
    jsonschema_migrate::migrate_to_2020_12(&mut val);
    serde_json::from_value(val).expect("fixture should deserialize into SchemaValue")
}

/// Properties with both `description` and `markdownDescription` should use
/// the `markdownDescription` (which includes "See more:" links in tsconfig).
#[test]
fn markdown_description_preferred_over_plain() {
    let schema = load_fixture();
    let output = explain(&schema, "tsconfig.json", &plain());

    // markdownDescription adds "See more:" links that plain description lacks
    assert!(
        output.contains("See more:"),
        "markdownDescription with 'See more:' links should appear in output"
    );
}

/// Each compilerOptions property's markdownDescription should appear, not its
/// plain description. Both share the first sentence, but markdownDescription
/// appends a "See more:" URL.
#[test]
fn compiler_options_properties_use_markdown_description() {
    let schema = load_fixture();
    let output = explain_at_path(
        &schema,
        "/$defs/compilerOptionsDefinition/properties/compilerOptions",
        "tsconfig.json",
        &plain(),
    )
    .expect("should resolve compilerOptions path");

    // Every property in the fixture has a markdownDescription with a "See more:" link
    assert!(
        output.contains("https://www.typescriptlang.org/tsconfig#strict"),
        "strict's markdownDescription link should appear"
    );
    assert!(
        output.contains("https://www.typescriptlang.org/tsconfig#target"),
        "target's markdownDescription link should appear"
    );
    assert!(
        output.contains("https://www.typescriptlang.org/tsconfig#module"),
        "module's markdownDescription link should appear"
    );
    assert!(
        output.contains("https://www.typescriptlang.org/tsconfig#noEmit"),
        "noEmit's markdownDescription link should appear"
    );
    assert!(
        output.contains("https://www.typescriptlang.org/tsconfig#declaration"),
        "declaration's markdownDescription link should appear"
    );
}

/// The compilerOptions definition uses plain `description` (no
/// `markdownDescription` at that level in the real tsconfig schema).
#[test]
fn compiler_options_definition_description() {
    let schema = load_fixture();
    let output = explain(&schema, "tsconfig.json", &plain());

    assert!(
        output.contains("Instructs the TypeScript compiler how to compile .ts files."),
        "compilerOptions definition description should appear"
    );
}

/// Pattern-only `anyOf` variants (e.g. `{"pattern": "^..."}`) should render
/// the pattern instead of the opaque `(schema)` fallback.
#[test]
fn pattern_only_variants_show_pattern() {
    let schema = load_fixture();
    let output = explain_at_path(
        &schema,
        "/$defs/compilerOptionsDefinition/properties/compilerOptions",
        "tsconfig.json",
        &plain(),
    )
    .expect("should resolve compilerOptions path");

    // target's anyOf has an enum variant + a pattern-only variant
    assert!(
        !output.contains("(schema)"),
        "pattern-only variants should not render as (schema)"
    );
    assert!(
        output.contains("pattern:"),
        "pattern-only variants should show the pattern"
    );
}

/// `markdownEnumDescriptions` should render each enum value with its
/// description instead of a flat comma-separated list.
#[test]
fn markdown_enum_descriptions_rendered() {
    let schema = load_fixture();
    let output = explain_at_path(
        &schema,
        "/$defs/compilerOptionsDefinition/properties/compilerOptions",
        "tsconfig.json",
        &plain(),
    )
    .expect("should resolve compilerOptions path");

    // moduleResolution has markdownEnumDescriptions
    assert!(
        output.contains("This is the recommended setting for libraries and Node.js applications"),
        "markdownEnumDescriptions should appear next to enum values"
    );
    // Values should be listed vertically with descriptions, indicated by " — "
    assert!(
        output.contains("—"),
        "enum values with descriptions should use '—' separator"
    );
}

/// Enum-only `anyOf` variants should list the values, not just say "enum".
#[test]
fn enum_variants_show_values() {
    let schema = load_fixture();
    let output = explain_at_path(
        &schema,
        "/$defs/compilerOptionsDefinition/properties/compilerOptions",
        "tsconfig.json",
        &plain(),
    )
    .expect("should resolve compilerOptions path");

    // module's anyOf has an enum variant with values like "commonjs", "es6", etc.
    assert!(
        output.contains("commonjs"),
        "enum variant should list actual values like 'commonjs'"
    );
    assert!(
        output.contains("esnext"),
        "enum variant should list actual values like 'esnext'"
    );
    // Should NOT render as the opaque word "enum"
    assert!(
        !output.contains("  - enum\n"),
        "enum variant should not render as bare 'enum'"
    );
}
