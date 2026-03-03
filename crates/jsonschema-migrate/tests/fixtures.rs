//! Integration tests for schema migration using real-world fixtures.
//!
//! Each fixture is a real-world schema that previously failed `migrate()`
//! due to non-standard values. The tests verify that migration succeeds
//! and that cleanup transforms are correctly applied.

macro_rules! fixture {
    ($name:ident, $file:expr) => {
        mod $name {
            use serde_json::Value;

            fn load_and_migrate() -> (Value, jsonschema_migrate::Schema) {
                let source: Value = serde_json::from_str(include_str!(concat!("fixtures/", $file)))
                    .expect("valid JSON");
                let schema =
                    jsonschema_migrate::migrate(source.clone()).expect("migration should succeed");
                (source, schema)
            }

            #[test]
            fn migrate_succeeds() {
                load_and_migrate();
            }

            #[test]
            fn round_trips_through_serde() {
                let (_, schema) = load_and_migrate();
                let json = serde_json::to_value(&schema).expect("serialize");
                let _: jsonschema_migrate::Schema =
                    serde_json::from_value(json).expect("re-deserialize");
            }

            #[test]
            fn schema_is_2020_12() {
                let (_, schema) = load_and_migrate();
                assert_eq!(
                    schema.schema.as_deref(),
                    Some("https://json-schema.org/draft/2020-12/schema")
                );
            }
        }
    };
}

// cibuildwheel: $defs contains a bare string value ("description": "A Python version...")
fixture!(cibuildwheel, "cibuildwheel.json");

// flagd: $defs contains a bare string value ("$comment": "Merge the variants...")
fixture!(flagd, "flagd.json");

// ninjs (draft-03): has `"required": true` on individual properties
fixture!(ninjs, "ninjs.json");

#[test]
fn ninjs_required_migrated_to_parent() {
    let schema = jsonschema_migrate::migrate(
        serde_json::from_str(include_str!("fixtures/ninjs.json")).expect("valid JSON"),
    )
    .expect("migration should succeed");
    let required = schema
        .required
        .as_ref()
        .expect("should have required array");
    assert!(required.contains(&"uri".to_string()));
    // The child property should not have a boolean `required` in extras
    let props = schema.properties.as_ref().expect("should have properties");
    let uri = props
        .get("uri")
        .expect("should have uri property")
        .as_schema()
        .expect("uri should be a schema");
    assert!(!uri.extra.contains_key("required"));
}

// cloud-init: has `"enum": {"$ref": "..."}` instead of an array
fixture!(cloudinit, "cloudinit.json");

// uet-schema: has bare arrays in property positions (e.g. "Type": ["Custom", "Steam"])
fixture!(uet, "uet.json");

// wiremock: $defs contains a nested definitions map ("schemas": {"format": {...}, ...})
// where "format" collides with Schema.format field
fixture!(wiremock, "wiremock.json");

// utcm-monitor: has `"examples": "Present"` (bare string instead of array)
fixture!(utcm_monitor, "utcm-monitor.json");
