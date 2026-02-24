# jsonschema-migrate

[![Crates.io](https://img.shields.io/crates/v/jsonschema-migrate.svg)](https://crates.io/crates/jsonschema-migrate)
[![docs.rs](https://docs.rs/jsonschema-migrate/badge.svg)](https://docs.rs/jsonschema-migrate)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/jsonschema-migrate.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Migrate JSON Schema documents to draft 2020-12

## Features

Transforms JSON Schema documents from any draft (04, 06, 07, 2019-09) to draft 2020-12 in-place. All transforms are idempotent — running on a schema that is already 2020-12 is a no-op.

### Keyword migrations

| Old (draft 04–2019-09)                  | New (2020-12)                                    |
| --------------------------------------- | ------------------------------------------------ |
| `definitions`                           | `$defs`                                          |
| `id`                                    | `$id` (draft-04 only, on schema-like objects)    |
| `$ref: "#/definitions/..."`             | `$ref: "#/$defs/..."`                            |
| `items: [...]  ` (array form)           | `prefixItems` + `items` (from `additionalItems`) |
| `exclusiveMinimum: true` + `minimum: N` | `exclusiveMinimum: N`                            |
| `exclusiveMaximum: true` + `maximum: N` | `exclusiveMaximum: N`                            |
| `dependencies` (mixed)                  | `dependentSchemas` + `dependentRequired`         |

### Regex normalization

JSON Schema uses ECMA 262 regular expressions. The `jsonschema` crate validates patterns using `fancy_regex`, which delegates parsing to `regex_syntax`. `normalize_ecma_regex` fixes ECMA 262 constructs that `regex_syntax` rejects:

- Escapes bare `{` and `}` that aren't valid quantifiers
- Expands `\d` → `0-9` inside character classes (avoids invalid range endpoints)
- Preserves Unicode property escapes (`\p{L}`, `\P{N}`, `\u{FFFF}`)

## Usage

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut schema: serde_json::Value = serde_json::from_str(r##"{
  "$schema": "http://json-schema.org/draft-04/schema#",
  "definitions": { "name": { "type": "string" } },
  "$ref": "#/definitions/name"
}"##)?;

jsonschema_migrate::migrate_to_2020_12(&mut schema);
// schema now uses $defs, $ref points to #/$defs/name, $schema is 2020-12

// Normalize a regex pattern for Rust compatibility
let pattern = jsonschema_migrate::normalize_ecma_regex(r"^[A-Z]{2,4}$");
assert_eq!(pattern, r"^[A-Z]{2,4}$"); // valid quantifier, unchanged

let pattern = jsonschema_migrate::normalize_ecma_regex(r"^foo{bar}$");
assert_eq!(pattern, r"^foo\{bar\}$"); // bare braces escaped
# Ok(())
# }
```

Part of [Lintel](https://github.com/lintel-rs/lintel), a JSON Schema toolkit.

## License

Apache-2.0
