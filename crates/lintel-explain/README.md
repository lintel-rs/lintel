# lintel-explain

[![Crates.io](https://img.shields.io/crates/v/lintel-explain.svg)](https://crates.io/crates/lintel-explain)
[![docs.rs](https://docs.rs/lintel-explain/badge.svg)](https://docs.rs/lintel-explain)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-explain.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Inspect JSON Schema documentation for specific properties and sub-schemas.

## Usage

```sh
lintel explain --schema <URL|FILE> [/pointer | $.jsonpath]
lintel explain --file <FILE> [/pointer | $.jsonpath]
```

When given a JSON Pointer (e.g. `/properties/name`), navigates to that sub-schema
and renders its documentation. When given a `JSONPath` expression (e.g. `$.name`),
converts it to the corresponding schema pointer automatically.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
