# sublime-syntaxes

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
