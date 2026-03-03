//! Schema resolution for files.
//!
//! Resolves a schema URI for a given file path using priority order:
//! 1. Inline `$schema` / YAML modeline
//! 2. Custom schema mappings from `lintel.toml [schemas]`
//! 3. Catalog matching

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use lintel_cli_common::CliCacheOptions;
use lintel_schema_cache::SchemaCache;
use lintel_validate::parsers;
use lintel_validate::validate;
use schema_catalog::{FileFormat, SchemaMatch};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The source that resolved the schema URI for a file.
#[derive(Debug)]
pub enum SchemaSource {
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

/// Result of resolving a schema for a given file path.
pub struct ResolvedFileSchema {
    /// The final schema URI (after rewrites and path resolution).
    pub schema_uri: String,
    /// A human-readable name (from catalog or URI).
    pub display_name: String,
    /// Whether the schema is a remote URL.
    pub is_remote: bool,
    /// How the schema was resolved.
    pub source: SchemaSource,
    /// The glob pattern that matched (config or catalog).
    pub matched_pattern: Option<String>,
    /// All file-match globs from the catalog entry.
    pub file_match: Vec<String>,
    /// Schema description from the catalog.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

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
// Public functions
// ---------------------------------------------------------------------------

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

    let search_dir = config_search_dir.or_else(|| file_path.parent());
    let ctx = lintel_config::ConfigContext::load_from_dir(search_dir, &[]);

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &ctx.config, cache.no_catalog).await;

    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, content, &path_str);

    let Some(resolved) = resolve_schema(
        parser.as_ref(),
        content,
        &instance,
        &path_str,
        file_name,
        &ctx.config,
        &compiled_catalogs,
    ) else {
        return Ok(None);
    };

    Ok(Some(build_resolved_file_schema(
        resolved,
        &ctx.config,
        &ctx.config_dir,
        file_path,
        &compiled_catalogs,
    )))
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

    let ctx = lintel_config::ConfigContext::load_from_dir(file_path.parent(), &[]);

    let compiled_catalogs =
        validate::fetch_compiled_catalogs(&retriever, &ctx.config, cache.no_catalog).await;

    let Some(resolved) =
        resolve_schema_path_only(&path_str, file_name, &ctx.config, &compiled_catalogs)
    else {
        return Ok(None);
    };

    Ok(Some(build_resolved_file_schema(
        resolved,
        &ctx.config,
        &ctx.config_dir,
        file_path,
        &compiled_catalogs,
    )))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build a `ResolvedFileSchema` from intermediate resolution data.
#[allow(clippy::too_many_arguments)]
fn build_resolved_file_schema(
    resolved: ResolvedSchema<'_>,
    cfg: &lintel_config::Config,
    config_dir: &Path,
    file_path: &Path,
    compiled_catalogs: &[schema_catalog::CompiledCatalog],
) -> ResolvedFileSchema {
    let from_inline = matches!(resolved.source, SchemaSource::Inline);
    let (schema_uri, is_remote) = finalize_uri(
        &resolved.uri,
        &cfg.rewrite,
        config_dir,
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

    let matched_pattern = match &resolved.source {
        SchemaSource::Config => resolved.config_pattern.map(str::to_string),
        SchemaSource::Catalog => resolved
            .catalog_match
            .as_ref()
            .map(|m| m.matched_pattern.to_string()),
        SchemaSource::Inline => None,
    };

    let file_match = resolved
        .catalog_match
        .as_ref()
        .map(|m| m.file_match.to_vec())
        .unwrap_or_default();

    let description = resolved
        .catalog_match
        .as_ref()
        .and_then(|m| m.description.map(str::to_string));

    ResolvedFileSchema {
        schema_uri,
        display_name,
        is_remote,
        source: resolved.source,
        matched_pattern,
        file_match,
        description,
    }
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
            glob_matcher::glob_match(pattern, p) || glob_matcher::glob_match(pattern, file_name)
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

/// Parse the file content, trying the detected format first, then all parsers as fallback.
///
/// Exits the process when the file cannot be parsed.
fn parse_file(
    detected_format: Option<FileFormat>,
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
