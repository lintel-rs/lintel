# cargo-furnish

[![Crates.io](https://img.shields.io/crates/v/cargo-furnish.svg)](https://crates.io/crates/cargo-furnish)
[![docs.rs](https://docs.rs/cargo-furnish/badge.svg)](https://docs.rs/cargo-furnish)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/cargo-furnish.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Furnish Rust crates with standardized `Cargo.toml` metadata, READMEs, and doc attributes. Reads workspace conventions and applies them to individual crates for consistent publishing.

## Why

Publishing a multi-crate workspace means every crate needs the same boilerplate: workspace-inherited fields, badge links, a license section, keywords, categories, and a `#![doc = include_str!("../README.md")]` line. Doing this by hand is tedious and inconsistent. `cargo furnish` automates the whole thing — it checks what's wrong, auto-fixes what it can, and tells you exactly what command to run for the rest.

## Commands

### `cargo furnish check`

Lint all workspace members (or a single crate) for metadata issues. Each diagnostic includes the exact command to fix it.

```shell
# Check every crate in the workspace
cargo furnish check

# Check a single crate by name or path
cargo furnish check schemastore
cargo furnish check crates/schemastore
```

**What it checks:**

| Code                      | What it catches                                                                             |
| ------------------------- | ------------------------------------------------------------------------------------------- |
| `missing_description`     | No `description` in `[package]`                                                             |
| `missing_keywords`        | No `keywords` in `[package]`                                                                |
| `missing_categories`      | No `categories` in `[package]`                                                              |
| `not_workspace_inherited` | Fields like `edition`, `license`, `repository` not using `.workspace = true`                |
| `unnecessary_readme`      | Explicit `readme` field (Cargo auto-discovers `README.md`)                                  |
| `field_order`             | `[package]` fields in non-canonical order                                                   |
| `section_order`           | Top-level sections in wrong order (expected: `[package]`, `[lints]`, `[dependencies]`, ...) |
| `missing_lints`           | No `[lints]` section when the workspace defines one                                         |
| `missing_readme`          | No `README.md` file                                                                         |
| `default_readme`          | README has no content beyond the auto-generated template                                    |
| `missing_crates_badge`    | README missing the crates.io badge                                                          |
| `missing_docs_badge`      | README missing the docs.rs badge                                                            |
| `missing_ci_badge`        | README missing the CI badge                                                                 |
| `missing_license_badge`   | README missing the license badge                                                            |
| `missing_license_section` | README missing `## License` section                                                         |
| `missing_doc_include`     | Source file missing `#![doc = include_str!("../README.md")]`                                |
| `doc_comment_exists`      | Hand-written `//!` doc comments that would be replaced                                      |

### `cargo furnish check --fix`

Auto-fix everything that doesn't require user input: field ordering, section ordering, workspace inheritance, `[lints]` section, `readme` field removal, and doc include injection.

```shell
cargo furnish check --fix
```

### `cargo furnish update`

Write or update a crate's `Cargo.toml` metadata, `README.md`, and doc include. Targets a single crate by name or path.

```shell
# Set metadata for a new crate
cargo furnish update \
  --description "Parse and match files against the SchemaStore catalog" \
  --keywords "json-schema,schemastore,validation" \
  --categories "development-tools" \
  schemastore

# Add custom README content (inserted between description and License)
cargo furnish update \
  --readme "## Usage\n\n\`\`\`rust\nuse schemastore::parse_catalog;\n\`\`\`" \
  schemastore

# Overwrite an existing README and doc comments
cargo furnish update --force \
  --description "Updated description" \
  --readme "## Features\n\n- Feature one\n- Feature two" \
  schemastore
```

**Flags:**

| Flag                     | Description                                                                         |
| ------------------------ | ----------------------------------------------------------------------------------- |
| `--description TEXT`     | Crate description (used in `Cargo.toml` and as the first line of the README)        |
| `--readme TEXT`          | Markdown body inserted between the description and `## License` (`\n` is unescaped) |
| `--keywords K1,K2,...`   | Comma-separated keywords for `Cargo.toml`                                           |
| `--categories C1,C2,...` | Comma-separated crates.io categories                                                |
| `--force`                | Overwrite existing README and replace hand-written `//!` doc comments               |

Without `--force`, update will not overwrite an existing `README.md` or replace `//!` doc comments — it prints a warning showing the existing contents instead.

## What it generates

**`Cargo.toml`** — fills in missing fields, converts to `.workspace = true` where applicable, and reorders fields and sections to a canonical layout.

**`README.md`** — generates a standard README with:

- Crate name heading
- crates.io, docs.rs, CI, and license badges
- Description from `Cargo.toml`
- Custom body content (from `--readme`)
- `## License` section

**`src/lib.rs` / `src/main.rs`** — prepends `#![doc = include_str!("../README.md")]` so the README becomes the crate-level rustdoc.

## Typical workflow

```shell
# 1. Auto-fix everything mechanical
cargo furnish check --fix

# 2. See what's left
cargo furnish check

# 3. Follow the diagnostic help messages — each one tells you what to run
cargo furnish update --description "..." --keywords "..." my-crate
cargo furnish update --force --readme "..." my-crate

# 4. Confirm zero issues
cargo furnish check
```

## License

Apache-2.0
