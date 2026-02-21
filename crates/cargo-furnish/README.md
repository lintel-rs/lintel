# cargo-furnish

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/cargo-furnish.svg
[crates-url]: https://crates.io/crates/cargo-furnish
[docs-badge]: https://docs.rs/cargo-furnish/badge.svg
[docs-url]: https://docs.rs/cargo-furnish
[license-badge]: https://img.shields.io/crates/l/cargo-furnish.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Furnish Rust crates with standardized Cargo.toml metadata, READMEs, and doc attributes. Reads workspace conventions and applies them to individual crates for consistent publishing.

## Usage

```shell
# Run on all workspace members
cargo furnish

# Target a specific crate by path or name
cargo furnish crates/schemastore
cargo furnish schemastore

# Overwrite existing README and doc comments
cargo furnish --force --description "My crate" schemastore

# Provide full README content
cargo furnish --force \
  --description "Short description" \
  --body "## Usage\n\nExample usage here." \
  --keywords "json-schema,validation" \
  --categories "development-tools" \
  my-crate
```

## License

Apache-2.0
