# lintel-config-schema-generator

[![Crates.io](https://img.shields.io/crates/v/lintel-config-schema-generator.svg)](https://crates.io/crates/lintel-config-schema-generator)
[![docs.rs](https://docs.rs/lintel-config-schema-generator/badge.svg)](https://docs.rs/lintel-config-schema-generator)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-config-schema-generator.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Generate JSON Schemas for Lintel configuration files (lintel.toml, lintel-catalog.toml)

Produces schemas for:

- **`lintel.json`** — schema for `lintel.toml` configuration files
- **`lintel-catalog.json`** — schema for `lintel-catalog.toml` catalog builder configuration files

## Usage

```sh
# Write schemas to the current directory
lintel-config-schema-generator

# Write schemas to a specific directory
lintel-config-schema-generator path/to/output/
```

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
