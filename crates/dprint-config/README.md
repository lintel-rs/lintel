# dprint-config

[![Crates.io](https://img.shields.io/crates/v/dprint-config.svg)](https://crates.io/crates/dprint-config)
[![docs.rs](https://docs.rs/dprint-config/badge.svg)](https://docs.rs/dprint-config)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/dprint-config.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Strongly-typed configuration structs and JSON Schema for dprint formatter plugins (TypeScript, JSON, TOML, Markdown). Used by [Lintel](https://github.com/lintel-rs/lintel) to pass user formatting options through to dprint.

## Features

- Typed configuration structs for dprint global settings and per-plugin options
- Covers TypeScript, JSON, TOML, and Markdown plugins
- Unknown plugins fall through to a generic `PluginConfig` with arbitrary settings
- JSON Schema generation via `schemars`
- `#[no_std]` compatible

## Usage

```rust
use dprint_config::DprintConfig;

let json = r#"{ "lineWidth": 100, "json": { "indentWidth": 4 } }"#;
let config: DprintConfig = serde_json::from_str(json).unwrap();
assert_eq!(config.line_width, Some(100));
```

## License

Apache-2.0
