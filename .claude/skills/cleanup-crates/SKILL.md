---
name: cleanup-crates
description: >
  Clean up crates/ — standardize Cargo.toml metadata and README files.
  Use when the user asks to "clean up crates", "fix crate metadata",
  "standardize Cargo.toml", "fix crate READMEs", or "clean up crates/".
---

# Clean up crates

## Procedure

1. `cargo-furnish check --fix` — auto-fix everything that doesn't need user input.
2. `cargo-furnish check` — see what's left. Each diagnostic tells you exactly what command to run.
3. For each remaining crate, run `cargo-furnish update` with the flags the diagnostics suggest. Read the crate's existing README and source to write good descriptions.
4. `cargo-furnish check` — verify zero issues remain.

## Content guidelines

- **`description`** — concise, factual, matches the README's first paragraph after badges.
- **`keywords`** — 1–5 lowercase. Include `"json-schema"` for lintel-related crates.
- **`categories`** — 1–2 valid crates.io categories.
- **`--readme`** — use `\\n` for newlines. Include a Usage or Features section at minimum.
- Link to the project as `[Lintel](https://github.com/lintel-rs/lintel)` in lintel-\* crate descriptions.
