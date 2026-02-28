use serde_json::Value;

fn load_and_migrate() -> (Value, Value) {
    let source: Value =
        serde_json::from_str(include_str!("fixtures/sarif-1.0.0.json")).expect("valid JSON");
    let mut migrated = source.clone();
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    (source, migrated)
}

#[test]
fn detected_as_draft_04() {
    let (source, _) = load_and_migrate();
    assert_eq!(
        jsonschema_migrate::detect_draft(&source),
        Some(jsonschema_migrate::Draft::Draft04)
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
fn id_migrated_to_dollar_id() {
    let (source, migrated) = load_and_migrate();
    // Draft-04 uses `id`; after migration it should become `$id`.
    let source_id = source["id"].as_str().expect("source has id");
    assert!(migrated.get("id").is_none(), "legacy id should be removed");
    assert_eq!(
        migrated["$id"].as_str().expect("migrated has $id"),
        source_id,
    );
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
    assert!(
        json_str.contains("#/$defs/"),
        "refs should point to #/$defs/"
    );
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
            "$schema" | "definitions" | "id" => {}
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
            "$schema" | "$defs" | "$id" => {}
            _ => {
                assert!(
                    source_obj.contains_key(key),
                    "unexpected top-level key {key:?} appeared during migration"
                );
            }
        }
    }
}
