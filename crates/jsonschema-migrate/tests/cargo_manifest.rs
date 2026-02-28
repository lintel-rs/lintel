use serde_json::Value;

/// Collect all keys starting with `x-` at every level, returning `(json_path, value)` pairs.
fn collect_extension_keys<'a>(value: &'a Value, path: &str, out: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let full = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                if k.starts_with("x-") {
                    out.push((full.clone(), v));
                }
                collect_extension_keys(v, &full, out);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                collect_extension_keys(v, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

fn load_and_migrate() -> (Value, Value) {
    let source: Value =
        serde_json::from_str(include_str!("fixtures/cargo.json")).expect("valid JSON");
    let mut migrated = source.clone();
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    (source, migrated)
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
    // Source uses $id (draft-07 style), should be kept as-is.
    assert_eq!(migrated["$id"], source["$id"]);
}

#[test]
fn definitions_migrated_to_defs() {
    let (_, migrated) = load_and_migrate();
    assert!(migrated.get("definitions").is_none());
    assert!(migrated.get("$defs").is_some());
}

#[test]
fn ref_paths_rewritten() {
    let (_, migrated) = load_and_migrate();
    // A $ref that pointed at #/definitions/... should now point at #/$defs/...
    let deps_ref = migrated["properties"]["dependencies"]["additionalProperties"]["$ref"]
        .as_str()
        .expect("json object");
    assert!(
        deps_ref.starts_with("#/$defs/"),
        "expected $ref to use $defs, got: {deps_ref}"
    );
}

#[test]
fn all_properties_preserved() {
    let (source, migrated) = load_and_migrate();
    let source_props = source["properties"].as_object().expect("json object");
    let migrated_props = migrated["properties"].as_object().expect("json object");

    for key in source_props.keys() {
        assert!(
            migrated_props.contains_key(key),
            "property {key:?} was lost during migration"
        );
    }

    // No spurious keys should be added inside properties
    for key in migrated_props.keys() {
        assert!(
            source_props.contains_key(key),
            "unexpected key {key:?} appeared in properties after migration"
        );
    }
}

#[test]
fn dependencies_property_preserved() {
    let (source, migrated) = load_and_migrate();
    let source_deps = &source["properties"]["dependencies"];
    let migrated_deps = &migrated["properties"]["dependencies"];

    // Must still exist
    assert!(
        !migrated_deps.is_null(),
        "dependencies property was removed during migration"
    );

    // Type should be the same
    assert_eq!(source_deps["type"], migrated_deps["type"]);
    assert_eq!(source_deps["description"], migrated_deps["description"]);
}

#[test]
fn no_dependent_schemas_in_properties() {
    let (_, migrated) = load_and_migrate();
    let props = migrated["properties"].as_object().expect("json object");
    assert!(
        !props.contains_key("dependentSchemas"),
        "dependencies was incorrectly migrated to dependentSchemas inside properties"
    );
    assert!(
        !props.contains_key("dependentRequired"),
        "dependencies was incorrectly migrated to dependentRequired inside properties"
    );
}

#[test]
fn nested_dependencies_properties_preserved() {
    let (_, migrated) = load_and_migrate();

    // Platform also has a dependencies property
    let platform_props = migrated["$defs"]["Platform"]["properties"]
        .as_object()
        .expect("json object");
    assert!(
        platform_props.contains_key("dependencies"),
        "Platform.properties.dependencies was lost"
    );

    // Workspace also has a dependencies property
    let workspace_props = migrated["$defs"]["Workspace"]["properties"]
        .as_object()
        .expect("json object");
    assert!(
        workspace_props.contains_key("dependencies"),
        "Workspace.properties.dependencies was lost"
    );
}

#[test]
fn top_level_extension_properties_preserved() {
    let (source, migrated) = load_and_migrate();

    // All non-schema top-level keys must survive, especially x-* extensions.
    let expected_keys = [
        "$id",
        "description",
        "title",
        "type",
        "additionalProperties",
    ];
    for key in expected_keys {
        assert_eq!(
            source[key], migrated[key],
            "top-level key {key:?} was changed during migration"
        );
    }

    let extension_keys = [
        "x-tombi-toml-version",
        "x-tombi-table-keys-order",
        "x-taplo-info",
    ];
    for key in extension_keys {
        assert_eq!(
            source[key], migrated[key],
            "extension property {key:?} was changed during migration"
        );
    }
}

#[test]
fn nested_extension_properties_preserved() {
    let (source, migrated) = load_and_migrate();

    // x-taplo on a definition (moves from definitions → $defs)
    assert_eq!(
        source["definitions"]["Build"]["x-taplo"],
        migrated["$defs"]["Build"]["x-taplo"],
    );

    // x-tombi-table-keys-order on a definition
    assert_eq!(
        source["definitions"]["DetailedDependency"]["x-tombi-table-keys-order"],
        migrated["$defs"]["DetailedDependency"]["x-tombi-table-keys-order"],
    );

    // x-tombi-additional-key-label inside a property definition
    assert_eq!(
        source["definitions"]["Lints"]["properties"]["rust"]["x-tombi-additional-key-label"],
        migrated["$defs"]["Lints"]["properties"]["rust"]["x-tombi-additional-key-label"],
    );
}

#[test]
fn all_extension_keys_preserved() {
    let (source, migrated) = load_and_migrate();

    // Collect every x-* key from source
    let mut source_exts = Vec::new();
    collect_extension_keys(&source, "", &mut source_exts);

    // Collect every x-* key from migrated
    let mut migrated_exts = Vec::new();
    collect_extension_keys(&migrated, "", &mut migrated_exts);

    assert!(
        !source_exts.is_empty(),
        "sanity: source should have extension keys"
    );

    // The migrated schema must have at least as many extension keys.
    // The paths will differ (definitions → $defs) but the count must match.
    assert_eq!(
        source_exts.len(),
        migrated_exts.len(),
        "extension key count changed: source has {}, migrated has {}",
        source_exts.len(),
        migrated_exts.len(),
    );

    // Verify each source value appears in the migrated output at the
    // corresponding path (adjusting definitions → $defs).
    for (path, source_val) in &source_exts {
        let migrated_path = path.replace("definitions.", "$defs.");
        let migrated_val = migrated_exts
            .iter()
            .find(|(p, _)| *p == migrated_path)
            .map(|(_, v)| *v);
        assert_eq!(
            migrated_val,
            Some(*source_val),
            "extension key {path:?} (migrated path: {migrated_path:?}) has wrong value"
        );
    }
}

#[test]
fn migration_only_changes_expected_keys() {
    let (source, migrated) = load_and_migrate();

    let source_obj = source.as_object().expect("json object");
    let migrated_obj = migrated.as_object().expect("json object");

    // The only top-level key changes should be:
    // - $schema: updated to 2020-12
    // - definitions: removed (replaced by $defs)
    // - $defs: added (from definitions)
    // Everything else must be identical.
    for (key, _source_val) in source_obj {
        match key.as_str() {
            "$schema" | "definitions" => {}
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
            "$schema" | "$defs" => {}
            _ => {
                assert!(
                    source_obj.contains_key(key),
                    "unexpected top-level key {key:?} appeared during migration"
                );
            }
        }
    }
}
