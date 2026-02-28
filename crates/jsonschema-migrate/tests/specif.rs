use serde_json::Value;

fn load_and_migrate() -> (Value, Value) {
    let source: Value =
        serde_json::from_str(include_str!("fixtures/specif-1.1.json")).expect("valid JSON");
    let mut migrated = source.clone();
    jsonschema_migrate::migrate_to_2020_12(&mut migrated);
    (source, migrated)
}

/// Check that "id" property names are preserved throughout the schema tree.
fn check_id_properties(source: &Value, migrated: &Value, path: &str) {
    match (source, migrated) {
        (Value::Object(s_map), Value::Object(m_map)) => {
            if let Some(Value::Object(s_props)) = s_map.get("properties")
                && s_props.contains_key("id")
            {
                let m_props = m_map["properties"]
                    .as_object()
                    .unwrap_or_else(|| panic!("properties at {path}"));
                assert!(
                    m_props.contains_key("id"),
                    "\"id\" property was lost at {path}"
                );
            }
            for (k, v) in s_map {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                let migrated_key = if k == "definitions" {
                    "$defs"
                } else {
                    k.as_str()
                };
                if let Some(m_v) = m_map.get(migrated_key) {
                    check_id_properties(v, m_v, &child_path);
                }
            }
        }
        (Value::Array(s_arr), Value::Array(m_arr)) => {
            for (i, (s, m)) in s_arr.iter().zip(m_arr.iter()).enumerate() {
                check_id_properties(s, m, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

#[test]
fn detected_as_draft_2019_09() {
    let (source, _) = load_and_migrate();
    assert_eq!(
        jsonschema_migrate::detect_draft(&source),
        Some(jsonschema_migrate::Draft::Draft2019_09)
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
        "all 31 definitions should be migrated to $defs"
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
fn id_property_names_preserved() {
    let (source, migrated) = load_and_migrate();

    // specif has "id" as a property name in 16 places — these must NOT be
    // renamed to "$id" since they are data properties, not schema identifiers.
    check_id_properties(&source, &migrated, "");
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
                "property {key:?} was lost during migration"
            );
        }
    }
}

#[test]
fn migration_only_changes_expected_keys() {
    let (source, migrated) = load_and_migrate();
    let source_obj = source.as_object().expect("root is object");
    let migrated_obj = migrated.as_object().expect("root is object");

    for (key, _) in source_obj {
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
