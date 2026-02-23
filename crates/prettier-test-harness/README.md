# prettier-test-harness

[![Crates.io](https://img.shields.io/crates/v/prettier-test-harness.svg)](https://crates.io/crates/prettier-test-harness)
[![docs.rs](https://docs.rs/prettier-test-harness/badge.svg)](https://docs.rs/prettier-test-harness)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/prettier-test-harness.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Test harness for prettier-compatible formatter crates

Test harness for prettier-compatible formatter crates.

Parses Jest snapshot files from the upstream prettier test suite into individual test cases with parser name, options, input, and expected output. Used by prettier-json5, prettier-jsonc, prettier-yaml, and prettier-markdown for conformance testing.

## License

Apache-2.0
