# jsonschema-schema

[![Crates.io](https://img.shields.io/crates/v/jsonschema-schema.svg)](https://crates.io/crates/jsonschema-schema)
[![docs.rs](https://docs.rs/jsonschema-schema/badge.svg)](https://docs.rs/jsonschema-schema)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/jsonschema-schema.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Typed Rust structs for JSON Schema (draft 2020-12) documents. Part of the
[Lintel](https://github.com/lintel-rs/lintel) project.

Unlike raw `serde_json::Value`, this crate gives you named fields for every
standard keyword — `properties`, `items`, `allOf`, `$ref`, `format`, and so
on — so you can pattern-match and navigate schemas without string lookups.

## Features

- **Strongly typed** — `Schema`, `SchemaValue` (object or boolean),
  `TypeValue` (single or union), and `SimpleType` (the seven primitive
  type names) with full serde round-tripping
- **All standard keywords** — core identifiers, metadata, validation,
  applicators, composition, conditionals, content, and dependencies
- **Editor extensions** — first-class `x-taplo`, `x-tombi-*`, `x-intellij-*`,
  and `x-lintel` extension structs
- **Catch-all** — unknown properties are preserved in `extra: BTreeMap<String, Value>`
- **Pointer navigation** — `navigate_pointer` walks a JSON Pointer path
  through nested schemas, resolving `$ref` along the way
- **Helper utilities** — `ref_name`, `resolve_ref`, `Schema::type_str`,
  `Schema::description`, and more

## Usage

### Parsing a schema from JSON

```rust
use jsonschema_schema::{Schema, SchemaValue, SimpleType, TypeValue};

let json = serde_json::json!({
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "title": "User",
    "properties": {
        "name": { "type": "string" },
        "age": { "type": "integer", "minimum": 0 }
    },
    "required": ["name"]
});

let schema: Schema = serde_json::from_value(json).unwrap();

assert_eq!(schema.title.as_deref(), Some("User"));
assert_eq!(schema.required_set(), &["name"]);

// type_str() produces a human-readable summary
assert_eq!(schema.type_str().as_deref(), Some("object"));

// Access a nested property schema
let name_sv = schema.properties.as_ref().unwrap().get("name").unwrap();
let name = name_sv.as_schema().unwrap();
assert!(matches!(name.type_, Some(TypeValue::Single(ref t)) if *t == SimpleType::String));
```

### Building a schema programmatically

```rust
use jsonschema_schema::{Schema, SchemaValue, SimpleType, TypeValue};
use indexmap::IndexMap;

let mut props = IndexMap::new();
props.insert("email".to_string(), SchemaValue::Schema(Box::new(Schema {
    type_: Some(TypeValue::Single(SimpleType::String)),
    format: Some("email".into()),
    ..Default::default()
})));

let schema = Schema {
    type_: Some(TypeValue::Single(SimpleType::Object)),
    properties: Some(props),
    required: Some(vec!["email".into()]),
    ..Default::default()
};

let json = serde_json::to_value(&schema).unwrap();
assert_eq!(json["required"], serde_json::json!(["email"]));
assert_eq!(json["properties"]["email"]["format"], "email");
```

### Navigating with JSON Pointers

`navigate_pointer` resolves a [RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901)
JSON Pointer through the typed schema tree, automatically following `$ref`
references within the same document.

```rust
use jsonschema_schema::{Schema, SchemaValue, SimpleType, TypeValue, navigate_pointer};
use std::collections::BTreeMap;

// Build a schema with $defs and a $ref
let item = SchemaValue::Schema(Box::new(Schema {
    type_: Some(TypeValue::Single(SimpleType::String)),
    ..Default::default()
}));
let mut defs = BTreeMap::new();
defs.insert("Tag".to_string(), item);

let root = SchemaValue::Schema(Box::new(Schema {
    defs: Some(defs),
    ..Default::default()
}));

let result = navigate_pointer(&root, &root, "/$defs/Tag").unwrap();
let tag = result.as_schema().unwrap();
assert_eq!(tag.type_str().as_deref(), Some("string"));
```

### Extracting a ref name

```rust
use jsonschema_schema::ref_name;

assert_eq!(ref_name("#/$defs/Address"), "Address");
assert_eq!(ref_name("./other.json"), "other.json");
```

## License

Apache-2.0
