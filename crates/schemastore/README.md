# schemastore

[![Crates.io](https://img.shields.io/crates/v/schemastore.svg)](https://crates.io/crates/schemastore)
[![docs.rs](https://docs.rs/schemastore/badge.svg)](https://docs.rs/schemastore)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/schemastore.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Parse and match files against the [SchemaStore](https://www.schemastore.org/) catalog.

`SchemaStore` is a community-maintained collection of JSON Schema definitions for common configuration files. This crate deserializes the catalog and matches file paths to their corresponding schemas using the `fileMatch` glob patterns.

## Usage

```rust
use schemastore::{parse_catalog, CompiledCatalog, CATALOG_URL};

// Fetch the catalog JSON yourself (using reqwest, ureq, etc.)
// let json = reqwest::blocking::get(CATALOG_URL)?.text()?;
// let value: serde_json::Value = serde_json::from_str(&json)?;
// let catalog = parse_catalog(value)?;

// Example with inline data:
let value = serde_json::json!({
    "schemas": [{
        "name": "TypeScript",
        "url": "https://json.schemastore.org/tsconfig.json",
        "fileMatch": ["tsconfig.json"]
    }]
});
let catalog = parse_catalog(value).unwrap();
let compiled = CompiledCatalog::compile(&catalog);

assert!(compiled.find_schema("tsconfig.json", "tsconfig.json").is_some());
```

## API

- `CATALOG_URL` — the well-known URL for the `SchemaStore` catalog JSON
- `Catalog` / `SchemaEntry` — serde types for the catalog
- `parse_catalog(Value)` — deserialize the catalog from a `serde_json::Value`
- `CompiledCatalog::compile(&Catalog)` — pre-compile all `fileMatch` globs
- `CompiledCatalog::find_schema(path, file_name)` — look up the schema URL for a file path

Bare filename patterns (e.g. `tsconfig.json`) are automatically expanded to also match nested paths (`**/tsconfig.json`). Negation patterns (starting with `!`) are skipped.

## Design

This crate is `#![no_std]` — it only depends on `alloc`, `serde`, `serde_json`, and `glob-match`. No HTTP client is included; callers fetch the catalog JSON themselves.

## License

Apache-2.0
