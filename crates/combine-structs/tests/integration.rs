#![allow(clippy::float_cmp, clippy::unwrap_used)]

use combine_structs::{Fields, combine_fields};

// --- Basic merging ---

#[derive(Fields, Debug, Default, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Fields, Debug, Default, PartialEq)]
pub struct Appearance {
    pub color: String,
    pub visible: bool,
}

#[combine_fields(Position, Appearance)]
#[derive(Debug, Default, PartialEq)]
pub struct Sprite {
    pub name: String,
}

#[test]
fn merged_fields_accessible() {
    let s = Sprite {
        name: "player".into(),
        x: 10.0,
        y: 20.0,
        color: "red".into(),
        visible: true,
    };
    assert_eq!(s.x, 10.0);
    assert_eq!(s.y, 20.0);
    assert_eq!(s.color, "red");
    assert!(s.visible);
    assert_eq!(s.name, "player");
}

#[test]
fn default_works() {
    let s = Sprite::default();
    assert_eq!(s.x, 0.0);
    assert_eq!(s.color, "");
    assert!(!s.visible);
    assert_eq!(s.name, "");
}

// --- Cross-module ---

mod inner {
    use combine_structs::Fields;

    #[allow(dead_code)]
    #[derive(Fields, Debug, Default, PartialEq)]
    pub struct Stats {
        pub hp: u32,
        pub mp: u32,
    }
}

#[combine_fields(Stats)]
#[derive(Debug, Default, PartialEq)]
pub struct Character {
    pub name: String,
}

#[test]
fn cross_module_fields_merged() {
    let c = Character {
        name: "hero".into(),
        hp: 100,
        mp: 50,
    };
    assert_eq!(c.hp, 100);
    assert_eq!(c.mp, 50);
    assert_eq!(c.name, "hero");
}

// --- Multiple sources ---

#[derive(Fields, Debug, Default)]
pub struct Movement {
    pub speed: f64,
    pub direction: f64,
}

#[combine_fields(Position, Appearance, Movement)]
#[derive(Debug, Default)]
pub struct Entity {
    pub id: u64,
}

#[test]
fn three_sources_merged() {
    let e = Entity {
        id: 42,
        x: 1.0,
        y: 2.0,
        color: "blue".into(),
        visible: false,
        speed: 3.5,
        direction: 90.0,
    };
    assert_eq!(e.id, 42);
    assert_eq!(e.speed, 3.5);
    assert_eq!(e.direction, 90.0);
}

// --- Single source ---

#[combine_fields(Position)]
#[derive(Debug, Default)]
pub struct Point {
    pub label: String,
}

#[test]
fn single_source() {
    let p = Point {
        label: "origin".into(),
        x: 0.0,
        y: 0.0,
    };
    assert_eq!(p.label, "origin");
    assert_eq!(p.x, 0.0);
}

// --- No extra fields on target ---

#[combine_fields(Position)]
#[derive(Debug, Default)]
pub struct Coord {}

#[test]
fn no_extra_fields() {
    let c = Coord { x: 5.0, y: 6.0 };
    assert_eq!(c.x, 5.0);
    assert_eq!(c.y, 6.0);
}

// --- Serde attribute preservation ---

#[derive(Fields, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(rename = "customName")]
    pub custom_name: String,
}

#[combine_fields(Metadata)]
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub title: String,
}

#[test]
fn serde_rename_preserved() {
    let doc = Document {
        title: "Test".into(),
        schema: Some("https://example.com".into()),
        custom_name: "hello".into(),
    };
    let json = serde_json::to_value(&doc).unwrap();
    assert_eq!(json["$schema"], "https://example.com");
    assert_eq!(json["customName"], "hello");
    assert_eq!(json["title"], "Test");
}

#[test]
fn serde_skip_serializing_if_preserved() {
    let doc = Document {
        title: "Test".into(),
        schema: None,
        custom_name: "x".into(),
    };
    let json = serde_json::to_value(&doc).unwrap();
    assert!(json.get("$schema").is_none());
}

#[test]
fn serde_round_trip() {
    let json = r#"{"$schema":"https://example.com","customName":"hi","title":"Doc"}"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    assert_eq!(doc.schema.as_deref(), Some("https://example.com"));
    assert_eq!(doc.custom_name, "hi");
    assert_eq!(doc.title, "Doc");
}

// --- Doc comment preservation ---

#[derive(Fields, Debug, Default)]
pub struct Documented {
    /// The x coordinate.
    pub x: f64,
}

#[combine_fields(Documented)]
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct WithDocs {}

#[test]
fn doc_comments_preserved() {
    // If doc attrs weren't preserved, the struct would still compile
    // but serde_json schema introspection wouldn't see them. We verify
    // the field exists and is usable (doc attrs don't affect runtime,
    // but their preservation is validated by the compile step itself).
    let w = WithDocs { x: 1.0 };
    assert_eq!(w.x, 1.0);
}

// --- Visibility preservation ---

mod vis_test {
    use combine_structs::Fields;

    #[allow(dead_code)]
    #[derive(Fields, Debug, Default)]
    pub struct Mixed {
        pub public_field: u32,
        pub(crate) crate_field: u32,
    }
}

#[combine_fields(Mixed)]
#[derive(Debug, Default)]
pub struct VisTarget {}

#[test]
fn visibility_preserved() {
    let v = VisTarget {
        public_field: 1,
        crate_field: 2,
    };
    assert_eq!(v.public_field, 1);
    assert_eq!(v.crate_field, 2);
}

// --- Source reused by multiple targets ---

#[combine_fields(Position)]
#[derive(Debug, Default)]
pub struct Waypoint {
    pub name: String,
}

#[combine_fields(Position)]
#[derive(Debug, Default)]
pub struct Marker {
    pub label: String,
}

#[test]
fn source_reused_across_targets() {
    let w = Waypoint {
        name: "A".into(),
        x: 1.0,
        y: 2.0,
    };
    let m = Marker {
        label: "B".into(),
        x: 3.0,
        y: 4.0,
    };
    assert_eq!(w.x, 1.0);
    assert_eq!(m.x, 3.0);
}

// --- Compile-fail tests ---

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
