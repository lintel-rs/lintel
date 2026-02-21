# markdown-to-ansi

Render Markdown as ANSI-formatted terminal text.

Uses [pulldown-cmark](https://crates.io/crates/pulldown-cmark) for CommonMark
parsing and [syntect](https://crates.io/crates/syntect) for syntax highlighting
of fenced code blocks.

## Usage

```rust
let opts = markdown_to_ansi::Options {
    syntax_highlight: true,
    width: Some(80),
};

let ansi = markdown_to_ansi::render("# Hello\n\nSome **bold** text.", &opts);
println!("{ansi}");
```

For inline-only rendering (no block-level elements):

```rust
let opts = markdown_to_ansi::Options {
    syntax_highlight: true,
    width: None,
};

let inline = markdown_to_ansi::render_inline("Use `foo` for **bar**", &opts);
```

## Rendering

- **Paragraphs** — reflowed (single newlines become spaces), word-wrapped to
  the configured `width`
- **Headings** — rendered in bold
- **Code blocks** — syntax-highlighted with background padding, or preserved
  with fence markers when `syntax_highlight` is false
- **Lists** — unordered (`-`) and ordered (`1.`), with continuation indent for
  wrapped lines
- **Inline markup** — `code` in blue, **bold**, _italic_, and
  [links](https://example.com) as OSC 8 terminal hyperlinks
- **Soft breaks** — converted to spaces (paragraph reflow)
