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

## License

Apache-2.0
