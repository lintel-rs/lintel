# lintel-catalog-builder

[![Crates.io](https://img.shields.io/crates/v/lintel-catalog-builder.svg)](https://crates.io/crates/lintel-catalog-builder)
[![docs.rs](https://docs.rs/lintel-catalog-builder/badge.svg)](https://docs.rs/lintel-catalog-builder)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-catalog-builder.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Build a custom JSON Schema catalog from local schemas and external sources like [SchemaStore](https://www.schemastore.org/).

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## Config format

Create a `lintel-catalog.toml` in your catalog repo:

```toml
[catalog]
# Output targets
[target.local]
type = "dir"
dir = "../catalog-generated"
base_url = "https://raw.githubusercontent.com/your-org/catalog-generated/main/"

[target.pages]
type = "github-pages"
base_url = "https://catalog.your-domain.com/"
cname = "catalog.your-domain.com"

# Schema groups (locally defined)
[groups.my-schemas]
name = "My Schemas"
description = "Custom configuration schemas"

[groups.my-schemas.schemas]
config = { name = "Config", description = "App config", file-match = ["config.json"] }
external = { url = "https://example.com/schema.json", name = "External", description = "An external schema", file-match = ["*.ext.json"] }

# Group for organized schemas from external sources
[groups.github]
name = "GitHub"
description = "GitHub configuration files"

# External sources
[sources.schemastore]
url = "https://www.schemastore.org/api/json/catalog.json"

# Organize entries route schemas into groups by match patterns.
# Group metadata (name, description) comes from [groups.*] above.
[sources.schemastore.organize.github]
match = ["**.github**"]
```

## Target types

### `dir`

Writes output to a local directory (resolved relative to the config file):

- `dir` — output directory path
- `base_url` — URL prefix for schema references in `catalog.json`

### `github-pages`

Generates output optimized for GitHub Pages deployment:

- `base_url` — URL prefix for schema references
- `cname` (optional) — custom domain; writes a `CNAME` file
- `dir` (optional) — output directory (defaults to `.lintel-pages-output/<target-name>/`)

Additional files generated: `.nojekyll`, `CNAME`, `index.html`, `README.md`.

## CLI usage

```sh
# Build all targets
lintel-catalog-builder generate --config lintel-catalog.toml

# Build a specific target
lintel-catalog-builder generate --config lintel-catalog.toml --target pages

# Skip cache reads (force re-download)
lintel-catalog-builder generate --no-cache

# Control concurrency
lintel-catalog-builder generate --concurrency 50
```

## GitHub Pages deployment

### 1. Add a `github-pages` target to your config

```toml
[target.pages]
type = "github-pages"
base_url = "https://catalog.your-domain.com/"
cname = "catalog.your-domain.com"
```

### 2. Create the GitHub Actions workflow

Save as `.github/workflows/deploy-catalog.yml`:

```yaml
name: Deploy Catalog to GitHub Pages

on:
  push:
    branches: [master]
  workflow_dispatch:
  schedule:
    - cron: "0 6 * * *"

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5

      - name: Install lintel-catalog-builder
        run: |
          curl -fsSL https://github.com/lintel-rs/lintel/releases/latest/download/lintel-catalog-builder-x86_64-unknown-linux-gnu.tar.gz \
            | tar xz -C /usr/local/bin/

      - name: Generate catalog
        run: lintel-catalog-builder generate --config lintel-catalog.toml --target pages

      - uses: actions/configure-pages@v5

      - uses: actions/upload-pages-artifact@v4
        with:
          path: .lintel-pages-output/pages/

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

### 3. Enable GitHub Pages

Go to repo **Settings > Pages > Source** and select **GitHub Actions**.

### 4. Configure DNS (if using a custom domain)

Create a CNAME DNS record pointing your domain to `<your-org>.github.io`. The `CNAME` file in the generated output tells GitHub Pages to use the custom domain automatically.

## License

Apache-2.0
