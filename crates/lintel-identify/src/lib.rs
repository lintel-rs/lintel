#![doc = include_str!("../README.md")]

use std::path::Path;

use anyhow::{Context, Result};
use bpaf::{Bpaf, ShellComp};
use lintel_cli_common::{CLIGlobalOptions, CliCacheOptions};

use lintel_explain::resolve::ResolvedFileSchema;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(identify_args_inner))]
#[allow(clippy::struct_excessive_bools)]
pub struct IdentifyArgs {
    /// Show detailed schema documentation
    #[bpaf(long("explain"), switch)]
    pub explain: bool,

    #[bpaf(external(lintel_cli_common::cli_cache_options))]
    pub cache: CliCacheOptions,

    /// Disable syntax highlighting in code blocks
    #[bpaf(long("no-syntax-highlighting"), switch)]
    pub no_syntax_highlighting: bool,

    /// Print output directly instead of piping through a pager
    #[bpaf(long("no-pager"), switch)]
    pub no_pager: bool,

    /// Show extended details like $comment annotations
    #[bpaf(long("extended"), switch)]
    pub extended: bool,

    /// File to identify
    #[bpaf(positional("FILE"), complete_shell(ShellComp::File { mask: None }))]
    pub file: String,
}

/// Construct the bpaf parser for `IdentifyArgs`.
pub fn identify_args() -> impl bpaf::Parser<IdentifyArgs> {
    identify_args_inner()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
pub async fn run(args: IdentifyArgs, global: &CLIGlobalOptions) -> Result<bool> {
    let file_path = Path::new(&args.file);
    if !file_path.exists() {
        anyhow::bail!("file not found: {}", args.file);
    }

    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("failed to read {}", args.file))?;

    let path_str = file_path.display().to_string();

    let Some(resolved) =
        lintel_explain::resolve::resolve_schema_for_content(&content, file_path, None, &args.cache)
            .await?
    else {
        eprintln!("{path_str}");
        eprintln!("  no schema found");
        return Ok(false);
    };

    print_identification(&path_str, &resolved);

    if args.explain {
        lintel_explain::explain_resolved_schema(
            &resolved,
            &args.cache,
            global,
            &lintel_explain::ExplainDisplayArgs {
                no_syntax_highlighting: args.no_syntax_highlighting,
                no_pager: args.no_pager,
                extended: args.extended,
            },
        )
        .await?;
    }

    Ok(false)
}

/// Print the identification summary to stdout.
fn print_identification(path_str: &str, resolved: &ResolvedFileSchema) {
    let schema_uri = &resolved.schema_uri;
    let display_name = &resolved.display_name;

    println!("{path_str}");
    if display_name == schema_uri {
        println!("  schema: {schema_uri}");
    } else {
        println!("  schema: {display_name} ({schema_uri})");
    }
    println!("  source: {}", resolved.source);

    if let Some(pattern) = &resolved.matched_pattern {
        println!("  matched: {pattern}");
    }
    if resolved.file_match.len() > 1 {
        let globs = resolved
            .file_match
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  globs: {globs}");
    }
    if let Some(desc) = &resolved.description {
        println!("  description: {desc}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bpaf::Parser;
    use lintel_cli_common::cli_global_options;

    // Helper to build the CLI parser matching the binary's structure.
    fn test_cli() -> bpaf::OptionParser<(CLIGlobalOptions, IdentifyArgs)> {
        bpaf::construct!(cli_global_options(), identify_args())
            .to_options()
            .descr("test identify args")
    }

    #[test]
    fn cli_parses_identify_basic() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["file.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file, "file.json");
        assert!(!args.explain);
        assert!(!args.cache.no_catalog);
        assert!(!args.cache.force_schema_fetch);
        assert!(args.cache.cache_dir.is_none());
        assert!(args.cache.schema_cache_ttl.is_none());
        Ok(())
    }

    #[test]
    fn cli_parses_identify_explain() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["file.json", "--explain"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file, "file.json");
        assert!(args.explain);
        Ok(())
    }

    #[test]
    fn cli_parses_identify_no_catalog() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&["--no-catalog", "file.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file, "file.json");
        assert!(args.cache.no_catalog);
        Ok(())
    }

    #[test]
    fn cli_parses_identify_all_options() -> anyhow::Result<()> {
        let (_, args) = test_cli()
            .run_inner(&[
                "--explain",
                "--no-catalog",
                "--force-schema-fetch",
                "--cache-dir",
                "/tmp/cache",
                "--schema-cache-ttl",
                "30m",
                "tsconfig.json",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert_eq!(args.file, "tsconfig.json");
        assert!(args.explain);
        assert!(args.cache.no_catalog);
        assert!(args.cache.force_schema_fetch);
        assert_eq!(args.cache.cache_dir.as_deref(), Some("/tmp/cache"));
        assert_eq!(
            args.cache.schema_cache_ttl,
            Some(core::time::Duration::from_secs(30 * 60))
        );
        Ok(())
    }
}
