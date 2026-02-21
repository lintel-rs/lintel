# lintel-github-action

[![License][license-badge]][license-url]

[license-badge]: https://img.shields.io/crates/l/lintel-github-action.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

GitHub Action binary for [Lintel](https://github.com/lintel-rs/lintel). Creates a GitHub Check Run named "Lintel" with inline annotations on pull requests using the [Checks API](https://docs.github.com/en/rest/checks/runs).

## How it works

1. Runs Lintel validation on the specified files
2. Collects errors with file path, line, and column information
3. Creates a GitHub Check Run with annotations (batched at 50 per API call)
4. Reports success/failure with a summary table

## Environment variables

| Variable            | Description                                                                   |
| ------------------- | ----------------------------------------------------------------------------- |
| `GITHUB_TOKEN`      | GitHub token with `checks:write` permission (required)                        |
| `GITHUB_REPOSITORY` | Repository in `owner/repo` format (set automatically by GitHub Actions)       |
| `GITHUB_SHA`        | Commit SHA to annotate (set automatically by GitHub Actions)                  |
| `GITHUB_API_URL`    | GitHub API base URL (set automatically, defaults to `https://api.github.com`) |

## Usage

This binary is used by the [lintel-rs/action](https://github.com/lintel-rs/action) GitHub Action. See that repository for usage instructions.
