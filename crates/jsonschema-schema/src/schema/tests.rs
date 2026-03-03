use alloc::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::{Value, json};
use url::Url;

use super::*;

#[test]
fn round_trip_simple_schema() {
    let json = json!({
        "type": "object",
        "title": "Test",
        "properties": {
            "name": { "type": "string" }
        }
    });
    let schema: Schema = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(schema.title.as_deref(), Some("Test"));
    assert!(!schema.properties.is_empty());

    let back = serde_json::to_value(&schema).unwrap();
    assert_eq!(back["type"], "object");
    assert_eq!(back["title"], "Test");
}

#[test]
fn bool_schema_value() {
    let json = json!(true);
    let sv: SchemaValue = serde_json::from_value(json).unwrap();
    assert!(matches!(sv, SchemaValue::Bool(true)));
    assert!(sv.as_schema().is_none());
}

#[test]
fn schema_value_object() {
    let json = json!({"type": "string"});
    let sv: SchemaValue = serde_json::from_value(json).unwrap();
    let s = sv.as_schema().unwrap();
    assert!(matches!(
        s.type_,
        Some(TypeValue::Single(SimpleType::String))
    ));
}

#[test]
fn type_value_single() {
    let json = json!("string");
    let tv: TypeValue = serde_json::from_value(json).unwrap();
    assert!(matches!(tv, TypeValue::Single(SimpleType::String)));
}

#[test]
fn type_value_union() {
    let json = json!(["string", "null"]);
    let tv: TypeValue = serde_json::from_value(json).unwrap();
    assert!(matches!(tv, TypeValue::Union(ref v) if v.len() == 2));
}

#[test]
fn simple_type_display() {
    assert_eq!(SimpleType::Array.to_string(), "array");
    assert_eq!(SimpleType::Boolean.to_string(), "boolean");
    assert_eq!(SimpleType::Integer.to_string(), "integer");
    assert_eq!(SimpleType::Null.to_string(), "null");
    assert_eq!(SimpleType::Number.to_string(), "number");
    assert_eq!(SimpleType::Object.to_string(), "object");
    assert_eq!(SimpleType::String.to_string(), "string");
}

#[test]
fn simple_type_round_trip() {
    for ty in [
        SimpleType::Array,
        SimpleType::Boolean,
        SimpleType::Integer,
        SimpleType::Null,
        SimpleType::Number,
        SimpleType::Object,
        SimpleType::String,
    ] {
        let json = serde_json::to_value(ty).unwrap();
        let back: SimpleType = serde_json::from_value(json).unwrap();
        assert_eq!(ty, back);
    }
}

#[test]
fn description_prefers_markdown() {
    let schema = Schema {
        description: Some("plain".into()),
        markdown_description: Some("**rich**".into()),
        ..Default::default()
    };
    assert_eq!(schema.description(), Some("**rich**"));
}

#[test]
fn description_falls_back() {
    let schema = Schema {
        description: Some("plain".into()),
        ..Default::default()
    };
    assert_eq!(schema.description(), Some("plain"));
}

#[test]
fn type_str_simple() {
    let schema = Schema {
        type_: Some(TypeValue::Single(SimpleType::String)),
        ..Default::default()
    };
    assert_eq!(schema.type_str().as_deref(), Some("string"));
}

#[test]
fn type_str_union() {
    let schema = Schema {
        type_: Some(TypeValue::Union(vec![SimpleType::String, SimpleType::Null])),
        ..Default::default()
    };
    assert_eq!(schema.type_str().as_deref(), Some("string | null"));
}

#[test]
fn type_str_array_with_items() {
    let items = SchemaValue::Schema(Box::new(Schema {
        type_: Some(TypeValue::Single(SimpleType::String)),
        ..Default::default()
    }));
    let schema = Schema {
        type_: Some(TypeValue::Single(SimpleType::Array)),
        items: Some(Box::new(items)),
        ..Default::default()
    };
    assert_eq!(schema.type_str().as_deref(), Some("string[]"));
}

#[test]
fn type_str_ref() {
    let schema = Schema {
        ref_: Some("#/$defs/Foo".into()),
        ..Default::default()
    };
    assert_eq!(schema.type_str().as_deref(), Some("Foo"));
}

#[test]
fn is_deprecated_default_false() {
    let schema = Schema::default();
    assert!(!schema.is_deprecated());
}

#[test]
fn is_deprecated_true() {
    let schema = Schema {
        deprecated: true,
        ..Default::default()
    };
    assert!(schema.is_deprecated());
}

#[test]
fn required_set_empty() {
    let schema = Schema::default();
    assert!(schema.required_set().is_empty());
}

#[test]
fn required_set_values() {
    let schema = Schema {
        required: Some(vec!["a".into(), "b".into()]),
        ..Default::default()
    };
    assert_eq!(schema.required_set(), &["a", "b"]);
}

#[test]
fn extra_fields_preserved() {
    let json = json!({
        "type": "object",
        "x-custom": "value",
        "x-another": 42
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    assert_eq!(schema.extra.get("x-custom").unwrap(), "value");
    assert_eq!(schema.extra.get("x-another").unwrap(), 42);
}

#[test]
fn x_taplo_deserialization() {
    let json = json!({
        "type": "object",
        "x-taplo": {
            "hidden": true,
            "docs": {
                "main": "Main docs"
            }
        }
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    let taplo = schema.x_taplo.unwrap();
    assert_eq!(taplo.hidden, Some(true));
    assert_eq!(taplo.docs.unwrap().main.as_deref(), Some("Main docs"));
}

#[test]
fn x_intellij_deserialization() {
    let json = json!({
        "type": "string",
        "enum": ["system", "local"],
        "x-intellij-html-description": "<b>bold</b> description",
        "x-intellij-language-injection": "Shell Script",
        "x-intellij-enum-metadata": {
            "system": { "description": "Use system nginx" },
            "local": { "description": "Use local nginx process" }
        }
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    assert_eq!(
        schema.x_intellij.html_description.as_deref(),
        Some("<b>bold</b> description")
    );
    assert_eq!(
        schema.x_intellij.language_injection.as_deref(),
        Some("Shell Script")
    );
    let meta = schema.x_intellij.enum_metadata.unwrap();
    assert_eq!(meta.len(), 2);
    assert_eq!(
        meta["system"].description.as_deref(),
        Some("Use system nginx")
    );
}

#[test]
fn x_intellij_fixture_huskyrc() {
    let content = include_str!("../../tests/fixtures/huskyrc.json");
    let value: Value = serde_json::from_str(content).expect("parse huskyrc.json");
    let mut migrated = value;
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    let schema: Schema = serde_json::from_value(migrated).expect("deserialize huskyrc schema");

    // definitions/hook has x-intellij-language-injection
    let hook = schema.defs.as_ref().expect("defs present")["hook"]
        .as_schema()
        .expect("hook is a schema");
    assert_eq!(
        hook.x_intellij.language_injection.as_deref(),
        Some("Shell Script")
    );

    // hooks/applypatch-msg has x-intellij-html-description
    let hooks = &schema.properties["hooks"]
        .as_schema()
        .expect("hooks is a schema");
    let applypatch = &hooks.properties["applypatch-msg"]
        .as_schema()
        .expect("applypatch-msg is a schema");
    assert!(
        applypatch
            .x_intellij
            .html_description
            .as_ref()
            .expect("html_description present")
            .starts_with("<p>This hook is invoked by")
    );

    // Neither should leak into extra
    assert!(!hook.extra.contains_key("x-intellij-language-injection"));
    assert!(!applypatch.extra.contains_key("x-intellij-html-description"));
}

#[test]
fn x_intellij_fixture_monade() {
    let content = include_str!("../../tests/fixtures/monade-stack-config.json");
    let value: Value = serde_json::from_str(content).expect("parse monade-stack-config.json");
    let mut migrated = value;
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    let schema: Schema = serde_json::from_value(migrated).expect("deserialize monade schema");

    // properties/nginx has x-intellij-enum-metadata
    let nginx = &schema.properties["nginx"]
        .as_schema()
        .expect("nginx is a schema");
    let meta = nginx
        .x_intellij
        .enum_metadata
        .as_ref()
        .expect("enum_metadata present");
    assert_eq!(meta.len(), 2);
    assert_eq!(
        meta["system"].description.as_deref(),
        Some("Use system nginx")
    );
    assert_eq!(
        meta["local"].description.as_deref(),
        Some("Use local nginx process")
    );
    assert!(!nginx.extra.contains_key("x-intellij-enum-metadata"));
}

#[test]
fn x_intellij_not_in_extra() {
    let json = json!({
        "type": "string",
        "x-intellij-html-description": "hello",
        "x-custom": "other"
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    assert!(schema.x_intellij.html_description.is_some());
    // x-intellij should NOT leak into extra
    assert!(!schema.extra.contains_key("x-intellij-html-description"));
    // but other x-* should still be in extra
    assert!(schema.extra.contains_key("x-custom"));
}

#[test]
fn x_lintel_deserialization() {
    let json = json!({
        "type": "object",
        "x-lintel": {
            "source": "https://example.com/schema.json",
            "sourceSha256": "abc123"
        }
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    let lintel = schema.x_lintel.unwrap();
    assert_eq!(
        lintel.source.as_deref(),
        Some("https://example.com/schema.json")
    );
    assert_eq!(lintel.source_sha256.as_deref(), Some("abc123"));
}

#[test]
fn navigate_pointer_empty() {
    let sv = SchemaValue::Schema(Box::new(Schema {
        type_: Some(TypeValue::Single(SimpleType::Object)),
        ..Default::default()
    }));
    let result = navigate_pointer(&sv, &sv, "").unwrap();
    assert!(result.as_schema().is_some());
}

#[test]
fn navigate_pointer_properties() {
    let name_schema = SchemaValue::Schema(Box::new(Schema {
        type_: Some(TypeValue::Single(SimpleType::String)),
        ..Default::default()
    }));
    let mut props = IndexMap::new();
    props.insert("name".into(), name_schema);
    let root = SchemaValue::Schema(Box::new(Schema {
        properties: props,
        ..Default::default()
    }));
    let result = navigate_pointer(&root, &root, "/properties/name").unwrap();
    let s = result.as_schema().unwrap();
    assert!(matches!(
        s.type_,
        Some(TypeValue::Single(SimpleType::String))
    ));
}

#[test]
fn navigate_pointer_resolves_ref() {
    let item_schema = SchemaValue::Schema(Box::new(Schema {
        type_: Some(TypeValue::Single(SimpleType::Object)),
        description: Some("An item".into()),
        ..Default::default()
    }));
    let ref_schema = SchemaValue::Schema(Box::new(Schema {
        ref_: Some("#/$defs/Item".into()),
        ..Default::default()
    }));
    let mut defs = BTreeMap::new();
    defs.insert("Item".into(), item_schema);
    let mut props = IndexMap::new();
    props.insert("item".into(), ref_schema);
    let root = SchemaValue::Schema(Box::new(Schema {
        properties: props,
        defs: Some(defs),
        ..Default::default()
    }));
    let result = navigate_pointer(&root, &root, "/properties/item").unwrap();
    let s = result.as_schema().unwrap();
    assert_eq!(s.description.as_deref(), Some("An item"));
}

#[test]
fn navigate_pointer_bad_segment_errors() {
    let sv = SchemaValue::Schema(Box::default());
    let err = navigate_pointer(&sv, &sv, "/nonexistent").unwrap_err();
    assert!(err.contains("nonexistent"));
}

#[test]
fn parse_cargo_fixture() {
    let content = include_str!("../../../jsonschema-migrate/tests/fixtures/cargo.json");
    let value: Value = serde_json::from_str(content).expect("parse cargo.json");
    let mut migrated = value;
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    let schema: Schema = serde_json::from_value(migrated).expect("deserialize cargo schema");
    assert!(schema.title.is_some() || schema.type_.is_some());
    // Verify x-taplo is parsed if present
    if schema.x_taplo.is_some() {
        // Just verify it parsed without error
    }
}

#[test]
fn numeric_fields_round_trip() {
    let json = json!({
        "type": "number",
        "minimum": 0,
        "maximum": 100.5,
        "exclusiveMinimum": -1,
        "exclusiveMaximum": 101,
        "multipleOf": 0.5
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    assert_eq!(schema.minimum.as_ref().unwrap().to_string(), "0");
    assert_eq!(schema.maximum.as_ref().unwrap().to_string(), "100.5");
    assert_eq!(schema.exclusive_minimum.as_ref().unwrap().to_string(), "-1");
    assert_eq!(
        schema.exclusive_maximum.as_ref().unwrap().to_string(),
        "101"
    );
    assert_eq!(schema.multiple_of.as_ref().unwrap().to_string(), "0.5");

    let back = serde_json::to_value(&schema).unwrap();
    assert_eq!(back["minimum"], 0);
    assert_eq!(back["maximum"], 100.5);
}

#[test]
fn schema_url_round_trip() {
    let json = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object"
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    assert_eq!(
        schema.schema.as_ref().unwrap().as_str(),
        "https://json-schema.org/draft/2020-12/schema"
    );

    let back = serde_json::to_value(&schema).unwrap();
    assert_eq!(
        back["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn vocabulary_round_trip() {
    let json = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$vocabulary": {
            "https://json-schema.org/draft/2020-12/vocab/core": true,
            "https://json-schema.org/draft/2020-12/vocab/applicator": true,
            "https://json-schema.org/draft/2020-12/vocab/validation": false
        }
    });
    let schema: Schema = serde_json::from_value(json).unwrap();
    let vocab = schema.vocabulary.as_ref().unwrap();
    assert_eq!(vocab.len(), 3);

    let core_url: Url = "https://json-schema.org/draft/2020-12/vocab/core"
        .parse()
        .unwrap();
    assert_eq!(vocab.get(&core_url), Some(&true));

    let validation_url: Url = "https://json-schema.org/draft/2020-12/vocab/validation"
        .parse()
        .unwrap();
    assert_eq!(vocab.get(&validation_url), Some(&false));

    let back = serde_json::to_value(&schema).unwrap();
    assert_eq!(
        back["$vocabulary"]["https://json-schema.org/draft/2020-12/vocab/core"],
        true
    );
}

/// Verify that the schemars-generated JSON Schema for `SchemaValue`
/// contains properties for every vocabulary keyword and extension field.
#[test]
fn generated_json_schema_contains_all_keywords() {
    let json_schema = crate::schema();

    // Find the Schema definition — schemars puts it in $defs (or definitions).
    // The root is SchemaValue (untagged enum of bool | Schema).
    let defs = json_schema
        .get("$defs")
        .or_else(|| json_schema.get("definitions"))
        .expect("schema should have $defs or definitions");
    let schema_def = defs.get("Schema").expect("$defs should contain Schema");
    let properties = schema_def
        .get("properties")
        .expect("Schema definition should have properties");

    // All standard JSON Schema keywords that must appear as explicit properties
    let expected_properties = [
        // Core vocabulary (9)
        "$schema",
        "$id",
        "$ref",
        "$anchor",
        "$dynamicRef",
        "$dynamicAnchor",
        "$comment",
        "$defs",
        "$vocabulary",
        // Applicator vocabulary (15)
        "prefixItems",
        "items",
        "contains",
        "additionalProperties",
        "properties",
        "patternProperties",
        "dependentSchemas",
        "propertyNames",
        "if",
        "then",
        "else",
        "allOf",
        "anyOf",
        "oneOf",
        "not",
        // Unevaluated vocabulary (2)
        "unevaluatedItems",
        "unevaluatedProperties",
        // Validation vocabulary (20)
        "type",
        "const",
        "enum",
        "multipleOf",
        "maximum",
        "exclusiveMaximum",
        "minimum",
        "exclusiveMinimum",
        "maxLength",
        "minLength",
        "pattern",
        "maxItems",
        "minItems",
        "uniqueItems",
        "maxContains",
        "minContains",
        "maxProperties",
        "minProperties",
        "required",
        "dependentRequired",
        // MetaData vocabulary (7)
        "title",
        "description",
        "default",
        "deprecated",
        "readOnly",
        "writeOnly",
        "examples",
        // FormatAnnotation vocabulary (1)
        "format",
        // Content vocabulary (3)
        "contentEncoding",
        "contentMediaType",
        "contentSchema",
        // Non-standard extensions
        "markdownDescription",
        "markdownEnumDescriptions",
        "x-lintel",
        "x-taplo",
        "x-taplo-info",
    ];

    let mut missing = Vec::new();
    for prop in &expected_properties {
        if properties.get(prop).is_none() {
            missing.push(*prop);
        }
    }
    assert!(
        missing.is_empty(),
        "generated JSON Schema missing properties: {missing:?}"
    );
}

/// Verify that the schemars-generated JSON Schema has doc comments
/// as descriptions on the Schema properties.
#[test]
fn generated_json_schema_has_field_descriptions() {
    let json_schema = crate::schema();

    let defs = json_schema
        .get("$defs")
        .or_else(|| json_schema.get("definitions"))
        .unwrap();
    let schema_def = defs.get("Schema").unwrap();
    let properties = schema_def.get("properties").unwrap();

    // Spot-check that rustdoc comments propagated into JSON Schema descriptions
    let checks = [
        ("$schema", "dialect"),
        ("$id", "identifier"),
        ("$ref", "reference"),
        ("title", "title"),
        ("description", "explanatory annotation"),
        ("type", "type"),
        ("properties", "property"),
        ("items", "items"),
        ("format", "format"),
        ("contentEncoding", "encoding"),
        ("markdownDescription", "Markdown"),
    ];

    for (prop, expected_substr) in &checks {
        let prop_schema = properties
            .get(prop)
            .unwrap_or_else(|| panic!("property {prop} missing"));
        let desc = prop_schema
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("property {prop} has no description"));
        assert!(
            desc.to_lowercase()
                .contains(&expected_substr.to_lowercase()),
            "property {prop} description doesn't contain '{expected_substr}': {desc}"
        );
    }
}
