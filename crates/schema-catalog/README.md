# schema-catalog

[![Crates.io](https://img.shields.io/crates/v/schema-catalog.svg)](https://crates.io/crates/schema-catalog)
[![docs.rs](https://docs.rs/schema-catalog/badge.svg)](https://docs.rs/schema-catalog)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/schema-catalog.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Types for the JSON Schema catalog format (`schema-catalog.json`), compatible with [SchemaStore](https://www.schemastore.org/).

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## Usage

```rust
use schema_catalog::{Catalog, parse_catalog};

let json = r#"{"version":1,"schemas":[{"name":"Example","description":"An example schema","url":"https://example.com/schema.json","fileMatch":["*.example.json"]}]}"#;
let catalog: Catalog = parse_catalog(json).unwrap();
assert_eq!(catalog.schemas[0].name, "Example");
```

## API

- `Catalog` / `SchemaEntry` — serde types for the catalog format
- `parse_catalog(json)` — deserialize a catalog from a JSON string
- `parse_catalog_value(value)` — deserialize from a `serde_json::Value`
- `schema()` — generate the JSON Schema for the `Catalog` type
- `CompiledCatalog::compile(&Catalog)` — pre-compile all `fileMatch` globs into a fast matcher
- `CompiledCatalog::find_schema(path, file_name)` — look up the schema URL for a file path
- `CompiledCatalog::find_schema_detailed(path, file_name)` — look up with full match details

Bare filename patterns (e.g. `tsconfig.json`) are automatically expanded to also match nested paths (`**/tsconfig.json`). Negation patterns (starting with `!`) are skipped.

## Design

`CompiledCatalog` uses a single `GlobMap` from the `glob-set` crate. The `GlobMap`'s `MatchEngine` automatically dispatches each pattern to the fastest strategy (literal hash, extension hash, prefix/suffix tries, Aho-Corasick pre-filter).

This crate is `#![no_std]` — it only depends on `alloc`, `serde`, `serde_json`, `schemars`, and `glob-set`.

## License

Apache-2.0
