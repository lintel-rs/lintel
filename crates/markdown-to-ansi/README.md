# markdown-to-ansi

[![Crates.io](https://img.shields.io/crates/v/markdown-to-ansi.svg)](https://crates.io/crates/markdown-to-ansi)
[![docs.rs](https://docs.rs/markdown-to-ansi/badge.svg)](https://docs.rs/markdown-to-ansi)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/markdown-to-ansi.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Render Markdown as ANSI-formatted terminal text

## Features

- Converts `CommonMark` Markdown to ANSI-escaped terminal text via `pulldown-cmark`
- Syntax-highlighted fenced code blocks using `syntect`
- Bold, italic, underline, colored headings, blockquotes, and lists
- Auto-wraps text to terminal width

## Usage

```rust
use markdown_to_ansi::{render, Options};

let opts = Options {
    syntax_highlight: true,
    width: Some(80),
};
let output = render("# Hello

Some **bold** text.", &opts);
println!("{output}");
```

## License

Apache-2.0
