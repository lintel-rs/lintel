# schemastore

[![Crates.io](https://img.shields.io/crates/v/schemastore.svg)](https://crates.io/crates/schemastore)
[![docs.rs](https://docs.rs/schemastore/badge.svg)](https://docs.rs/schemastore)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/schemastore.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Constants and re-exports for working with the [SchemaStore](https://www.schemastore.org/) catalog.

This is a thin convenience crate that provides:

- `CATALOG_URL` — the well-known URL for the `SchemaStore` catalog JSON
- `pub use schema_catalog` — re-exports the `schema-catalog` crate for catalog types, parsing, and compiled matching

## Usage

```rust
use schemastore::CATALOG_URL;
use schemastore::schema_catalog::{self, CompiledCatalog};

// Fetch the catalog JSON yourself (using reqwest, ureq, etc.)
// let json = reqwest::blocking::get(CATALOG_URL)?.text()?;
// let catalog = schema_catalog::parse_catalog(&json)?;
// let compiled = CompiledCatalog::compile(&catalog);

assert!(CATALOG_URL.starts_with("https://"));
```

## License

Apache-2.0
