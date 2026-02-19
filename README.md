<div align="center">

# Lintel

**Fast, multi-format JSON Schema linter for all your config files.**

[![CI][ci-badge]][ci-url]
[![crates.io][crates-badge]][crates-url]

[ci-badge]: https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/lintel-rs/lintel/actions/workflows/ci.yml
[crates-badge]: https://img.shields.io/crates/v/lintel?color=60a5fa
[crates-url]: https://crates.io/crates/lintel

</div>

**Lintel** validates JSON, YAML, TOML, JSON5, and JSONC files against [JSON Schema](https://json-schema.org/) in a single command. It auto-discovers schemas via [SchemaStore](https://www.schemastore.org/), inline `$schema` properties, and YAML modelines — zero config required.

**Fast.** Written in Rust with no async runtime, deterministic schema caching, and pre-compiled SchemaStore catalog matching. Warm runs are pure computation.

**Drop-in CI check.** Machine-parseable output, nonzero exit codes on failure, `.gitignore`-aware file walking. Add one line to your pipeline and catch config mistakes before they ship.

**Zero config.** Point it at your repo and go. Lintel figures out which schemas to use.

### Installation

```shell
cargo install lintel
```

### Usage

```shell
# validate with rich terminal output
lintel check

# validate with CI-friendly one-error-per-line output
lintel ci
```

## Documentation

Lintel auto-discovers schemas in priority order: YAML modeline (`# yaml-language-server: $schema=...`), inline `$schema` property, then SchemaStore catalog matching by filename. Files without a matching schema are silently skipped.

Lintel respects `.gitignore` — `node_modules`, `target/`, and build artifacts are skipped automatically.

Lintel supports project configuration via `lintel.toml` for exclude patterns and per-repo customization.

No Node.js required.

## License

Copyright Ian Macalinao. Licensed under the [Apache License, Version 2.0](LICENSE).
