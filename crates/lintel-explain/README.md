# lintel-explain

[![Crates.io](https://img.shields.io/crates/v/lintel-explain.svg)](https://crates.io/crates/lintel-explain)
[![docs.rs](https://docs.rs/lintel-explain/badge.svg)](https://docs.rs/lintel-explain)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-explain.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Inspect JSON Schema documentation for specific properties and sub-schemas.

## Usage

```sh
lintel explain <FILE|URL> [/pointer | $.jsonpath]
lintel explain --file <FILE|URL> [/pointer | $.jsonpath]
lintel explain --path <FILE|URL> [/pointer | $.jsonpath]
lintel explain --schema <URL|FILE> [/pointer | $.jsonpath]
lintel explain --schema <URL|FILE> --file <FILE|URL> [/pointer | $.jsonpath]
```

The simplest form, `lintel explain <FILE>`, resolves the schema from the given
file path (equivalent to `--path`). Both `--file` and `--path` also accept URLs.
`--schema` can be combined with `--file` or `--path` to override the schema while
still validating the data file.

When given a JSON Pointer (e.g. `/properties/name`), navigates to that sub-schema
and renders its documentation. When given a `JSONPath` expression (e.g. `$.name`),
converts it to the corresponding schema pointer automatically.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
