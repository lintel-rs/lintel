#![doc = include_str!("../README.md")]

/// Re-export canonical catalog types from `schema-catalog`.
pub use schema_catalog;

/// The URL of the `SchemaStore` catalog.
pub const CATALOG_URL: &str = "https://www.schemastore.org/api/json/catalog.json";
