# lintel

[![Crates.io](https://img.shields.io/crates/v/lintel.svg)](https://crates.io/crates/lintel)
[![docs.rs](https://docs.rs/lintel/badge.svg)](https://docs.rs/lintel)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Fast, multi-format JSON Schema linter CLI. Validates JSON, YAML, TOML, JSON5, and JSONC files against [JSON Schema](https://json-schema.org/) in a single command.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## Installation

```shell
cargo install lintel
```

Or with npm:

```shell
npx lintel check
```

Or with Nix:

```shell
nix run github:lintel-rs/lintel
```

## Usage

```shell
# validate with rich terminal output
lintel check

# validate with CI-friendly one-error-per-line output
lintel ci

# generate a lintel.toml with auto-detected schemas
lintel init

# convert between formats
lintel convert config.yaml --to toml
```

## Schema Discovery

Lintel auto-discovers schemas in priority order:

1. **YAML modeline** — `# yaml-language-server: $schema=...`
2. **Inline `$schema` property** — in the document itself
3. **`lintel.toml` mappings** — custom `[schemas]` table entries
4. **Lintel catalog** — schemas for tools not in `SchemaStore`
5. **`SchemaStore` catalog** — matching by filename

## License

Apache-2.0
