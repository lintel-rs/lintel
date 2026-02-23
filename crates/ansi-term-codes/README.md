# ansi-term-codes

[![Crates.io](https://img.shields.io/crates/v/ansi-term-codes.svg)](https://crates.io/crates/ansi-term-codes)
[![docs.rs](https://docs.rs/ansi-term-codes/badge.svg)](https://docs.rs/ansi-term-codes)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/ansi-term-codes.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

ANSI terminal escape code constants. Zero-dependency, `no_std` compatible.

## Usage

```rust
use ansi_term_codes::{BOLD, RED, RESET};

println!("{BOLD}{RED}error:{RESET} something went wrong");
```

## Constants

- **Attributes:** `BOLD`, `DIM`, `ITALIC`, `UNDERLINE`, `RESET`
- **Colors:** `RED`, `GREEN`, `YELLOW`, `BLUE`, `MAGENTA`, `CYAN`
- **Bold colors:** `BOLD_RED`, `BOLD_GREEN`

## License

Apache-2.0
