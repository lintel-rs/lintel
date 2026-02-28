use serde_json::Value;

fn load_and_migrate() -> (Value, Value) {
    let source: Value =
        serde_json::from_str(include_str!("fixtures/yamllint.json")).expect("valid JSON");
    let mut migrated = source.clone();
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    (source, migrated)
}

#[test]
fn detected_as_draft_2020_12() {
    let (source, _) = load_and_migrate();
    assert_eq!(
        jsonschema_migrate::detect_draft(&source),
        Some(jsonschema_migrate::Draft::Draft2020_12)
    );
}

#[test]
fn schema_stays_2020_12() {
    let (_, migrated) = load_and_migrate();
    assert_eq!(
        migrated["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn idempotent_migration() {
    let (source, migrated) = load_and_migrate();
    // For a 2020-12 schema, migration should be essentially a no-op
    // (only cleanup transforms like regex normalization could change things).
    // Check that the structure is preserved.
    let source_obj = source.as_object().expect("root is object");
    let migrated_obj = migrated.as_object().expect("root is object");

    // Same top-level keys
    for key in source_obj.keys() {
        assert!(
            migrated_obj.contains_key(key),
            "top-level key {key:?} was removed"
        );
    }
    for key in migrated_obj.keys() {
        assert!(
            source_obj.contains_key(key),
            "top-level key {key:?} appeared"
        );
    }
}

#[test]
fn dollar_id_preserved() {
    let (source, migrated) = load_and_migrate();
    assert_eq!(migrated["$id"], source["$id"]);
}

#[test]
fn defs_unchanged() {
    let (source, migrated) = load_and_migrate();
    // Already uses $defs — should not be touched
    assert!(migrated.get("definitions").is_none());
    let source_defs = source["$defs"].as_object().expect("$defs is object");
    let migrated_defs = migrated["$defs"].as_object().expect("$defs is object");
    assert_eq!(source_defs.len(), migrated_defs.len());
    for key in source_defs.keys() {
        assert!(
            migrated_defs.contains_key(key),
            "$defs entry {key:?} was lost"
        );
    }
}

#[test]
fn unevaluated_properties_preserved() {
    let (source, migrated) = load_and_migrate();
    // yamllint heavily uses unevaluatedProperties — must be preserved
    let source_str = serde_json::to_string(&source).expect("serialize");
    let migrated_str = serde_json::to_string(&migrated).expect("serialize");

    let source_count = source_str.matches("unevaluatedProperties").count();
    let migrated_count = migrated_str.matches("unevaluatedProperties").count();

    assert!(
        source_count > 0,
        "sanity: source should have unevaluatedProperties"
    );
    assert_eq!(
        source_count, migrated_count,
        "unevaluatedProperties count changed: source has {source_count}, migrated has {migrated_count}"
    );
}

#[test]
fn if_then_else_preserved() {
    let (source, migrated) = load_and_migrate();
    let source_str = serde_json::to_string(&source).expect("serialize");
    let migrated_str = serde_json::to_string(&migrated).expect("serialize");

    // Count if/then/else occurrences — should be identical
    for keyword in &["\"if\"", "\"then\"", "\"else\""] {
        let source_count = source_str.matches(keyword).count();
        let migrated_count = migrated_str.matches(keyword).count();
        assert_eq!(
            source_count, migrated_count,
            "{keyword} count changed: source has {source_count}, migrated has {migrated_count}"
        );
    }
}

#[test]
fn ref_paths_unchanged() {
    let (source, migrated) = load_and_migrate();
    // 2020-12 uses $defs already — no rewriting should happen
    let source_str = serde_json::to_string(&source).expect("serialize");
    let migrated_str = serde_json::to_string(&migrated).expect("serialize");

    assert!(
        !source_str.contains("#/definitions/"),
        "sanity: 2020-12 source should not have #/definitions/"
    );
    assert!(
        !migrated_str.contains("#/definitions/"),
        "migrated should not have #/definitions/"
    );
}

#[test]
fn all_properties_preserved() {
    let (source, migrated) = load_and_migrate();
    if let Some(source_props) = source.get("properties").and_then(|v| v.as_object()) {
        let migrated_props = migrated["properties"]
            .as_object()
            .expect("properties is object");
        for key in source_props.keys() {
            assert!(
                migrated_props.contains_key(key),
                "property {key:?} was lost"
            );
        }
    }
}
