# lintel-schemastore-catalog

CLI tool that mirrors the entire [SchemaStore](https://www.schemastore.org/) catalog (catalog index + all schema files) into a git repo, keeping it up to date via CI.

This gives [lintel](https://github.com/lintel-rs/lintel) a self-hosted, version-controlled schema source at [`lintel-rs/schemastore-catalog`](https://github.com/lintel-rs/schemastore-catalog).

## How it works

1. Fetches the [SchemaStore catalog](https://www.schemastore.org/api/json/catalog.json)
2. Downloads every schema referenced in the catalog concurrently
3. Derives clean filenames from schema names (e.g. "Releasaurus Config" → `releasaurus-config.json`)
4. Rewrites the catalog's `url` fields to point to `raw.githubusercontent.com` URLs in the mirror repo
5. Validates each download is parseable JSON; skips failures gracefully
6. Writes `catalog.json` + `schemas/*.json` to the output directory

## Usage

```
lintel-schemastore-catalog generate -o <DIR> [--concurrency N] [--base-url URL]
lintel-schemastore-catalog update [--repo OWNER/NAME] [--branch BRANCH]
lintel-schemastore-catalog version
```

### `generate`

Fetch the SchemaStore catalog and download all schemas to a local directory.

```sh
lintel-schemastore-catalog generate -o /tmp/catalog
```

### `update`

CI command that clones the mirror repo, regenerates the catalog, runs `lintel check`, and pushes if there are changes. Requires `GITHUB_TOKEN` to be set.

```sh
GITHUB_TOKEN=... lintel-schemastore-catalog update
```

### Logging

Set `LINTEL_LOG` to control log output:

```sh
LINTEL_LOG=info lintel-schemastore-catalog generate -o /tmp/catalog
LINTEL_LOG=debug lintel-schemastore-catalog generate -o /tmp/catalog  # includes per-schema downloads
```

## Output structure

```
catalog.json
schemas/
  tsconfig.json
  package-json.json
  github-workflow.json
  releasaurus-config.json
  ...
```
