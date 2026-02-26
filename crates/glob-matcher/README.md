# glob-matcher

[![Crates.io](https://img.shields.io/crates/v/glob-matcher.svg)](https://crates.io/crates/glob-matcher)
[![docs.rs](https://docs.rs/glob-matcher/badge.svg)](https://docs.rs/glob-matcher)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/glob-matcher.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

A `no_std` fork of [glob-match](https://github.com/devongovett/glob-match) by [Devon Govett](https://github.com/devongovett).

## Origin

This crate is a fork of [`glob-match`](https://crates.io/crates/glob-match) v0.2.1,
an extremely fast glob matching library. Changes from upstream:

- `no_std` support (`std::path::is_separator` replaced with an inline helper, `std` → `core`/`alloc`)
- Ported [PR #18](https://github.com/devongovett/glob-match/pull/18): `skip_to_separator` optimization for `**` patterns (~2x faster), fixes [issue #9](https://github.com/devongovett/glob-match/issues/9)
- Ported [PR #24](https://github.com/devongovett/glob-match/pull/24): empty brace alternatives (`a{,/**}`) and `**` inside braces
- Fixed [issue #8](https://github.com/devongovett/glob-match/issues/8): leading `**` inside braces (`{**/*b}`)

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

## Benchmarks

Matching 17 patterns against a single path (`cargo bench -p glob-matcher`):

| Crate            | Time       | vs glob-matcher |
| ---------------- | ---------- | --------------- |
| **glob-matcher** | **108 ns** | 1.0x            |
| glob-match 0.2.1 | 209 ns     | 1.9x slower     |
| glob 0.3         | 307 ns     | 2.8x slower     |
| globset 0.4      | 15,746 ns  | 146x slower     |

## License

MIT (same as the original glob-match)
