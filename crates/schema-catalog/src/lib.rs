#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A JSON Schema catalog following the `SchemaStore` catalog format.
/// See: <https://json.schemastore.org/schema-catalog.json>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub schemas: Vec<SchemaEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<CatalogGroup>,
}

/// A group of related schemas in the catalog.
///
/// Groups provide richer metadata for catalog consumers that support them.
/// Consumers that don't understand `groups` simply ignore the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogGroup {
    pub name: String,
    pub description: String,
    /// Schema names that belong to this group.
    pub schemas: Vec<String>,
}

/// A single schema entry in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub name: String,
    pub description: String,
    pub url: String,
    #[serde(default, rename = "fileMatch", skip_serializing_if = "Vec::is_empty")]
    pub file_match: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub versions: BTreeMap<String, String>,
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
            file_match: vec![],
            versions: BTreeMap::new(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(!json.contains("fileMatch"));
        assert!(!json.contains("versions"));
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
}
