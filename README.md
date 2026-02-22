<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/lintel-rs/lintel/master/assets/logo.png">
  <img src="https://raw.githubusercontent.com/lintel-rs/lintel/master/assets/logo.png" alt="Lintel" width="300">
</picture>

**Fast, multi-format JSON Schema linter for all your config files.**

[![CI][ci-badge]][ci-url]
[![crates.io][crates-badge]][crates-url]

[ci-badge]: https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/lintel-rs/lintel/actions/workflows/ci.yml
[crates-badge]: https://img.shields.io/crates/v/lintel?color=60a5fa
[crates-url]: https://crates.io/crates/lintel

</div>

**Lintel** validates JSON, YAML, TOML, JSON5, and JSONC files against [JSON Schema](https://json-schema.org/) in a single command. It auto-discovers schemas via [SchemaStore](https://www.schemastore.org/), the [Lintel catalog](https://catalog.lintel.tools/), inline `$schema` properties, and YAML modelines — zero config required.

**Fast.** Written in Rust with no async runtime, deterministic schema caching, and pre-compiled SchemaStore catalog matching. Warm runs are pure computation.

**Drop-in CI check.** Machine-parseable output, nonzero exit codes on failure, `.gitignore`-aware file walking. Add one line to your pipeline and catch config mistakes before they ship.

**Zero config.** Point it at your repo and go. Lintel figures out which schemas to use.

## Installation

```shell
cargo install lintel
```

Or with npm:

```shell
npx lintel check
```

Or with Nix:

```shell
nix run github:lintel-rs/lintel
```

## Usage

```shell
# validate with rich terminal output
lintel check

# validate with CI-friendly one-error-per-line output
lintel ci

# generate a lintel.toml with auto-detected schemas
lintel init

# convert between formats
lintel convert config.yaml --to toml
```

## Schema Discovery

Lintel auto-discovers schemas in priority order:

1. **YAML modeline** — `# yaml-language-server: $schema=...`
2. **Inline `$schema` property** — in the document itself
3. **`lintel.toml` mappings** — custom `[schemas]` table entries
4. **Lintel catalog** — schemas for tools not in SchemaStore (Cargo.toml, Claude Code agents/skills/commands, devenv.yaml, and more)
5. **SchemaStore catalog** — matching by filename

Files without a matching schema are silently skipped. Lintel respects `.gitignore` — `node_modules`, `target/`, and build artifacts are skipped automatically.

## The Lintel Catalog

The [Lintel catalog](https://catalog.lintel.tools/) provides schemas for tools that don't have SchemaStore entries. It's fetched automatically alongside SchemaStore — no configuration needed.

Currently includes schemas for:

- **Cargo.toml** — Rust package manifests
- **Claude Code** — agent, skill, and command definitions
- **devenv.yaml** — devenv configuration

To add your own catalogs, use `registries` in `lintel.toml`:

```toml
registries = ["github:my-org/my-schemas"]
```

The `github:org/repo` shorthand resolves to `https://raw.githubusercontent.com/org/repo/master/catalog.json`.

## Configuration

Lintel supports project configuration via `lintel.toml`:

```toml
# stop walking up the directory tree
root = true

# exclude files from validation
exclude = ["vendor/**", "testdata/**"]

# map file patterns to schema URLs
[schemas]
"my-config.yaml" = "https://example.com/my-schema.json"
".ci/*.yml" = "//schemas/ci.json"  # // resolves relative to lintel.toml

# additional schema catalogs
registries = ["github:my-org/my-schemas"]

# rewrite schema URLs (e.g. for local development)
[rewrite]
"http://localhost:8000/" = "//schemas/"

# per-file overrides
[[override]]
files = ["schemas/vector.json"]
validate_formats = false
```

## Adding Lintel to devenv

Add Lintel as an input in `devenv.yaml`:

```yaml
inputs:
  lintel:
    url: github:lintel-rs/lintel
    flake: true
```

Then use it in `devenv.nix`:

```nix
{ pkgs, inputs, ... }:

let
  lintel = inputs.lintel.packages.${pkgs.system}.default;
in
{
  packages = [ lintel ];

  # optional: run lintel as a pre-commit hook
  git-hooks.hooks.lintel = {
    enable = true;
    name = "lintel";
    entry = "${lintel}/bin/lintel check";
    types_or = [ "json" "yaml" ];
  };
}
```

## GitHub Action

Use the official [lintel-rs/action](https://github.com/lintel-rs/action):

```yaml
- uses: lintel-rs/action@v1
```

## License

Copyright Ian Macalinao. Licensed under the [Apache License, Version 2.0](LICENSE).
