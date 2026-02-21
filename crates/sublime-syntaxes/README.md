# sublime-syntaxes

[![Crates.io](https://img.shields.io/crates/v/sublime-syntaxes.svg)](https://crates.io/crates/sublime-syntaxes)
[![docs.rs](https://docs.rs/sublime-syntaxes/badge.svg)](https://docs.rs/sublime-syntaxes)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/sublime-syntaxes.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Precompiled [Sublime Text syntax definitions](https://www.sublimetext.com/docs/syntax.html) for languages not included in [syntect](https://github.com/trishume/syntect)'s default set.

Syntax files in `syntaxes/` are compiled into a binary `SyntaxSet` at build time, so consumers pay no YAML parsing cost at runtime.

## Included syntaxes

- TOML

## Usage

```rust
use sublime_syntaxes::extra_syntax_set;

let extras = extra_syntax_set();
if let Some(syntax) = extras.find_syntax_by_token("toml") {
    println!("Found: {}", syntax.name);
}
```

To add a new syntax, drop a `.sublime-syntax` file into `syntaxes/` and rebuild.

## License

Apache-2.0
