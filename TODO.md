# TODO

Ideas and planned work for Lintel. Nothing here is committed — these are directions we're exploring.

Have your own idea? [Open a GitHub issue](https://github.com/lintel-rs/lintel/issues/new) with the `feature request` label.

## Schema Discovery & Caching

- [ ] `schemastore` command for looking up schemas and their information from the catalog
- [ ] Commands for managing and clearing the schema cache
- [ ] Cache schema validation results (not just schemas themselves)

## Validation

- [ ] CSV validation and parsing (needs design for how to specify schemas in `lintel.toml`)
- [ ] `.env` file validation — type checking, required keys, referencing a `.env.example` as schema
- [ ] XML validation (pom.xml, .csproj, Android manifests — still everywhere)
- [ ] HCL/Terraform validation
- [ ] INI / .properties file validation
- [ ] Secrets detection — flag values that look like API keys, tokens, or passwords in config files
- [ ] Deprecation warnings — warn when config fields are deprecated in newer schema versions
- [ ] Cross-file validation — e.g. ensure docker-compose service names match Dockerfile paths
- [ ] Inline ignore comments (`# lintel-ignore` / `// lintel-ignore-next-line`)

## Formatting

- [ ] `format` command with biome.json compatibility for JSON files
- [ ] YAML formatting
- [ ] TOML formatting
- [ ] Markdown and MDX formatting
- [ ] Sort keys in JSON/YAML/TOML (opinionated mode for deterministic configs)
- [ ] Trailing newline / trailing comma normalization

The formatting goal is to cover what [Biome](https://biomejs.dev/) doesn't — YAML, TOML, Markdown — and stay compatible where they overlap.

## Editor & IDE

- [ ] LSP server — real-time validation, hover-to-see-schema, completions from schema
- [ ] VS Code extension
- [ ] Neovim plugin
- [ ] JetBrains plugin

## CI & Git Integration

- [ ] SARIF output for GitHub Code Scanning / Security tab integration
- [ ] `--changed` flag — only validate files changed since a base ref (fast PR checks)
- [ ] Baseline / error suppression file — adopt Lintel in large repos without fixing everything first
- [ ] Pre-commit hook (`pre-commit` framework compatible)
- [ ] PR comment bot — post validation results as inline PR comments
- [ ] JSON / JUnit / SARIF report output for CI dashboards

## Distribution

- [ ] Homebrew formula
- [ ] WASM build for browser playground

## Documentation

- [ ] Documentation website — usage guides, configuration reference, catalog docs, and integration examples

## Benchmarks

- [ ] Performance benchmarks vs other validators (e.g. ajv, check-jsonschema, v8r)

## DX

- [ ] Better `--verbose` logging showing which schema was resolved for each file
- [ ] `explain` command — given a validation error, explain what it means and how to fix it
- [ ] `schema` command — pass in a file, resolve its schema, and explore all available options (required/optional fields, types, defaults, descriptions, enum values). Interactive schema explorer
- [ ] Static documentation website generated from SchemaStore schemas — browsable reference docs for tsconfig.json, package.json, etc. with all fields, types, defaults, and descriptions rendered as clean web pages
- [ ] Auto-fix suggestions for common schema violations (missing required fields, wrong types)
- [ ] Watch mode — continuously validate on file save
- [ ] Config playground / REPL — interactively test a config against a schema
- [ ] `diff` command — show what changed between two versions of a config and whether the diff is schema-valid

## Library & Extensibility

- [ ] Publish `lintel-check` as a standalone Rust library on crates.io
- [ ] Plugin system for custom validators (e.g. "all S3 bucket names must match this regex")
- [ ] WASM plugin support for non-Rust custom validators
