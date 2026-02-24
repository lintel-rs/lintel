# lintel-check

[![Crates.io](https://img.shields.io/crates/v/lintel-check.svg)](https://crates.io/crates/lintel-check)
[![docs.rs](https://docs.rs/lintel-check/badge.svg)](https://docs.rs/lintel-check)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-check.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Check command for [Lintel](https://github.com/lintel-rs/lintel). Provides the `lintel check` subcommand which runs schema validation and additional checks.

The core validation engine lives in [`lintel-validate`](https://crates.io/crates/lintel-validate). This crate defines the bpaf CLI arguments for the `check` command and delegates to `lintel-validate` for validation.

## License

Apache-2.0
