# lintel-config

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/lintel-config.svg
[crates-url]: https://crates.io/crates/lintel-config
[docs-badge]: https://docs.rs/lintel-config/badge.svg
[docs-url]: https://docs.rs/lintel-config
[license-badge]: https://img.shields.io/crates/l/lintel-config.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Configuration types and loader for [Lintel](https://github.com/lintel-rs/lintel). Defines the `lintel.toml` schema, parses config files, and provides utilities for schema URI rewriting and path resolution.

## Features

- **Config types** — `Config` and `Override` structs with serde deserialization and JSON Schema generation via [schemars](https://crates.io/crates/schemars)
- **Hierarchical loading** — walks up the directory tree merging `lintel.toml` files until `root = true`
- **URI rewriting** — prefix-based rewrite rules with longest-prefix-wins semantics
- **`//` path resolution** — resolve `//`-prefixed paths relative to the config directory
- **Schema generation** — generates the JSON Schema for `lintel.toml` (used at build time by `lintel-check` and as a standalone binary)

## Usage

```rust
use lintel_config::{Config, find_and_load, apply_rewrites, resolve_double_slash};

// Load config by walking up from a directory
let config = find_and_load(std::path::Path::new("."))?
    .unwrap_or_default();

// Check for custom schema mappings
if let Some(url) = config.find_schema_mapping("src/config.json", "config.json") {
    println!("Schema: {url}");
}

// Apply rewrite rules and resolve // paths
let uri = apply_rewrites("http://localhost:8000/schema.json", &config.rewrite);
let uri = resolve_double_slash(&uri, std::path::Path::new("/project"));
```

### Standalone binary

```sh
cargo run -p lintel-config
```

Prints the JSON Schema for `lintel.toml` to stdout.
