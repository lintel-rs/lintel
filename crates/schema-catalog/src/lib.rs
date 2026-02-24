#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod compiled;
pub use compiled::{CompiledCatalog, SchemaMatch};

/// Schema catalog index that maps file patterns to JSON Schema URLs.
///
/// A catalog is a collection of schema entries used by editors and tools to
/// automatically associate files with the correct schema for validation and
/// completion. Follows the `SchemaStore` catalog format.
///
/// See: <https://json.schemastore.org/schema-catalog.json>
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "catalog.json")]
pub struct Catalog {
    /// The catalog format version. Currently always `1`.
    #[serde(default = "default_version")]
    pub version: u32,
    /// An optional human-readable title for the catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The list of schema entries in this catalog.
    pub schemas: Vec<SchemaEntry>,
    /// Optional grouping of related schemas for catalog consumers that
    /// support richer organization. Consumers that don't understand
    /// `groups` simply ignore this field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<CatalogGroup>,
}

/// A group of related schemas in the catalog.
///
/// Groups provide richer metadata for catalog consumers that support them.
/// Consumers that don't understand `groups` simply ignore the field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Schema Group")]
pub struct CatalogGroup {
    /// The display name for this group.
    pub name: String,
    /// A short description of the schemas in this group.
    pub description: String,
    /// Schema names that belong to this group.
    pub schemas: Vec<String>,
}

/// A single schema entry in the catalog.
///
/// Each entry maps a schema to its URL and the file patterns it applies to.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Schema Entry")]
pub struct SchemaEntry {
    /// The display name of the schema.
    #[schemars(example = example_schema_name())]
    pub name: String,
    /// A short description of what the schema validates.
    #[serde(default)]
    pub description: String,
    /// The URL where the schema can be fetched.
    #[schemars(example = example_schema_url())]
    pub url: String,
    /// An optional URL pointing to the upstream or canonical source of
    /// the schema (e.g. a GitHub raw URL).
    #[serde(default, rename = "sourceUrl", skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Glob patterns for files this schema should be applied to.
    ///
    /// Editors and tools use these patterns to automatically associate
    /// matching files with this schema.
    #[serde(default, rename = "fileMatch", skip_serializing_if = "Vec::is_empty")]
    #[schemars(title = "File Match")]
    #[schemars(example = example_file_match())]
    pub file_match: Vec<String>,
    /// Alternate versions of this schema, keyed by version identifier.
    /// Values are URLs to the versioned schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub versions: BTreeMap<String, String>,
}

fn default_version() -> u32 {
    1
}

fn example_schema_name() -> String {
    "My Config".to_owned()
}

fn example_schema_url() -> String {
    "https://example.com/schemas/my-config.json".to_owned()
}

fn example_file_match() -> Vec<String> {
    vec!["*.config.json".to_owned(), "**/.config.json".to_owned()]
}

/// Generate the JSON Schema for the [`Catalog`] type.
///
/// # Panics
///
/// Panics if the schema cannot be serialized to JSON (should never happen).
pub fn schema() -> Value {
    serde_json::to_value(schema_for!(Catalog)).expect("schema serialization cannot fail")
}

/// Parse a catalog from a JSON string.
///
/// # Errors
///
/// Returns an error if the string is not valid JSON or does not match the catalog schema.
pub fn parse_catalog(json: &str) -> Result<Catalog, serde_json::Error> {
    serde_json::from_str(json)
}

/// Parse a catalog from a `serde_json::Value`.
///
/// # Errors
///
/// Returns an error if the value does not match the expected catalog schema.
pub fn parse_catalog_value(value: serde_json::Value) -> Result<Catalog, serde_json::Error> {
    serde_json::from_value(value)
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn round_trip_catalog() {
        let catalog = Catalog {
            version: 1,
            title: None,
            schemas: vec![SchemaEntry {
                name: "Test Schema".into(),
                description: "A test schema".into(),
                url: "https://example.com/test.json".into(),
                source_url: None,
                file_match: vec!["*.test.json".into()],
                versions: BTreeMap::new(),
            }],
            groups: vec![],
        };
        let json = serde_json::to_string_pretty(&catalog).expect("serialize");
        let parsed: Catalog = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.schemas.len(), 1);
        assert_eq!(parsed.schemas[0].name, "Test Schema");
        assert_eq!(parsed.schemas[0].file_match, vec!["*.test.json"]);
    }

    #[test]
    fn parse_catalog_from_json_string() {
        let json = r#"{"version":1,"schemas":[{"name":"test","description":"desc","url":"https://example.com/s.json","fileMatch":["*.json"]}]}"#;
        let catalog = parse_catalog(json).expect("parse");
        assert_eq!(catalog.schemas.len(), 1);
        assert_eq!(catalog.schemas[0].name, "test");
        assert_eq!(catalog.schemas[0].file_match, vec!["*.json"]);
    }

    #[test]
    fn empty_file_match_omitted_in_serialization() {
        let entry = SchemaEntry {
            name: "No Match".into(),
            description: "desc".into(),
            url: "https://example.com/no.json".into(),
            source_url: None,
            file_match: vec![],
            versions: BTreeMap::new(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(!json.contains("fileMatch"));
        assert!(!json.contains("sourceUrl"));
        assert!(!json.contains("versions"));
    }

    #[test]
    fn source_url_serialized_as_camel_case() {
        let entry = SchemaEntry {
            name: "Test".into(),
            description: "desc".into(),
            url: "https://catalog.example.com/test.json".into(),
            source_url: Some("https://upstream.example.com/test.json".into()),
            file_match: vec![],
            versions: BTreeMap::new(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("\"sourceUrl\""));
        assert!(json.contains("https://upstream.example.com/test.json"));

        // Round-trip
        let parsed: SchemaEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed.source_url.as_deref(),
            Some("https://upstream.example.com/test.json")
        );
    }

    #[test]
    fn deserialize_with_versions() {
        let json = r#"{
            "version": 1,
            "schemas": [{
                "name": "test",
                "description": "desc",
                "url": "https://example.com/s.json",
                "versions": {"draft-07": "https://example.com/draft07.json"}
            }]
        }"#;
        let catalog = parse_catalog(json).expect("parse");
        assert_eq!(
            catalog.schemas[0].versions.get("draft-07"),
            Some(&"https://example.com/draft07.json".to_string())
        );
    }

    #[test]
    fn schema_has_camel_case_properties() {
        let schema = schema();
        let schema_str = serde_json::to_string(&schema).expect("serialize");
        assert!(
            schema_str.contains("fileMatch"),
            "schema should contain fileMatch"
        );
        assert!(
            schema_str.contains("sourceUrl"),
            "schema should contain sourceUrl"
        );
    }
}
