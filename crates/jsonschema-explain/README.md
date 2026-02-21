# jsonschema-explain

Render JSON Schema as human-readable terminal documentation, similar to a man page.

Takes a `serde_json::Value` containing a JSON Schema and produces formatted text with optional ANSI color output. No dependencies beyond `serde_json`.

## Usage

```rust
use serde_json::json;

let schema = json!({
    "title": "Example",
    "description": "An example schema",
    "type": "object",
    "required": ["name"],
    "properties": {
        "name": {
            "type": "string",
            "description": "The name"
        },
        "debug": {
            "type": "boolean",
            "default": false,
            "markdownDescription": "Enable `debug` mode"
        }
    }
});

// color=false for plain text, color=true for ANSI terminal output
// syntax_highlight=true to enable syntax highlighting in fenced code blocks
let output = jsonschema_explain::explain(&schema, "example", false, false);
println!("{output}");
```

## Output Format

```
EXAMPLE                         JSON Schema                         EXAMPLE

NAME
    Example - An example schema

DESCRIPTION
    An example schema

TYPE
    object

PROPERTIES
    name (string, required)
        The name

    debug (boolean)
        Enable debug mode
        Default: false
```

## Features

- **Man-page layout** with NAME, DESCRIPTION, TYPE, PROPERTIES, ITEMS, ONE OF / ANY OF / ALL OF, and DEFINITIONS sections
- **Nested properties** rendered with indentation (up to 3 levels deep)
- **`$ref` resolution** within the same document (`#/definitions/...`, `#/$defs/...`)
- **`allOf`/`oneOf`/`anyOf`** variants expanded inline when they resolve to objects with properties
- **Prefers `markdownDescription`** over `description` when both are present (common in VS Code / SchemaStore schemas)
- **Inline markdown rendering**: `` `code` ``, `**bold**`, `[text](url)`, and raw URLs
- **ANSI colors** when `color` is `true`:
  - Cyan for type annotations (`string`, `boolean | null`)
  - Green for property names
  - Yellow for section headers and `required` tags
  - Magenta for values (defaults, enums, constants)
  - Blue for inline code (backtick-delimited text)
  - Dim for URLs, metadata labels, variant numbers
- **No wrapping** - output is not pre-wrapped, letting the terminal or pager handle line wrapping at the actual terminal width
