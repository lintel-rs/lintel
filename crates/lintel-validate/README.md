# lintel-validate

[![Crates.io](https://img.shields.io/crates/v/lintel-validate.svg)](https://crates.io/crates/lintel-validate)
[![docs.rs](https://docs.rs/lintel-validate/badge.svg)](https://docs.rs/lintel-validate)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-validate.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Core validation engine for Lintel — validates JSON, YAML, TOML, JSON5, and JSONC files against JSON Schema.

## Features

- File discovery via glob patterns and `.gitignore`-aware walking
- Multi-format parsing (JSON, YAML, TOML, JSON5, JSONC, Markdown frontmatter)
- Schema resolution from inline annotations, config mappings, and catalog matching
- Schema fetching with disk-based caching
- Validation with rich diagnostics (source spans, labels)
- Validation result caching for incremental re-checks
- `Reporter` trait for pluggable output formatting

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
