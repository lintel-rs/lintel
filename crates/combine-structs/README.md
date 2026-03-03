# combine-structs

[![Crates.io](https://img.shields.io/crates/v/combine-structs.svg)](https://crates.io/crates/combine-structs)
[![docs.rs](https://docs.rs/combine-structs/badge.svg)](https://docs.rs/combine-structs)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/combine-structs.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Proc macros for compile-time struct field merging.

Define fields once in separate source structs, then merge them all into a
single flat target struct — no runtime cost, no `#[serde(flatten)]`, and
field access stays direct (`target.field`, not `target.group.field`).

## Why

Some types are naturally decomposed into logical groups, but consumers
expect a single flat struct. JSON Schema, for example, defines seven
vocabularies (Core, Applicator, Validation, etc.) with ~60 keyword fields
total. We want each vocabulary in its own file with its own docs and
derives, but the final `Schema` struct should have all fields at the top
level so users write `schema.title` instead of `schema.meta_data.title`.

The alternatives have drawbacks:

- **`#[serde(flatten)]`** — works for serialization, but doubles memory
  (nested structs), breaks `#[derive(Default)]` expectations, and doesn't
  compose well with `schemars::JsonSchema`.
- **Copy-paste** — fields appear in two places (vocabulary struct + final
  struct), creating a maintenance burden.
- **A single huge file** — no logical grouping, hard to navigate.

`combine-structs` solves this: define fields once per vocabulary struct,
derive `Fields`, and let `#[combine_fields(...)]` merge them at compile
time.

## How it works

Two proc macros work together via a shared in-memory cache within the
compiler process:

1. **`#[derive(Fields)]`** on a source struct stores its field definitions
   (including all attributes, doc comments, and visibility) in the cache.

2. **`#[combine_fields(A, B, C)]`** on the target struct reads those
   cached field definitions and emits the target struct with all fields
   merged in.

The target struct's own fields (defined in its body) are preserved and
appear alongside the merged fields.

## Usage

```rust
use combine_structs::Fields;
use combine_structs::combine_fields;

/// Position fields.
#[derive(Fields, Debug, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Appearance fields.
#[derive(Fields, Debug, Default)]
pub struct Appearance {
    pub color: String,
    pub visible: bool,
}

/// A sprite with position and appearance fields merged in,
/// plus its own `name` field.
#[combine_fields(Position, Appearance)]
#[derive(Debug, Default)]
pub struct Sprite {
    pub name: String,
}

// Sprite now has all five fields: x, y, color, visible, name.
let s = Sprite {
    name: "player".into(),
    x: 10.0,
    y: 20.0,
    color: "red".into(),
    visible: true,
};
assert_eq!(s.x, 10.0);
assert_eq!(s.name, "player");
```

### With serde and schemars

Attributes on source struct fields are preserved through the merge,
so `#[serde(rename = ...)]` and `#[schemars(...)]` work as expected:

```rust
use combine_structs::{Fields, combine_fields};
use serde::{Serialize, Deserialize};

#[derive(Fields, Debug, Default, Serialize, Deserialize)]
pub struct CoreVocabulary {
    /// The `$schema` keyword.
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// The `$id` keyword.
    #[serde(rename = "$id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[combine_fields(CoreVocabulary)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Schema {
    /// Extra extension field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// Schema has: schema, id, and description — all at the top level.
let s = Schema {
    schema: Some("https://json-schema.org/draft/2020-12/schema".into()),
    description: Some("A test".into()),
    ..Default::default()
};

let json = serde_json::to_value(&s).unwrap();
assert_eq!(json["$schema"], "https://json-schema.org/draft/2020-12/schema");
assert_eq!(json["description"], "A test");
```

### Cross-module usage

Source structs can live in any module with no special annotations:

```rust
use combine_structs::{Fields, combine_fields};

mod vocabularies {
    use combine_structs::Fields;

    #[derive(Fields, Debug, Default)]
    pub struct Position { pub x: f64, pub y: f64 }

    #[derive(Fields, Debug, Default)]
    pub struct Appearance { pub color: String, pub visible: bool }
}

#[combine_fields(Position, Appearance)]
#[derive(Debug, Default)]
pub struct Sprite {
    pub name: String,
}

let s = Sprite { name: "player".into(), x: 10.0, y: 20.0,
                 color: "red".into(), visible: true };
assert_eq!(s.x, 10.0);
assert_eq!(s.name, "player");
```

### Real-world usage

In [Lintel](https://github.com/lintel-rs/lintel), this crate merges
seven JSON Schema vocabulary structs (~60 fields) into a single flat
`Schema` type:

```rust,ignore
#[combine_fields(
    CoreVocabulary,
    ApplicatorVocabulary,
    UnevaluatedVocabulary,
    ValidationVocabulary,
    MetaDataVocabulary,
    FormatAnnotationVocabulary,
    ContentVocabulary
)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Schema {
    // Non-standard extension fields defined here
    pub markdown_description: Option<String>,
    // ...
}
```

Each vocabulary lives in its own module with focused docs and tests,
but the final `Schema` struct has all fields directly accessible.

## License

Apache-2.0
