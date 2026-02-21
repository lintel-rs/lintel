# lintel-reporters

[![Crates.io](https://img.shields.io/crates/v/lintel-reporters.svg)](https://crates.io/crates/lintel-reporters)
[![docs.rs](https://docs.rs/lintel-reporters/badge.svg)](https://docs.rs/lintel-reporters)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-reporters.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Reporter implementations for [Lintel](https://github.com/lintel-rs/lintel). Provides pluggable output formatting for validation results.

## Reporters

- **Pretty** — rich terminal output with [miette](https://crates.io/crates/miette) diagnostics and source code snippets (default for `lintel check`)
- **Text** — one-line-per-error plain text output (default for `lintel ci`)
- **GitHub** — `::error` workflow commands with `file`, `line`, `col` for inline PR annotations

## Usage

```rust
use lintel_reporters::{ReporterKind, make_reporter, run, ValidateArgs};

let mut args = ValidateArgs { /* ... */ };
let mut reporter = make_reporter(ReporterKind::Pretty, false);
let had_errors = run(&mut args, client, reporter.as_mut()).await?;
```

## License

Apache-2.0
