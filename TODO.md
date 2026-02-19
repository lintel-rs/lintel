# TODO

Ideas and planned work for Lintel. Nothing here is committed — these are directions we're exploring.

Have your own idea? [Open a GitHub issue](https://github.com/lintel-rs/lintel/issues/new) with the `feature request` label.

## Schema Discovery & Caching

- [ ] `schemastore` command for looking up schemas and their information from the catalog
- [ ] Commands for managing and clearing the schema cache
- [ ] Cache schema validation results (not just schemas themselves)
- [ ] Ensure `$schema` properties and YAML modeline comments always override automatic schema detection

## Validation

- [ ] Markdown frontmatter validation (YAML/TOML frontmatter in `.md` files)
- [ ] Claude Code skill, command, and agent schema validation (depends on frontmatter support)
- [ ] CSV validation and parsing (needs design for how to specify schemas in `lintel.toml`)

## Formatting

- [ ] `format` command with biome.json compatibility for JSON files
- [ ] YAML formatting
- [ ] TOML formatting
- [ ] Markdown and MDX formatting

The formatting goal is to cover what [Biome](https://biomejs.dev/) doesn't — YAML, TOML, Markdown — and stay compatible where they overlap.

## DX

- [ ] Better `--verbose` logging showing which schema was resolved for each file
