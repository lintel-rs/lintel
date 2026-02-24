# glob-matcher

[![Crates.io](https://img.shields.io/crates/v/glob-matcher.svg)](https://crates.io/crates/glob-matcher)
[![docs.rs](https://docs.rs/glob-matcher/badge.svg)](https://docs.rs/glob-matcher)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/glob-matcher.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

A `no_std` fork of [glob-match](https://github.com/devongovett/glob-match) by [Devon Govett](https://github.com/devongovett).

## Origin

This crate is a minimal fork of [`glob-match`](https://crates.io/crates/glob-match) v0.2.1,
an extremely fast glob matching library. The only change is replacing
`std::path::is_separator` with an inline helper and switching from `std` to
`core`/`alloc`, making the crate usable in `no_std` environments.

All credit for the matching algorithm goes to Devon Govett and the
glob-match contributors. The original repository is at
<https://github.com/devongovett/glob-match>.

## Usage

```rust
use glob_matcher::glob_match;

assert!(glob_match("**/*.rs", "src/main.rs"));
assert!(glob_match("*.{js,ts}", "app.ts"));
assert!(!glob_match("*.rs", "src/main.rs"));
```

## Features

- `no_std` compatible (uses `alloc`)
- Supports `*`, `**`, `?`, `[...]`, and `{a,b}` patterns
- Captures: extract matched segments with `glob_match_with_captures`

## License

MIT (same as the original glob-match)
