# jsonschema-explain

[![Crates.io](https://img.shields.io/crates/v/jsonschema-explain.svg)](https://crates.io/crates/jsonschema-explain)
[![docs.rs](https://docs.rs/jsonschema-explain/badge.svg)](https://docs.rs/jsonschema-explain)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/jsonschema-explain.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Render JSON Schema as human-readable terminal documentation

## Features

- Man-page-style terminal output from `serde_json::Value` schemas
- ANSI colors with bold headers, dimmed metadata, and highlighted types
- Syntax-highlighted code blocks in schema descriptions (via `markdown-to-ansi`)
- Renders properties, required fields, enums, defaults, `oneOf`/`anyOf`/`allOf` variants
- Caller-provided width for terminal-aware layout

## Usage

```rust
use jsonschema_explain::{explain, ExplainOptions};
use serde_json::Value;

let schema: Value = serde_json::from_str(r#"{"type": "object"}"#).unwrap();
let opts = ExplainOptions { color: true, syntax_highlight: true, width: 120, validation_errors: vec![] };
let output = explain(&schema, "my-config", &opts);
println!("{output}");
```

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## License

Apache-2.0
