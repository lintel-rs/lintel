# glob-set

[![Crates.io](https://img.shields.io/crates/v/glob-set.svg)](https://crates.io/crates/glob-set)
[![docs.rs](https://docs.rs/glob-set/badge.svg)](https://docs.rs/glob-set)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/glob-set.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

A globset-compatible glob matcher -- `no_std`, no regex, built on glob-matcher

## Usage

```rust
use glob_set::{Glob, GlobSet, GlobSetBuilder};

let mut builder = GlobSetBuilder::new();
builder.add(Glob::new("*.rs").unwrap());
builder.add(Glob::new("*.toml").unwrap());
let set = builder.build().unwrap();

assert!(set.is_match("main.rs"));
assert!(set.is_match("Cargo.toml"));
assert!(!set.is_match("index.js"));
```

## Features

- `no_std` compatible (uses `alloc`)
- No `regex` dependency -- built on [glob-matcher](https://crates.io/crates/glob-matcher)
- Aho-Corasick pre-filter for efficient multi-pattern matching
- API-compatible with [globset](https://crates.io/crates/globset): `Glob`, `GlobSet`, `GlobBuilder`, `Candidate`
- Supports `*`, `**`, `?`, `[...]`, and `{a,b}` patterns

## License

Apache-2.0
