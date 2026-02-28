use serde_json::Value;

fn load_and_migrate() -> (Value, Value) {
    let source: Value =
        serde_json::from_str(include_str!("fixtures/chrome-manifest.json")).expect("valid JSON");
    let mut migrated = source.clone();
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    (source, migrated)
}

#[test]
fn detected_as_draft_07() {
    let (source, _) = load_and_migrate();
    assert_eq!(
        jsonschema_migrate::detect_draft(&source),
        Some(jsonschema_migrate::Draft::Draft07)
    );
}

#[test]
fn schema_upgraded_to_2020_12() {
    let (_, migrated) = load_and_migrate();
    assert_eq!(
        migrated["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn dollar_id_preserved() {
    let (source, migrated) = load_and_migrate();
    assert_eq!(migrated["$id"], source["$id"]);
}

#[test]
fn definitions_migrated_to_defs() {
    let (source, migrated) = load_and_migrate();
    assert!(migrated.get("definitions").is_none());
    let defs = migrated["$defs"].as_object().expect("$defs is object");
    let source_defs = source["definitions"]
        .as_object()
        .expect("definitions is object");
    assert_eq!(
        defs.len(),
        source_defs.len(),
        "all definitions should be migrated to $defs"
    );
}

#[test]
fn ref_paths_rewritten() {
    let (_, migrated) = load_and_migrate();
    let json_str = serde_json::to_string(&migrated).expect("serialize");
    assert!(
        !json_str.contains("#/definitions/"),
        "no $ref should still point to #/definitions/"
    );
}

#[test]
fn schema_dependencies_migrated() {
    let (source, migrated) = load_and_migrate();

    // chrome-manifest has `dependencies` as a schema keyword at the root
    // and inside some definitions. These should be migrated to
    // dependentSchemas/dependentRequired.
    if source.get("dependencies").is_some() {
        assert!(
            migrated.get("dependencies").is_none(),
            "root-level schema dependencies should be migrated"
        );
        // Should have been split into dependentSchemas and/or dependentRequired
        assert!(
            migrated.get("dependentSchemas").is_some()
                || migrated.get("dependentRequired").is_some(),
            "dependencies should become dependentSchemas or dependentRequired"
        );
    }
}

#[test]
fn if_then_else_preserved() {
    let (source, migrated) = load_and_migrate();
    let json_str = serde_json::to_string(&migrated).expect("serialize");

    // if/then/else is valid in draft-07 and 2020-12, should be preserved
    if serde_json::to_string(&source)
        .expect("serialize")
        .contains("\"if\"")
    {
        assert!(
            json_str.contains("\"if\""),
            "if/then/else should be preserved during migration"
        );
    }
}

#[test]
fn all_definitions_preserved() {
    let (source, migrated) = load_and_migrate();
    let source_defs = source["definitions"]
        .as_object()
        .expect("definitions is object");
    let migrated_defs = migrated["$defs"].as_object().expect("$defs is object");

    for key in source_defs.keys() {
        assert!(
            migrated_defs.contains_key(key),
            "definition {key:?} was lost during migration"
        );
    }
}

#[test]
fn all_properties_preserved() {
    let (source, migrated) = load_and_migrate();
    let source_props = source["properties"]
        .as_object()
        .expect("properties is object");
    let migrated_props = migrated["properties"]
        .as_object()
        .expect("properties is object");

    for key in source_props.keys() {
        assert!(
            migrated_props.contains_key(key),
            "property {key:?} was lost during migration"
        );
    }
    for key in migrated_props.keys() {
        assert!(
            source_props.contains_key(key),
            "unexpected property {key:?} appeared after migration"
        );
    }
}

#[test]
fn migration_only_changes_expected_keys() {
    let (source, migrated) = load_and_migrate();
    let source_obj = source.as_object().expect("root is object");
    let migrated_obj = migrated.as_object().expect("root is object");

    for (key, _) in source_obj {
        match key.as_str() {
            "$schema" | "definitions" | "dependencies" => {}
            _ => {
                assert!(
                    migrated_obj.contains_key(key),
                    "top-level key {key:?} was removed during migration"
                );
            }
        }
    }
    for key in migrated_obj.keys() {
        match key.as_str() {
            "$schema" | "$defs" | "dependentSchemas" | "dependentRequired" => {}
            _ => {
                assert!(
                    source_obj.contains_key(key),
                    "unexpected top-level key {key:?} appeared during migration"
                );
            }
        }
    }
}
