#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bpaf::Bpaf;
use lintel_cli_common::{CLIGlobalOptions, CliCacheOptions};

use lintel_schema_cache::SchemaCache;
use lintel_validate::parsers;
use lintel_validate::validate;
use schema_catalog::SchemaMatch;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(identify_args_inner))]
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

    /// File to identify
    #[bpaf(positional("FILE"))]
    pub file: String,
}

/// Construct the bpaf parser for `IdentifyArgs`.
pub fn identify_args() -> impl bpaf::Parser<IdentifyArgs> {
    identify_args_inner()
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// The source that resolved the schema URI for a file.
#[derive(Debug)]
enum SchemaSource {
    Inline,
    Config,
    Catalog,
}

impl core::fmt::Display for SchemaSource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SchemaSource::Inline => write!(f, "inline"),
            SchemaSource::Config => write!(f, "config"),
            SchemaSource::Catalog => write!(f, "catalog"),
        }
    }
}

/// Match details captured during schema resolution.
struct ResolvedSchema<'a> {
    uri: String,
    source: SchemaSource,
    /// Present only for catalog matches.
    catalog_match: Option<CatalogMatchInfo<'a>>,
    /// Present only for config matches.
    config_pattern: Option<&'a str>,
}

/// Details from a catalog match, borrowed from the `CompiledCatalog`.
struct CatalogMatchInfo<'a> {
    matched_pattern: &'a str,
    file_match: &'a [String],
    name: &'a str,
    description: Option<&'a str>,
}

impl<'a> From<SchemaMatch<'a>> for CatalogMatchInfo<'a> {
    fn from(m: SchemaMatch<'a>) -> Self {
        Self {
            matched_pattern: m.matched_pattern,
            file_match: m.file_match,
            name: m.name,
            description: m.description,
        }
    }
}

// ---------------------------------------------------------------------------
// Resolved file schema — reusable by `lintel explain`
// ---------------------------------------------------------------------------

/// Result of resolving a schema for a given file path.
pub struct ResolvedFileSchema {
    /// The final schema URI (after rewrites and path resolution).
    pub schema_uri: String,
    /// A human-readable name (from catalog or URI).
    pub display_name: String,
    /// Whether the schema is a remote URL.
    pub is_remote: bool,
}

/// Build a [`SchemaCache`] from [`CliCacheOptions`].
pub fn build_retriever(cache: &CliCacheOptions) -> SchemaCache {
    let mut builder = SchemaCache::builder().force_fetch(cache.force_schema_fetch || cache.force);
    if let Some(dir) = &cache.cache_dir {
        builder = builder.cache_dir(PathBuf::from(dir));
    }
    if let Some(ttl) = cache.schema_cache_ttl {
        builder = builder.ttl(ttl);
    }
    builder.build()
}

/// Resolve the schema URI for a file path using the same priority as validation:
/// 1. Inline `$schema` / YAML modeline
/// 2. Custom schema mappings from `lintel.toml [schemas]`
/// 3. Catalog matching
///
/// # Errors
///
/// Returns an error if the file cannot be read.
#[allow(clippy::missing_panics_doc)]
pub async fn resolve_schema_for_file(
    file_path: &Path,
    cache: &CliCacheOptions,
) -> Result<Option<ResolvedFileSchema>> {
    let path_str = file_path.display().to_string();
    let content =
        std::fs::read_to_string(file_path).with_context(|| format!("failed to read {path_str}"))?;

    resolve_schema_for_content(&content, file_path, None, cache).await
}

/// Resolve a schema from in-memory content and a virtual file path.
///
/// Uses `file_path` for extension detection and catalog matching, and
/// `config_search_dir` for locating `lintel.toml` (falls back to
/// `file_path.parent()` when `None`).
///
/// Resolution order: inline `$schema` > config > catalogs.
///
/// # Errors
///
/// Returns an error if catalogs cannot be fetched.
#[allow(clippy::missing_panics_doc)]
pub async fn resolve_schema_for_content(
    content: &str,
    file_path: &Path,
    config_search_dir: Option<&Path>,
    cache: &CliCacheOptions,
) -> Result<Option<ResolvedFileSchema>> {
    let path_str = file_path.display().to_string();
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let retriever = build_retriever(cache);

    let search_dir = config_search_dir
        .map(Path::to_path_buf)
        .or_else(|| file_path.parent().map(Path::to_path_buf));
    let (cfg, config_dir, _config_path) = validate::load_config(search_dir.as_deref());

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &cfg, cache.no_catalog).await;

    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, content, &path_str);

    let Some(resolved) = resolve_schema(
        parser.as_ref(),
        content,
        &instance,
        &path_str,
        file_name,
        &cfg,
        &compiled_catalogs,
    ) else {
        return Ok(None);
    };

    let from_inline = matches!(resolved.source, SchemaSource::Inline);
    let (schema_uri, is_remote) = finalize_uri(
        &resolved.uri,
        &cfg.rewrite,
        &config_dir,
        file_path,
        from_inline,
    );

    let display_name = resolved
        .catalog_match
        .as_ref()
        .map(|m| m.name.to_string())
        .or_else(|| {
            compiled_catalogs
                .iter()
                .find_map(|cat| cat.schema_name(&schema_uri))
                .map(str::to_string)
        })
        .unwrap_or_else(|| schema_uri.clone());

    Ok(Some(ResolvedFileSchema {
        schema_uri,
        display_name,
        is_remote,
    }))
}

/// Resolve the schema URI for a file path using only path-based matching:
/// 1. Custom schema mappings from `lintel.toml [schemas]`
/// 2. Catalog matching
///
/// Unlike [`resolve_schema_for_file`], this does NOT read the file or check
/// for inline `$schema` directives. The file does not need to exist.
///
/// # Errors
///
/// Returns an error if the catalogs cannot be fetched.
#[allow(clippy::missing_panics_doc)]
pub async fn resolve_schema_for_path(
    file_path: &Path,
    cache: &CliCacheOptions,
) -> Result<Option<ResolvedFileSchema>> {
    let path_str = file_path.display().to_string();
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let retriever = build_retriever(cache);

    let config_search_dir = file_path.parent().map(Path::to_path_buf);
    let (cfg, config_dir, _config_path) = validate::load_config(config_search_dir.as_deref());

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &cfg, cache.no_catalog).await;

    let Some(resolved) = resolve_schema_path_only(&path_str, file_name, &cfg, &compiled_catalogs)
    else {
        return Ok(None);
    };

    let from_inline = matches!(resolved.source, SchemaSource::Inline);
    let (schema_uri, is_remote) = finalize_uri(
        &resolved.uri,
        &cfg.rewrite,
        &config_dir,
        file_path,
        from_inline,
    );

    let display_name = resolved
        .catalog_match
        .as_ref()
        .map(|m| m.name.to_string())
        .or_else(|| {
            compiled_catalogs
                .iter()
                .find_map(|cat| cat.schema_name(&schema_uri))
                .map(str::to_string)
        })
        .unwrap_or_else(|| schema_uri.clone());

    Ok(Some(ResolvedFileSchema {
        schema_uri,
        display_name,
        is_remote,
    }))
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
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let retriever = build_retriever(&args.cache);

    let config_search_dir = file_path.parent().map(Path::to_path_buf);
    let (cfg, config_dir, _config_path) = validate::load_config(config_search_dir.as_deref());

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &cfg, args.cache.no_catalog).await;

    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, &content, &path_str);

    let Some(resolved) = resolve_schema(
        parser.as_ref(),
        &content,
        &instance,
        &path_str,
        file_name,
        &cfg,
        &compiled_catalogs,
    ) else {
        eprintln!("{path_str}");
        eprintln!("  no schema found");
        return Ok(false);
    };

    let from_inline = matches!(resolved.source, SchemaSource::Inline);
    let (schema_uri, is_remote) = finalize_uri(
        &resolved.uri,
        &cfg.rewrite,
        &config_dir,
        file_path,
        from_inline,
    );

    let display_name = resolved
        .catalog_match
        .as_ref()
        .map(|m| m.name)
        .or_else(|| {
            compiled_catalogs
                .iter()
                .find_map(|cat| cat.schema_name(&schema_uri))
        })
        .unwrap_or(&schema_uri);

    print_identification(&path_str, &schema_uri, display_name, &resolved);

    if args.explain {
        run_explain(
            &args,
            global,
            &schema_uri,
            display_name,
            is_remote,
            &retriever,
        )
        .await?;
    }

    Ok(false)
}

/// Try each resolution source in priority order, returning `None` if no schema is found.
#[allow(clippy::too_many_arguments)]
fn resolve_schema<'a>(
    parser: &dyn parsers::Parser,
    content: &str,
    instance: &serde_json::Value,
    path_str: &str,
    file_name: &'a str,
    cfg: &'a lintel_config::Config,
    catalogs: &'a [schema_catalog::CompiledCatalog],
) -> Option<ResolvedSchema<'a>> {
    if let Some(uri) = parser.extract_schema_uri(content, instance) {
        return Some(ResolvedSchema {
            uri,
            source: SchemaSource::Inline,
            catalog_match: None,
            config_pattern: None,
        });
    }

    resolve_schema_path_only(path_str, file_name, cfg, catalogs)
}

/// Try config mappings and catalog matching only (no inline `$schema`).
fn resolve_schema_path_only<'a>(
    path_str: &str,
    file_name: &'a str,
    cfg: &'a lintel_config::Config,
    catalogs: &'a [schema_catalog::CompiledCatalog],
) -> Option<ResolvedSchema<'a>> {
    if let Some((pattern, url)) = cfg
        .schemas
        .iter()
        .find(|(pattern, _)| {
            let p = path_str.strip_prefix("./").unwrap_or(path_str);
            glob_match::glob_match(pattern, p) || glob_match::glob_match(pattern, file_name)
        })
        .map(|(pattern, url)| (pattern.as_str(), url.as_str()))
    {
        return Some(ResolvedSchema {
            uri: url.to_string(),
            source: SchemaSource::Config,
            catalog_match: None,
            config_pattern: Some(pattern),
        });
    }

    catalogs
        .iter()
        .find_map(|cat| cat.find_schema_detailed(path_str, file_name))
        .map(|schema_match| ResolvedSchema {
            uri: schema_match.url.to_string(),
            source: SchemaSource::Catalog,
            catalog_match: Some(schema_match.into()),
            config_pattern: None,
        })
}

/// Apply rewrites, resolve relative paths, and determine whether the URI is remote.
///
/// When `from_inline` is true, relative paths resolve against the file's parent
/// directory (inline `$schema`). Otherwise they resolve against the config
/// directory where `lintel.toml` lives.
#[allow(clippy::too_many_arguments)]
fn finalize_uri(
    raw_uri: &str,
    rewrites: &HashMap<String, String>,
    config_dir: &Path,
    file_path: &Path,
    from_inline: bool,
) -> (String, bool) {
    let schema_uri = lintel_config::apply_rewrites(raw_uri, rewrites);
    let schema_uri = lintel_config::resolve_double_slash(&schema_uri, config_dir);

    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
    let schema_uri = if is_remote {
        schema_uri
    } else {
        let base_dir = if from_inline {
            file_path.parent()
        } else {
            Some(config_dir)
        };
        base_dir
            .map(|dir| dir.join(&schema_uri).to_string_lossy().to_string())
            .unwrap_or(schema_uri)
    };

    (schema_uri, is_remote)
}

/// Print the identification summary to stdout.
fn print_identification(
    path_str: &str,
    schema_uri: &str,
    display_name: &str,
    resolved: &ResolvedSchema<'_>,
) {
    println!("{path_str}");
    if display_name == schema_uri {
        println!("  schema: {schema_uri}");
    } else {
        println!("  schema: {display_name} ({schema_uri})");
    }
    println!("  source: {}", resolved.source);

    match &resolved.source {
        SchemaSource::Inline => {}
        SchemaSource::Config => {
            if let Some(pattern) = resolved.config_pattern {
                println!("  matched: {pattern}");
            }
        }
        SchemaSource::Catalog => {
            if let Some(ref m) = resolved.catalog_match {
                println!("  matched: {}", m.matched_pattern);
                if m.file_match.len() > 1 {
                    let globs = m
                        .file_match
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("  globs: {globs}");
                }
                if let Some(desc) = m.description {
                    println!("  description: {desc}");
                }
            }
        }
    }
}

/// Fetch the schema and render its documentation.
#[allow(clippy::too_many_arguments)]
async fn run_explain(
    args: &IdentifyArgs,
    global: &CLIGlobalOptions,
    schema_uri: &str,
    display_name: &str,
    is_remote: bool,
    retriever: &SchemaCache,
) -> Result<()> {
    let schema_value = if is_remote {
        match retriever.fetch(schema_uri).await {
            Ok((val, _)) => val,
            Err(e) => {
                eprintln!("  error fetching schema: {e}");
                return Ok(());
            }
        }
    } else {
        let schema_content = std::fs::read_to_string(schema_uri)
            .with_context(|| format!("failed to read schema: {schema_uri}"))?;
        serde_json::from_str(&schema_content)
            .with_context(|| format!("failed to parse schema: {schema_uri}"))?
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
        validation_errors: vec![],
    };
    let output = jsonschema_explain::explain(&schema_value, display_name, &opts);

    if is_tty && !args.no_pager {
        lintel_cli_common::pipe_to_pager(&format!("\n{output}"));
    } else {
        println!();
        print!("{output}");
    }
    Ok(())
}

/// Parse the file content, trying the detected format first, then all parsers as fallback.
///
/// Exits the process when the file cannot be parsed.
fn parse_file(
    detected_format: Option<parsers::FileFormat>,
    content: &str,
    path_str: &str,
) -> (Box<dyn parsers::Parser>, serde_json::Value) {
    if let Some(fmt) = detected_format {
        let parser = parsers::parser_for(fmt);
        if let Ok(val) = parser.parse(content, path_str) {
            return (parser, val);
        }
        // Try all parsers as fallback
        if let Some((fmt, val)) = validate::try_parse_all(content, path_str) {
            return (parsers::parser_for(fmt), val);
        }
        eprintln!("{path_str}");
        eprintln!("  no schema found (file could not be parsed)");
        std::process::exit(0);
    }

    if let Some((fmt, val)) = validate::try_parse_all(content, path_str) {
        return (parsers::parser_for(fmt), val);
    }

    eprintln!("{path_str}");
    eprintln!("  no schema found (unrecognized format)");
    std::process::exit(0);
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
