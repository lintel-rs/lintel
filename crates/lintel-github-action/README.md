# lintel-github-action

[![Crates.io](https://img.shields.io/crates/v/lintel-github-action.svg)](https://crates.io/crates/lintel-github-action)
[![docs.rs](https://docs.rs/lintel-github-action/badge.svg)](https://docs.rs/lintel-github-action)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-github-action.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Validate JSON, YAML, and TOML files against [JSON Schema](https://json-schema.org/) in your pull requests. Creates a GitHub Check Run named **Lintel** with inline annotations using the [Checks API](https://docs.github.com/en/rest/checks/runs).

This is the binary behind the [`lintel-rs/action`](https://github.com/lintel-rs/action) GitHub Action.

## Quick start

```yaml
name: Lint
on: [pull_request]

permissions:
  checks: write
  contents: read

jobs:
  lintel:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: lintel-rs/action@v0
```

With zero configuration Lintel auto-discovers files and matches them against schemas from the [SchemaStore](https://www.schemastore.org/) catalog.

## Action inputs

| Input          | Description                                | Default               |
| -------------- | ------------------------------------------ | --------------------- |
| `version`      | `lintel-github-action` version to install  | `latest`              |
| `github-token` | GitHub token for creating Check Runs       | `${{ github.token }}` |
| `paths`        | Space-separated paths or globs to validate | _(auto-discover)_     |
| `exclude`      | Comma-separated exclude patterns           |                       |
| `args`         | Additional arguments passed to the binary  |                       |

## Examples

### Validate specific paths

```yaml
- uses: lintel-rs/action@v0
  with:
    paths: "config/**/*.yaml src/*.json"
```

### Exclude directories

```yaml
- uses: lintel-rs/action@v0
  with:
    exclude: "vendor/**, node_modules/**"
```

### Pin a specific version

```yaml
- uses: lintel-rs/action@v0
  with:
    version: v0.0.9
```

## Configuration

Place a `lintel.toml` in your repository root to configure schema mappings, exclude patterns, and more. See the [Lintel documentation](https://github.com/lintel-rs/lintel) for details.

## How it works

1. Downloads the `lintel-github-action` binary from the [action releases](https://github.com/lintel-rs/action/releases)
2. Runs Lintel validation on your repository files
3. Creates a GitHub Check Run with inline annotations on files with schema violations (batched at 50 per API call)
4. Reports pass/fail with a summary table

## Permissions

The action needs `checks: write` to create Check Runs and `contents: read` to access repository files.

```yaml
permissions:
  checks: write
  contents: read
```

## Environment variables

When running the binary directly (outside the GitHub Action wrapper), these environment variables are required:

| Variable            | Description                                                |
| ------------------- | ---------------------------------------------------------- |
| `GITHUB_TOKEN`      | GitHub token with `checks:write` permission                |
| `GITHUB_REPOSITORY` | Repository in `owner/repo` format                          |
| `GITHUB_SHA`        | Commit SHA to annotate                                     |
| `GITHUB_API_URL`    | GitHub API base URL (defaults to `https://api.github.com`) |

All of these are set automatically when running inside GitHub Actions.

## License

Apache-2.0
