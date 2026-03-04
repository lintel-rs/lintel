use jsonschema_schema::Schema;
use serde_json::Value;

#[test]
fn x_intellij_fixture_huskyrc() {
    let content = include_str!("fixtures/huskyrc.json");
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
    let content = include_str!("fixtures/monade-stack-config.json");
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
fn parse_cargo_fixture() {
    let content = include_str!("fixtures/cargo.json");
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
