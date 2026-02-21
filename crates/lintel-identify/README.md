# lintel-identify

[![Crates.io](https://img.shields.io/crates/v/lintel-identify.svg)](https://crates.io/crates/lintel-identify)
[![docs.rs](https://docs.rs/lintel-identify/badge.svg)](https://docs.rs/lintel-identify)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-identify.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Schema identification for JSON and YAML files using JSON Schema

## Features

Identifies which JSON Schema applies to a given file using multiple discovery strategies:

1. YAML modeline (`# yaml-language-server: $schema=...`)
2. Inline `$schema` property
3. `lintel.toml` schema mappings
4. [SchemaStore](https://www.schemastore.org/) catalog matching by filename

Optionally renders schema documentation in the terminal with `--explain`.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
