#![doc = include_str!("../README.md")]

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bpaf::Bpaf;
use lintel_cli_common::{CLIGlobalOptions, CliCacheOptions};

use lintel_check::config;
use lintel_check::parsers;
use lintel_check::retriever::SchemaCache;
use lintel_check::validate;
use schemastore::SchemaMatch;

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
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let content =
        std::fs::read_to_string(file_path).with_context(|| format!("failed to read {path_str}"))?;

    let retriever = build_retriever(cache);

    let config_search_dir = file_path.parent().map(Path::to_path_buf);
    let (cfg, config_dir, _config_path) = validate::load_config(config_search_dir.as_deref());

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &cfg, cache.no_catalog).await;

    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, &content, &path_str);

    let resolved = if let Some(uri) = parser.extract_schema_uri(&content, &instance) {
        ResolvedSchema {
            uri,
            source: SchemaSource::Inline,
            catalog_match: None,
            config_pattern: None,
        }
    } else if let Some((_pattern, url)) = cfg
        .schemas
        .iter()
        .find(|(pattern, _)| {
            let p = path_str.strip_prefix("./").unwrap_or(&path_str);
            glob_match::glob_match(pattern, p) || glob_match::glob_match(pattern, file_name)
        })
        .map(|(pattern, url)| (pattern.as_str(), url.as_str()))
    {
        ResolvedSchema {
            uri: url.to_string(),
            source: SchemaSource::Config,
            catalog_match: None,
            config_pattern: None,
        }
    } else if let Some(schema_match) = compiled_catalogs
        .iter()
        .find_map(|cat| cat.find_schema_detailed(&path_str, file_name))
    {
        ResolvedSchema {
            uri: schema_match.url.to_string(),
            source: SchemaSource::Catalog,
            catalog_match: Some(schema_match.into()),
            config_pattern: None,
        }
    } else {
        return Ok(None);
    };

    let schema_uri = config::apply_rewrites(&resolved.uri, &cfg.rewrite);
    let schema_uri = config::resolve_double_slash(&schema_uri, &config_dir);

    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
    let schema_uri = if is_remote {
        schema_uri
    } else {
        file_path
            .parent()
            .map(|parent| parent.join(&schema_uri).to_string_lossy().to_string())
            .unwrap_or(schema_uri)
    };

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

#[allow(
    clippy::too_many_lines,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
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

    // Load config
    let config_search_dir = file_path.parent().map(Path::to_path_buf);
    let (cfg, config_dir, _config_path) = validate::load_config(config_search_dir.as_deref());

    // Fetch catalogs
    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &cfg, args.cache.no_catalog).await;

    // Detect format and parse
    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, &content, &path_str);

    // Schema resolution priority (same as validate):
    // 1. Inline $schema / YAML modeline
    // 2. Custom schema mappings from lintel.toml [schemas]
    // 3. Catalog matching (detailed)
    let resolved = if let Some(uri) = parser.extract_schema_uri(&content, &instance) {
        ResolvedSchema {
            uri,
            source: SchemaSource::Inline,
            catalog_match: None,
            config_pattern: None,
        }
    } else if let Some((pattern, url)) = cfg
        .schemas
        .iter()
        .find(|(pattern, _)| {
            let p = path_str.strip_prefix("./").unwrap_or(&path_str);
            glob_match::glob_match(pattern, p) || glob_match::glob_match(pattern, file_name)
        })
        .map(|(pattern, url)| (pattern.as_str(), url.as_str()))
    {
        ResolvedSchema {
            uri: url.to_string(),
            source: SchemaSource::Config,
            catalog_match: None,
            config_pattern: Some(pattern),
        }
    } else if let Some(schema_match) = compiled_catalogs
        .iter()
        .find_map(|cat| cat.find_schema_detailed(&path_str, file_name))
    {
        ResolvedSchema {
            uri: schema_match.url.to_string(),
            source: SchemaSource::Catalog,
            catalog_match: Some(schema_match.into()),
            config_pattern: None,
        }
    } else {
        eprintln!("{path_str}");
        eprintln!("  no schema found");
        return Ok(false);
    };

    // Apply rewrites
    let schema_uri = config::apply_rewrites(&resolved.uri, &cfg.rewrite);
    let schema_uri = config::resolve_double_slash(&schema_uri, &config_dir);

    // Resolve relative local paths against the file's parent directory
    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
    let schema_uri = if is_remote {
        schema_uri
    } else {
        file_path
            .parent()
            .map(|parent| parent.join(&schema_uri).to_string_lossy().to_string())
            .unwrap_or(schema_uri)
    };

    // Determine display name
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

    // Basic output
    println!("{path_str}");
    if display_name == schema_uri {
        println!("  schema: {schema_uri}");
    } else {
        println!("  schema: {display_name} ({schema_uri})");
    }
    println!("  source: {}", resolved.source);

    // Show match details
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

    // Explain mode
    if args.explain {
        // Fetch the schema
        let schema_value = if is_remote {
            match retriever.fetch(&schema_uri).await {
                Ok((val, _)) => val,
                Err(e) => {
                    eprintln!("  error fetching schema: {e}");
                    return Ok(false);
                }
            }
        } else {
            let schema_content = std::fs::read_to_string(&schema_uri)
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
        let syntax_hl = use_color && !args.no_syntax_highlighting;
        let output = jsonschema_explain::explain(&schema_value, display_name, use_color, syntax_hl);

        if is_tty && !args.no_pager {
            lintel_cli_common::pipe_to_pager(&format!("\n{output}"));
        } else {
            println!();
            print!("{output}");
        }
    }

    Ok(false)
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
