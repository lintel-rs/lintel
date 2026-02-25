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

## Benchmarks

All benchmarks use [Criterion](https://crates.io/crates/criterion) and compare `glob-set` against the original [`globset`](https://crates.io/crates/globset) crate. The single-pattern benchmarks (`ext`, `short`, `long`, `many_short`) are sourced from the [upstream ripgrep globset benchmarks](https://github.com/BurntSushi/ripgrep/blob/master/crates/globset/benches/bench.rs). The multi-pattern benchmarks (`glob_set` vs `globset`) test 8 patterns against 10 paths, representative of typical schema-catalog file matching.

Run them with:

```sh
cargo bench -p glob-set
```

### Multi-pattern `GlobSet` (8 patterns x 10 paths)

| Benchmark       | Time   | vs globset      |
| --------------- | ------ | --------------- |
| `glob_set`      | 289 ns | **1.3x faster** |
| `globset`       | 379 ns | baseline        |
| `tiny_glob_set` | 313 ns | 1.2x faster     |

### Build time (8 patterns)

| Benchmark             | Time   | vs globset      |
| --------------------- | ------ | --------------- |
| `glob_set_build`      | 5.5 µs | **8.4x faster** |
| `globset_build`       | 46 µs  | baseline        |
| `tiny_glob_set_build` | 2.2 µs | **21x faster**  |

The build-time advantage comes from avoiding regex compilation entirely. This matters in applications that recompile pattern sets frequently (e.g. re-reading a schema catalog on file change).

### Single-pattern matching (upstream ripgrep benchmarks)

| Benchmark                                | glob-set | globset | Notes                      |
| ---------------------------------------- | -------- | ------- | -------------------------- |
| `ext` (`*.txt`)                          | 53 ns    | 53 ns   | Tied                       |
| `short` (`some/**/needle.txt`)           | 51 ns    | 23 ns   | Regex wins on `**`         |
| `long` (`some/**/needle.txt`, deep path) | 285 ns   | 53 ns   | Regex wins on `**`         |
| `many_short` (14-pattern set)            | 249 ns   | 102 ns  | Regex wins on set matching |

For single-pattern `**` matching, `globset`'s regex backend is faster. The `glob-set` advantage shows in multi-pattern `GlobSet` matching (where the Aho-Corasick pre-filter and strategy engine apply) and dramatically in build times.

## Acknowledgments

This crate is based on the API and design of [globset](https://crates.io/crates/globset) by [Andrew Gallant (BurntSushi)](https://github.com/BurntSushi), part of the [ripgrep](https://github.com/BurntSushi/ripgrep) project. The original `globset` crate is an excellent, battle-tested library — `glob-set` reimplements its API surface with a `no_std`-compatible, regex-free backend built on [glob-matcher](https://crates.io/crates/glob-matcher).

## License

Apache-2.0
