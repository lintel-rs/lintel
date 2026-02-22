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
    let pointer = match &args.path {
        Some(p) => Some(path::to_schema_pointer(p).map_err(|e| anyhow::anyhow!("{e}"))?),
        None => None,
    };

    let is_tty = std::io::stdout().is_terminal();
    let use_color = match global.colors {
        Some(lintel_cli_common::ColorsArg::Force) => true,
        Some(lintel_cli_common::ColorsArg::Off) => false,
        None => is_tty,
    };
    let syntax_hl = use_color && !args.no_syntax_highlighting;

    let output = if let Some(ref ptr) = pointer {
        if ptr.is_empty() {
            jsonschema_explain::explain(&schema_value, &display_name, use_color, syntax_hl)
        } else {
            jsonschema_explain::explain_at_path(
                &schema_value,
                ptr,
                &display_name,
                use_color,
                syntax_hl,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?
        }
    } else {
        jsonschema_explain::explain(&schema_value, &display_name, use_color, syntax_hl)
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
