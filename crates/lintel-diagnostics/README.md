# lintel-diagnostics

[![Crates.io](https://img.shields.io/crates/v/lintel-diagnostics.svg)](https://crates.io/crates/lintel-diagnostics)
[![docs.rs](https://docs.rs/lintel-diagnostics/badge.svg)](https://docs.rs/lintel-diagnostics)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-diagnostics.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Shared diagnostic types for Lintel — errors, results, and the Reporter trait

Shared diagnostic types for [Lintel](https://github.com/lintel-rs/lintel) — errors, results, and the `Reporter` trait.

## Features

- `LintelDiagnostic` — unified error type for parse, validation, I/O, schema, and format errors
- `CheckResult` / `CheckedFile` — structured result types for check runs
- `Reporter` trait — interface for formatting and outputting check results

## License

Apache-2.0
