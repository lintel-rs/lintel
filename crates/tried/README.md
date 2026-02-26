# tried

[![Crates.io](https://img.shields.io/crates/v/tried.svg)](https://crates.io/crates/tried)
[![docs.rs](https://docs.rs/tried/badge.svg)](https://docs.rs/tried)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/tried.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

A fast, compact double-array trie with `no_std` + `alloc` support. Fork of [yada](https://crates.io/crates/yada) providing O(key-length) exact-match and common-prefix search over a flat, cache-friendly byte buffer (4 bytes per node).

## Features

- **`no_std`** — only requires `alloc`
- **Compact** — 4 bytes per node in a single contiguous buffer
- **Fast** — O(key-length) lookups via double-array indexing
- **Zero-allocation iteration** — `common_prefix_search` returns a lazy iterator

## Examples

### Exact-match lookup

Build a trie from sorted key-value pairs, then look up keys in O(key-length) time. Keys must be sorted lexicographically and must not contain NUL bytes.

```rust
use tried::{DoubleArray, DoubleArrayBuilder};

// Keys must be sorted and unique.
let keyset: &[(&[u8], u32)] = &[
    (b"apple", 1),
    (b"banana", 2),
    (b"cherry", 3),
];
let bytes = DoubleArrayBuilder::build(keyset).unwrap();
let da = DoubleArray::new(bytes);

assert_eq!(da.exact_match_search(b"banana"), Some(2));
assert_eq!(da.exact_match_search(b"grape"), None);

// String keys work too (anything implementing AsRef<[u8]>).
assert_eq!(da.exact_match_search("cherry"), Some(3));
```

### Common-prefix search

Find all keys that are prefixes of a given input. Returns `(value, prefix_length)` pairs in ascending length order.

```rust
use tried::{DoubleArray, DoubleArrayBuilder};

let keyset: &[(&[u8], u32)] = &[
    (b"r", 0),
    (b"ru", 1),
    (b"rus", 2),
    (b"rust", 3),
];
let bytes = DoubleArrayBuilder::build(keyset).unwrap();
let da = DoubleArray::new(bytes);

let matches: Vec<_> = da.common_prefix_search("rustacean").collect();

// All four keys are prefixes of "rustacean":
//   "r"    → (value=0, len=1)
//   "ru"   → (value=1, len=2)
//   "rus"  → (value=2, len=3)
//   "rust" → (value=3, len=4)
assert_eq!(matches, vec![(0, 1), (1, 2), (2, 3), (3, 4)]);
```

### Zero-copy with borrowed data

`DoubleArray` is generic over any `Deref<Target = [u8]>`, so you can use borrowed slices to avoid copies.

```rust
use tried::{DoubleArray, DoubleArrayBuilder};

let keyset: &[(&[u8], u32)] = &[(b"hello", 42)];
let bytes = DoubleArrayBuilder::build(keyset).unwrap();

// Borrow the built bytes instead of moving them.
let da = DoubleArray::new(bytes.as_slice());
assert_eq!(da.exact_match_search("hello"), Some(42));
```

## License

MIT OR Apache-2.0
