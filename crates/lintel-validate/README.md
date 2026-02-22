# lintel-validate

[![Crates.io](https://img.shields.io/crates/v/lintel-validate.svg)](https://crates.io/crates/lintel-validate)
[![docs.rs](https://docs.rs/lintel-validate/badge.svg)](https://docs.rs/lintel-validate)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-validate.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Schema validation command for Lintel — validates JSON, YAML, TOML, JSON5, and JSONC against JSON Schema

## Features

Runs schema validation against files, combining the core validation engine (`lintel-check`) with reporter output (`lintel-reporters`).

Provides the `lintel validate` subcommand and its bpaf CLI argument struct.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
