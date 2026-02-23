#![doc = include_str!("../README.md")]

mod path;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bpaf::Bpaf;

use lintel_cli_common::{CLIGlobalOptions, CliCacheOptions};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(explain_args_inner))]
pub struct ExplainArgs {
    /// Schema URL or local file path to explain.
    /// Can be combined with `--file` or `--path` to override schema resolution
    /// while still validating the data file.
    #[bpaf(long("schema"), argument("URL|FILE"))]
    pub schema: Option<String>,

    /// Data file (local path or URL) to resolve the schema from and validate.
    /// The file must exist (or be fetchable). For URLs, the filename is used
    /// for catalog matching.
    #[bpaf(long("file"), argument("FILE|URL"))]
    pub file: Option<String>,

    /// File path or URL to resolve the schema from using catalogs.
    /// Local files need not exist; if the file exists (or is a URL), it is
    /// also validated.
    #[bpaf(long("path"), argument("FILE|URL"))]
    pub resolve_path: Option<String>,

    #[bpaf(external(lintel_cli_common::cli_cache_options))]
    pub cache: CliCacheOptions,

    /// Disable syntax highlighting in code blocks
    #[bpaf(long("no-syntax-highlighting"), switch)]
    pub no_syntax_highlighting: bool,

    /// Print output directly instead of piping through a pager
    #[bpaf(long("no-pager"), switch)]
    pub no_pager: bool,

    /// First positional argument. When no `--file`, `--path`, or `--schema`
    /// flag is given this is treated as a file path (equivalent to `--path`).
    /// Otherwise it is a JSON Pointer or `JSONPath` to a sub-schema.
    ///
    /// Examples:
    /// - `lintel explain package.json`       — explains the schema for `package.json`
    /// - `lintel explain --file f.yaml name`  — explains the `name` property
    #[bpaf(positional("FILE|POINTER"))]
    pub positional: Option<String>,

    /// JSON Pointer (`/properties/name`) or `JSONPath` (`$.name`) to a sub-schema.
    /// Only used when the first positional is a file path.
    ///
    /// Example: `lintel explain package.json name`
    #[bpaf(positional("POINTER"))]
    pub pointer: Option<String>,
}

/// Construct the bpaf parser for `ExplainArgs`.
pub fn explain_args() -> impl bpaf::Parser<ExplainArgs> {
    explain_args_inner()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Extract the last path segment from a URL, e.g. "package.json".
fn url_filename(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|seg| {
            // Strip query string / fragment
            let seg = seg.split('?').next().unwrap_or(seg);
            let seg = seg.split('#').next().unwrap_or(seg);
            if seg.is_empty() {
                None
            } else {
                Some(seg.to_string())
            }
        })
        .unwrap_or_else(|| "file".to_string())
}

/// Fetch URL content via HTTP GET.
async fn fetch_url_content(url: &str) -> Result<String> {
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("failed to fetch URL: {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} fetching {url}");
    }
    resp.text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))
}

/// Data fetched from a remote URL.
struct FetchedData {
    content: String,
    filename: String,
}

/// Temporary file used to give the validation pipeline a real file path with
/// the correct filename for catalog matching. Cleaned up on drop.
struct TempDataFile {
    _dir: tempfile::TempDir,
    file_path: PathBuf,
}

impl TempDataFile {
    fn new(filename: &str, content: &str) -> Result<Self> {
        let dir = tempfile::tempdir().context("failed to create temp directory")?;
        let file_path = dir.path().join(filename);
        std::fs::write(&file_path, content)
            .with_context(|| format!("failed to write temp file: {}", file_path.display()))?;
        Ok(Self {
            _dir: dir,
            file_path,
        })
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the explain command.
///
/// # Errors
///
/// Returns an error if the schema cannot be fetched, parsed, or the pointer
/// cannot be resolved.
#[allow(clippy::missing_panics_doc)]
pub async fn run(args: ExplainArgs, global: &CLIGlobalOptions) -> Result<bool> {
    // Normalize positional args: when no flag is given, the first positional
    // is a file path (equivalent to --path) and the second is the pointer.
    let has_flag = args.file.is_some() || args.resolve_path.is_some() || args.schema.is_some();
    let mut args = args;
    let pointer_str = if has_flag {
        // Flags present: first positional is the pointer, second is invalid.
        if args.pointer.is_some() {
            anyhow::bail!("unexpected extra positional argument");
        }
        args.positional.take()
    } else if args.positional.is_some() {
        // No flags: first positional is the file path.
        args.resolve_path = args.positional.take();
        args.pointer.take()
    } else {
        anyhow::bail!(
            "a file path or one of --file <FILE>, --path <FILE>, --schema <URL|FILE> is required"
        );
    };

    let data_source_str = args.file.as_deref().or(args.resolve_path.as_deref());
    let is_file_flag = args.file.is_some();

    let fetched = fetch_data_source(data_source_str).await?;

    let (schema_uri, display_name, is_remote) =
        resolve_schema_info(&args, data_source_str, is_file_flag, fetched.as_ref()).await?;

    let schema_value = fetch_schema(&schema_uri, is_remote, &args.cache).await?;

    let pointer = pointer_str
        .as_deref()
        .map(path::to_schema_pointer)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let instance_prefix = pointer
        .as_deref()
        .map(schema_pointer_to_instance_prefix)
        .unwrap_or_default();

    let validation_errors = run_validation(
        fetched.as_ref(),
        data_source_str,
        &args.cache,
        &instance_prefix,
    )
    .await?;

    render_output(
        global,
        &args,
        &schema_value,
        &display_name,
        pointer.as_deref(),
        validation_errors,
    )
}

/// If the data source is a URL, fetch its content; otherwise return `None`.
async fn fetch_data_source(data_source_str: Option<&str>) -> Result<Option<FetchedData>> {
    let Some(src) = data_source_str else {
        return Ok(None);
    };
    if !is_url(src) {
        return Ok(None);
    }
    let content = fetch_url_content(src).await?;
    let filename = url_filename(src);
    Ok(Some(FetchedData { content, filename }))
}

/// Determine the schema URI, display name, and whether it's remote.
async fn resolve_schema_info(
    args: &ExplainArgs,
    data_source_str: Option<&str>,
    is_file_flag: bool,
    fetched: Option<&FetchedData>,
) -> Result<(String, String, bool)> {
    if let Some(ref schema) = args.schema {
        let is_remote = is_url(schema);
        if !is_remote && !is_url(data_source_str.unwrap_or("")) {
            let resolved = data_source_str
                .map(Path::new)
                .and_then(|p| p.parent())
                .map_or_else(
                    || schema.clone(),
                    |parent| parent.join(schema).to_string_lossy().to_string(),
                );
            Ok((resolved.clone(), resolved, false))
        } else {
            Ok((schema.clone(), schema.clone(), is_remote))
        }
    } else if let Some(fetched) = fetched {
        let cwd = std::env::current_dir().ok();
        let virtual_path = PathBuf::from(&fetched.filename);
        let resolved = lintel_identify::resolve_schema_for_content(
            &fetched.content,
            &virtual_path,
            cwd.as_deref(),
            &args.cache,
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("no schema found for URL: {}", data_source_str.unwrap_or(""))
        })?;
        Ok((
            resolved.schema_uri,
            resolved.display_name,
            resolved.is_remote,
        ))
    } else if let Some(src) = data_source_str {
        resolve_local_schema(src, is_file_flag, &args.cache).await
    } else {
        unreachable!("at least --schema is set (checked above)")
    }
}

/// Resolve schema from a local file or path.
async fn resolve_local_schema(
    src: &str,
    is_file_flag: bool,
    cache: &CliCacheOptions,
) -> Result<(String, String, bool)> {
    let path = Path::new(src);
    if path.exists() {
        let resolved = lintel_identify::resolve_schema_for_file(path, cache)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no schema found for {src}"))?;
        Ok((
            resolved.schema_uri,
            resolved.display_name,
            resolved.is_remote,
        ))
    } else if is_file_flag {
        anyhow::bail!("file not found: {src}");
    } else {
        let resolved = lintel_identify::resolve_schema_for_path(path, cache)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no schema found for path: {src}"))?;
        Ok((
            resolved.schema_uri,
            resolved.display_name,
            resolved.is_remote,
        ))
    }
}

/// Collect validation errors from the data source if available.
async fn run_validation(
    fetched: Option<&FetchedData>,
    data_source_str: Option<&str>,
    cache: &CliCacheOptions,
    instance_prefix: &str,
) -> Result<Vec<jsonschema_explain::ExplainError>> {
    if let Some(fetched) = fetched {
        let temp = TempDataFile::new(&fetched.filename, &fetched.content)?;
        let config_dir = std::env::current_dir().ok();
        Ok(collect_validation_errors(
            &temp.file_path.to_string_lossy(),
            cache,
            instance_prefix,
            config_dir,
        )
        .await)
    } else if let Some(src) = data_source_str {
        if Path::new(src).exists() {
            Ok(collect_validation_errors(src, cache, instance_prefix, None).await)
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

/// Render the schema explanation output.
#[allow(clippy::too_many_arguments)]
fn render_output(
    global: &CLIGlobalOptions,
    args: &ExplainArgs,
    schema_value: &serde_json::Value,
    display_name: &str,
    pointer: Option<&str>,
    validation_errors: Vec<jsonschema_explain::ExplainError>,
) -> Result<bool> {
    let is_tty = std::io::stdout().is_terminal();
    let use_color = match global.colors {
        Some(lintel_cli_common::ColorsArg::Force) => true,
        Some(lintel_cli_common::ColorsArg::Off) => false,
        None => is_tty,
    };
    let opts = jsonschema_explain::ExplainOptions {
        color: use_color,
        syntax_highlight: use_color && !args.no_syntax_highlighting,
        width: terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .or_else(|| std::env::var("COLUMNS").ok()?.parse().ok())
            .unwrap_or(80),
        validation_errors,
    };

    let output = match pointer {
        Some(ptr) => jsonschema_explain::explain_at_path(schema_value, ptr, display_name, &opts)
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        None => jsonschema_explain::explain(schema_value, display_name, &opts),
    };

    if is_tty && !args.no_pager {
        lintel_cli_common::pipe_to_pager(&output);
    } else {
        print!("{output}");
    }

    Ok(false)
}

async fn fetch_schema(
    schema_uri: &str,
    is_remote: bool,
    cache: &CliCacheOptions,
) -> Result<serde_json::Value> {
    if is_remote {
        let retriever = lintel_identify::build_retriever(cache);
        let (val, _) = retriever
            .fetch(schema_uri)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch schema '{schema_uri}': {e}"))?;
        Ok(val)
    } else {
        let content = std::fs::read_to_string(schema_uri)
            .with_context(|| format!("failed to read schema: {schema_uri}"))?;
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse schema: {schema_uri}"))
    }
}

/// Convert a schema pointer (e.g. `/properties/badges`) to an instance path
/// prefix (e.g. `/badges`) by stripping `/properties/` segments.
fn schema_pointer_to_instance_prefix(schema_pointer: &str) -> String {
    let mut result = String::new();
    let mut segments = schema_pointer.split('/').peekable();
    // Skip the leading empty segment from the leading `/`.
    segments.next();
    while let Some(seg) = segments.next() {
        if seg == "properties" {
            // The next segment is the actual property name.
            if let Some(prop) = segments.next() {
                result.push('/');
                result.push_str(prop);
            }
        } else if seg == "items" {
            // Array items — keep descending but don't add to the prefix.
        } else {
            result.push('/');
            result.push_str(seg);
        }
    }
    result
}

/// Run validation on a data file and return errors filtered to a given
/// instance path prefix.
async fn collect_validation_errors(
    file_path: &str,
    cache: &CliCacheOptions,
    instance_prefix: &str,
    config_dir: Option<PathBuf>,
) -> Vec<jsonschema_explain::ExplainError> {
    let validate_args = lintel_validate::validate::ValidateArgs {
        globs: vec![file_path.to_string()],
        exclude: vec![],
        cache_dir: cache.cache_dir.clone(),
        force_schema_fetch: cache.force_schema_fetch || cache.force,
        force_validation: false,
        no_catalog: cache.no_catalog,
        config_dir,
        schema_cache_ttl: cache.schema_cache_ttl,
    };

    let result = match lintel_validate::validate::run(&validate_args).await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("validation failed: {e}");
            return vec![];
        }
    };

    result
        .errors
        .into_iter()
        .filter_map(|err| {
            if let lintel_validate::validate::LintError::Validation {
                instance_path,
                message,
                ..
            } = err
            {
                // When explaining the root, show all errors.
                // Otherwise only show errors under the given property.
                if instance_prefix.is_empty()
                    || instance_path == instance_prefix
                    || instance_path.starts_with(&format!("{instance_prefix}/"))
                {
                    Some(jsonschema_explain::ExplainError {
                        instance_path,
                        message,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bpaf::Parser;
    use lintel_cli_common::cli_global_options;

    fn test_cli() -> bpaf::OptionParser<(CLIGlobalOptions, ExplainArgs)> {
        bpaf::construct!(cli_global_options(), explain_args())
            .to_options()
            .descr("test explain args")
    }

    #[test]
    fn cli_parses_schema_only() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--schema", "https://example.com/schema.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(
            args.schema.as_deref(),
            Some("https://example.com/schema.json")
        );
        assert!(args.file.is_none());
        assert!(args.positional.is_none());
        Ok(())
    }

    #[test]
    fn cli_parses_file_with_pointer() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--file", "config.yaml", "/properties/name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file.as_deref(), Some("config.yaml"));
        assert_eq!(args.positional.as_deref(), Some("/properties/name"));
        Ok(())
    }

    #[test]
    fn cli_parses_schema_with_jsonpath() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--schema", "schema.json", "$.name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.schema.as_deref(), Some("schema.json"));
        assert_eq!(args.positional.as_deref(), Some("$.name"));
        Ok(())
    }

    #[test]
    fn cli_parses_display_options() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&[
                "--schema",
                "s.json",
                "--no-syntax-highlighting",
                "--no-pager",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert!(args.no_syntax_highlighting);
        assert!(args.no_pager);
        Ok(())
    }

    #[test]
    fn cli_parses_path_only() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--path", "tsconfig.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.resolve_path.as_deref(), Some("tsconfig.json"));
        assert!(args.file.is_none());
        assert!(args.schema.is_none());
        assert!(args.positional.is_none());
        Ok(())
    }

    #[test]
    fn cli_parses_path_with_pointer() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--path", "config.yaml", "/properties/name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.resolve_path.as_deref(), Some("config.yaml"));
        assert_eq!(args.positional.as_deref(), Some("/properties/name"));
        Ok(())
    }

    #[test]
    fn cli_parses_path_with_jsonpath() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--path", "config.yaml", "$.name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.resolve_path.as_deref(), Some("config.yaml"));
        assert_eq!(args.positional.as_deref(), Some("$.name"));
        Ok(())
    }

    #[test]
    fn cli_file_takes_precedence_over_path() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--file", "data.yaml", "--path", "other.yaml"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file.as_deref(), Some("data.yaml"));
        assert_eq!(args.resolve_path.as_deref(), Some("other.yaml"));
        // Both are parsed — precedence is enforced at runtime in run()
        Ok(())
    }

    #[test]
    fn cli_path_takes_precedence_over_schema() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--path", "config.yaml", "--schema", "s.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.resolve_path.as_deref(), Some("config.yaml"));
        assert_eq!(args.schema.as_deref(), Some("s.json"));
        // Both are parsed — precedence is enforced at runtime in run()
        Ok(())
    }

    #[test]
    fn cli_schema_with_file() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--schema", "s.json", "--file", "data.yaml"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.schema.as_deref(), Some("s.json"));
        assert_eq!(args.file.as_deref(), Some("data.yaml"));
        Ok(())
    }

    #[test]
    fn cli_schema_with_path() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--schema", "s.json", "--path", "data.yaml"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.schema.as_deref(), Some("s.json"));
        assert_eq!(args.resolve_path.as_deref(), Some("data.yaml"));
        Ok(())
    }

    #[tokio::test]
    async fn run_rejects_no_source() {
        let args = ExplainArgs {
            schema: None,
            file: None,
            resolve_path: None,
            cache: CliCacheOptions {
                cache_dir: None,
                schema_cache_ttl: None,
                force_schema_fetch: false,
                force_validation: false,
                force: false,
                no_catalog: false,
            },
            no_syntax_highlighting: false,
            no_pager: false,
            positional: None,
            pointer: None,
        };
        let global = CLIGlobalOptions {
            colors: None,
            verbose: false,
            log_level: lintel_cli_common::LogLevel::None,
        };
        let err = run(args, &global).await.unwrap_err();
        assert!(
            err.to_string().contains("a file path or one of --file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn cli_parses_cache_options() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&[
                "--schema",
                "s.json",
                "--cache-dir",
                "/tmp/cache",
                "--no-catalog",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.cache.cache_dir.as_deref(), Some("/tmp/cache"));
        assert!(args.cache.no_catalog);
        Ok(())
    }

    // --- positional-only usage ---

    #[test]
    fn cli_positional_file_only() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["package.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.positional.as_deref(), Some("package.json"));
        assert!(args.pointer.is_none());
        assert!(args.file.is_none());
        assert!(args.resolve_path.is_none());
        assert!(args.schema.is_none());
        Ok(())
    }

    #[test]
    fn cli_positional_file_with_pointer() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["package.json", "name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.positional.as_deref(), Some("package.json"));
        assert_eq!(args.pointer.as_deref(), Some("name"));
        assert!(args.file.is_none());
        assert!(args.resolve_path.is_none());
        assert!(args.schema.is_none());
        Ok(())
    }

    #[test]
    fn cli_positional_file_with_json_pointer() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["config.yaml", "/properties/name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.positional.as_deref(), Some("config.yaml"));
        assert_eq!(args.pointer.as_deref(), Some("/properties/name"));
        Ok(())
    }

    // --- URL filename extraction ---

    #[test]
    fn url_filename_simple() {
        assert_eq!(
            url_filename("https://example.com/package.json"),
            "package.json"
        );
    }

    #[test]
    fn url_filename_with_query() {
        assert_eq!(
            url_filename("https://example.com/config.yaml?ref=main"),
            "config.yaml"
        );
    }

    #[test]
    fn url_filename_with_fragment() {
        assert_eq!(
            url_filename("https://example.com/config.yaml#section"),
            "config.yaml"
        );
    }

    #[test]
    fn url_filename_nested_path() {
        assert_eq!(
            url_filename(
                "https://raw.githubusercontent.com/org/repo/main/.github/workflows/ci.yml"
            ),
            "ci.yml"
        );
    }

    #[test]
    fn url_filename_trailing_slash() {
        assert_eq!(url_filename("https://example.com/"), "file");
    }

    // --- is_url ---

    #[test]
    fn is_url_detects_https() {
        assert!(is_url("https://example.com/schema.json"));
    }

    #[test]
    fn is_url_detects_http() {
        assert!(is_url("http://example.com/schema.json"));
    }

    #[test]
    fn is_url_rejects_local() {
        assert!(!is_url("./schema.json"));
        assert!(!is_url("/tmp/schema.json"));
        assert!(!is_url("schema.json"));
    }
}
