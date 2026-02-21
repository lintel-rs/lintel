# lintel-annotate

[![Crates.io](https://img.shields.io/crates/v/lintel-annotate.svg)](https://crates.io/crates/lintel-annotate)
[![docs.rs](https://docs.rs/lintel-annotate/badge.svg)](https://docs.rs/lintel-annotate)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-annotate.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Annotation-based linting for JSON and YAML files using JSON Schema

## Features

- Runs [Lintel](https://github.com/lintel-rs/lintel) validation on the specified files
- Collects errors with file path, line, and column information
- Outputs annotations in a format suitable for CI systems
- Supports glob patterns and exclude filters

## License

Apache-2.0
