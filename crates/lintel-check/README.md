# lintel-check

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/lintel-check.svg
[crates-url]: https://crates.io/crates/lintel-check
[docs-badge]: https://docs.rs/lintel-check/badge.svg
[docs-url]: https://docs.rs/lintel-check
[license-badge]: https://img.shields.io/crates/l/lintel-check.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Core validation engine for [Lintel](https://github.com/lintel-rs/lintel). Validates JSON, YAML, TOML, JSON5, and JSONC files against JSON Schema.

## Features

- **Multi-format parsing** — JSON, YAML, TOML, JSON5, JSONC with format-specific `$schema` extraction (inline properties, YAML modelines, TOML header comments)
- **SchemaStore catalog** — auto-matches files to schemas using [SchemaStore](https://www.schemastore.org/) `fileMatch` patterns
- **Schema caching** — disk-backed cache for remote schemas with configurable cache directory
- **Project configuration** — `lintel.toml` with exclude patterns, schema URI rewrites, `//`-relative paths, and per-file overrides
- **Rich diagnostics** — [miette](https://crates.io/crates/miette)-powered error reporting with source spans

## Usage

```rust
use lintel_check::validate::{self, ValidateArgs};
use lintel_check::retriever::UreqClient;

let args = ValidateArgs {
    globs: vec!["**/*.json".to_string()],
    exclude: vec![],
    cache_dir: None,
    force_schema_fetch: false,
    force_validation: false,
    no_catalog: false,
    format: None,
    config_dir: None,
};

let result = validate::run(&args, UreqClient).await?;
for error in result.errors {
    eprintln!("{}: {}", error.path(), error.message());
}
```

## Configuration (`lintel.toml`)

```toml
root = true
exclude = ["vendor/**", "node_modules/**"]

[rewrite]
"https://json.schemastore.org/" = "//schemas/"

[[override]]
files = ["**/vector.json"]
schemas = ["**/vector.json"]
validate_formats = false
```

- **`root`** — stop walking up the directory tree for parent configs
- **`exclude`** — glob patterns to skip during validation
- **`rewrite`** — URI prefix replacement rules (longest prefix wins)
- **`//` paths** — resolve relative to the directory containing `lintel.toml`
- **`[[override]]`** — per-file/per-schema settings (e.g. disable format validation)
