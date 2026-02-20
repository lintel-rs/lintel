# schemastore

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/schemastore.svg
[crates-url]: https://crates.io/crates/schemastore
[docs-badge]: https://docs.rs/schemastore/badge.svg
[docs-url]: https://docs.rs/schemastore
[license-badge]: https://img.shields.io/crates/l/schemastore.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Parse and match files against the [SchemaStore](https://www.schemastore.org/) catalog.

SchemaStore is a community-maintained collection of JSON Schema definitions for common configuration files. This crate deserializes the catalog and matches file paths to their corresponding schemas using the `fileMatch` glob patterns.

## Usage

```rust
use schemastore::{parse_catalog, CompiledCatalog, CATALOG_URL};

// Fetch the catalog JSON yourself (using ureq, reqwest, etc.)
let json: String = ureq::get(CATALOG_URL).call()?.body_mut().read_to_string()?;
let value: serde_json::Value = serde_json::from_str(&json)?;

let catalog = parse_catalog(value)?;
let compiled = CompiledCatalog::compile(&catalog);

compiled.find_schema("tsconfig.json", "tsconfig.json");
// => Some("https://json.schemastore.org/tsconfig.json")

compiled.find_schema(".github/workflows/ci.yml", "ci.yml");
// => Some("https://www.schemastore.org/github-workflow.json")
```

## API

- `CATALOG_URL` — the well-known URL for the SchemaStore catalog JSON
- `Catalog` / `SchemaEntry` — serde types for the catalog
- `parse_catalog(Value)` — deserialize the catalog from a `serde_json::Value`
- `CompiledCatalog::compile(&Catalog)` — pre-compile all `fileMatch` globs
- `CompiledCatalog::find_schema(path, file_name)` — look up the schema URL for a file path

Bare filename patterns (e.g. `tsconfig.json`) are automatically expanded to also match nested paths (`**/tsconfig.json`). Negation patterns (starting with `!`) are skipped.

## Design

This crate is `#![no_std]` — it only depends on `alloc`, `serde`, `serde_json`, and `glob-match`. No HTTP client is included; callers fetch the catalog JSON themselves.
