# lintel-annotate

[![Crates.io](https://img.shields.io/crates/v/lintel-annotate.svg)](https://crates.io/crates/lintel-annotate)
[![docs.rs](https://docs.rs/lintel-annotate/badge.svg)](https://docs.rs/lintel-annotate)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-annotate.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Add schema annotations (`$schema`, YAML modelines, TOML schema comments) to JSON, YAML, and TOML files.

## Features

- Automatically resolves schemas via catalog matching and `lintel.toml` mappings
- Adds `$schema` to JSON/JSON5/JSONC, YAML modelines, and TOML `:schema` comments
- Updates existing annotations with `--update`
- Supports glob patterns and exclude filters

## License

Apache-2.0
