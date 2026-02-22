#![doc = include_str!("../README.md")]

mod path;

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use bpaf::Bpaf;

use lintel_cli_common::{CLIGlobalOptions, CliCacheOptions};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(explain_args_inner))]
pub struct ExplainArgs {
    /// Schema URL or local file path to explain
    #[bpaf(long("schema"), argument("URL|FILE"))]
    pub schema: Option<String>,

    /// Resolve the schema from a data file (like `lintel identify`)
    #[bpaf(long("file"), argument("FILE"))]
    pub file: Option<String>,

    #[bpaf(external(lintel_cli_common::cli_cache_options))]
    pub cache: CliCacheOptions,

    /// Disable syntax highlighting in code blocks
    #[bpaf(long("no-syntax-highlighting"), switch)]
    pub no_syntax_highlighting: bool,

    /// Print output directly instead of piping through a pager
    #[bpaf(long("no-pager"), switch)]
    pub no_pager: bool,

    /// JSON Pointer (`/properties/name`) or `JSONPath` (`$.name`) to a sub-schema
    #[bpaf(positional("PATH"))]
    pub path: Option<String>,
}

/// Construct the bpaf parser for `ExplainArgs`.
pub fn explain_args() -> impl bpaf::Parser<ExplainArgs> {
    explain_args_inner()
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
    if args.schema.is_none() && args.file.is_none() {
        anyhow::bail!("either --schema <URL|FILE> or --file <FILE> is required");
    }

    let (schema_uri, display_name, is_remote) = if let Some(ref file_path) = args.file {
        resolve_from_file(file_path, &args.cache).await?
    } else {
        let uri = args.schema.as_deref().expect("checked above");
        let is_remote = uri.starts_with("http://") || uri.starts_with("https://");
        (uri.to_string(), uri.to_string(), is_remote)
    };

    // Fetch the schema
    let schema_value = fetch_schema(&schema_uri, is_remote, &args.cache).await?;

    // Resolve path to a schema pointer
    let pointer = args
        .path
        .as_deref()
        .map(path::to_schema_pointer)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Run validation when explaining a data file, and collect relevant errors.
    let validation_errors = if args.file.is_some() {
        let file_path = args.file.as_deref().expect("checked above");
        let instance_prefix = pointer
            .as_deref()
            .map(schema_pointer_to_instance_prefix)
            .unwrap_or_default();
        collect_validation_errors(file_path, &args.cache, &instance_prefix).await
    } else {
        vec![]
    };

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

    let output = match pointer.as_deref() {
        Some(ptr) => jsonschema_explain::explain_at_path(&schema_value, ptr, &display_name, &opts)
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        None => jsonschema_explain::explain(&schema_value, &display_name, &opts),
    };

    if is_tty && !args.no_pager {
        lintel_cli_common::pipe_to_pager(&output);
    } else {
        print!("{output}");
    }

    Ok(false)
}

async fn resolve_from_file(
    file_path: &str,
    cache: &CliCacheOptions,
) -> Result<(String, String, bool)> {
    let path = Path::new(file_path);
    if !path.exists() {
        anyhow::bail!("file not found: {file_path}");
    }

    let resolved = lintel_identify::resolve_schema_for_file(path, cache)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no schema found for {file_path}"))?;

    Ok((
        resolved.schema_uri,
        resolved.display_name,
        resolved.is_remote,
    ))
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
) -> Vec<jsonschema_explain::ExplainError> {
    let validate_args = lintel_check::validate::ValidateArgs {
        globs: vec![file_path.to_string()],
        exclude: vec![],
        cache_dir: cache.cache_dir.clone(),
        force_schema_fetch: cache.force_schema_fetch || cache.force,
        force_validation: false,
        no_catalog: cache.no_catalog,
        config_dir: None,
        schema_cache_ttl: cache.schema_cache_ttl,
    };

    let result = match lintel_check::validate::run(&validate_args).await {
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
            if let lintel_check::validate::LintError::Validation {
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
        assert!(args.path.is_none());
        Ok(())
    }

    #[test]
    fn cli_parses_file_with_pointer() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--file", "config.yaml", "/properties/name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file.as_deref(), Some("config.yaml"));
        assert_eq!(args.path.as_deref(), Some("/properties/name"));
        Ok(())
    }

    #[test]
    fn cli_parses_schema_with_jsonpath() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--schema", "schema.json", "$.name"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.schema.as_deref(), Some("schema.json"));
        assert_eq!(args.path.as_deref(), Some("$.name"));
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
}
