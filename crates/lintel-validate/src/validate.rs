use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;
use serde_json::Value;

use lintel_diagnostics::reporter::{CheckResult, CheckedFile};
use lintel_diagnostics::{DEFAULT_LABEL, LintelDiagnostic, find_instance_path_span, format_label};
use lintel_schema_cache::{CacheStatus, SchemaCache};
use lintel_validation_cache::{ValidationCacheStatus, ValidationError};
use schema_catalog::{CompiledCatalog, FileFormat};

use crate::catalog;
use crate::discover;
use crate::parsers::{self, Parser};
use crate::registry;

/// Conservative limit for concurrent file reads to avoid exhausting file
/// descriptors. 128 is well below the default soft limit on macOS (256) and
/// Linux (1024) while still providing good throughput.
const FD_CONCURRENCY_LIMIT: usize = 128;

/// Composite retriever that dispatches `file://` URIs to local disk reads
/// and everything else to the HTTP-backed [`SchemaCache`].
struct LocalRetriever {
    http: SchemaCache,
}

#[async_trait::async_trait]
impl jsonschema::AsyncRetrieve for LocalRetriever {
    async fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn core::error::Error + Send + Sync>> {
        let s = uri.as_str();
        if let Some(raw) = s.strip_prefix("file://") {
            let path = percent_encoding::percent_decode_str(raw).decode_utf8()?;
            let content = tokio::fs::read_to_string(path.as_ref()).await?;
            Ok(serde_json::from_str(&content)?)
        } else {
            self.http.retrieve(uri).await
        }
    }
}

pub struct ValidateArgs {
    /// Glob patterns to find files (empty = auto-discover)
    pub globs: Vec<String>,

    /// Exclude files matching these globs (repeatable)
    pub exclude: Vec<String>,

    /// Cache directory for remote schemas
    pub cache_dir: Option<String>,

    /// Bypass schema cache reads (still writes fetched schemas to cache)
    pub force_schema_fetch: bool,

    /// Bypass validation cache reads (still writes results to cache)
    pub force_validation: bool,

    /// Disable `SchemaStore` catalog matching
    pub no_catalog: bool,

    /// Directory to search for `lintel.toml` (defaults to cwd)
    pub config_dir: Option<PathBuf>,

    /// TTL for cached schemas. `None` means no expiry.
    pub schema_cache_ttl: Option<core::time::Duration>,
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// A file that has been parsed and matched to a schema URI.
struct ParsedFile {
    path: String,
    content: String,
    instance: Value,
    /// Original schema URI before rewrites (for override matching).
    original_schema_uri: String,
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Locate `lintel.toml`, load the full config, and return the config directory.
/// Returns `(config, config_dir, config_path)`.  When no config is found or
/// cwd is unavailable the config is default and `config_path` is `None`.
#[tracing::instrument(skip_all)]
pub fn load_config(search_dir: Option<&Path>) -> (lintel_config::Config, PathBuf, Option<PathBuf>) {
    let start_dir = match search_dir {
        Some(d) => d.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return (lintel_config::Config::default(), PathBuf::from("."), None),
        },
    };

    let Some(config_path) = lintel_config::find_config_path(&start_dir) else {
        return (lintel_config::Config::default(), start_dir, None);
    };

    let dir = config_path.parent().unwrap_or(&start_dir).to_path_buf();
    let cfg = lintel_config::find_and_load(&start_dir)
        .ok()
        .flatten()
        .unwrap_or_default();
    (cfg, dir, Some(config_path))
}

// ---------------------------------------------------------------------------
// File collection
// ---------------------------------------------------------------------------

/// Collect input files from globs/directories, applying exclude filters.
///
/// # Errors
///
/// Returns an error if a glob pattern is invalid or a directory cannot be walked.
#[tracing::instrument(skip_all, fields(glob_count = globs.len(), exclude_count = exclude.len()))]
pub fn collect_files(globs: &[String], exclude: &[String]) -> Result<Vec<PathBuf>> {
    if globs.is_empty() {
        return discover::discover_files(".", exclude);
    }

    let mut result = Vec::new();
    for pattern in globs {
        let path = Path::new(pattern);
        if path.is_dir() {
            result.extend(discover::discover_files(pattern, exclude)?);
        } else {
            for entry in glob(pattern).with_context(|| format!("invalid glob: {pattern}"))? {
                let path = entry?;
                if path.is_file() && !is_excluded(&path, exclude) {
                    result.push(path);
                }
            }
        }
    }
    Ok(result)
}

fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let path_str = match path.to_str() {
        Some(s) => s.strip_prefix("./").unwrap_or(s),
        None => return false,
    };
    excludes
        .iter()
        .any(|pattern| glob_match::glob_match(pattern, path_str))
}

// ---------------------------------------------------------------------------
// Phase 1: Parse files and resolve schema URIs
// ---------------------------------------------------------------------------

/// Try parsing content with each known format, returning the first success.
///
/// JSONC is tried first (superset of JSON, handles comments), then YAML and
/// TOML which cover the most common config formats, followed by the rest.
pub fn try_parse_all(content: &str, file_name: &str) -> Option<(FileFormat, Value)> {
    use FileFormat::{Json, Json5, Jsonc, Markdown, Toml, Yaml};
    const FORMATS: [FileFormat; 6] = [Jsonc, Yaml, Toml, Json, Json5, Markdown];

    for fmt in FORMATS {
        let parser = parsers::parser_for(fmt);
        if let Ok(val) = parser.parse(content, file_name) {
            return Some((fmt, val));
        }
    }
    None
}

/// Result of processing a single file: either a parsed file with its schema URI,
/// a lint error, or nothing (file was skipped).
enum FileResult {
    Parsed {
        schema_uri: String,
        parsed: ParsedFile,
    },
    Error(LintelDiagnostic),
    Skip,
}

/// Resolve a relative local schema path against a base directory.
///
/// Remote URIs (http/https) are returned unchanged. For local paths, joins with
/// the provided base directory (file's parent for inline `$schema`, config dir
/// for config/catalog sources).
fn resolve_local_schema_path(schema_uri: &str, base_dir: Option<&Path>) -> String {
    if schema_uri.starts_with("http://") || schema_uri.starts_with("https://") {
        return schema_uri.to_string();
    }
    if let Some(dir) = base_dir {
        dir.join(schema_uri).to_string_lossy().to_string()
    } else {
        schema_uri.to_string()
    }
}

/// Process a single file's already-read content: parse and resolve schema URI.
///
/// Returns a `Vec` because JSONL files expand to one result per non-empty line.
#[allow(clippy::too_many_arguments)]
fn process_one_file(
    path: &Path,
    content: String,
    config: &lintel_config::Config,
    config_dir: &Path,
    compiled_catalogs: &[CompiledCatalog],
) -> Vec<FileResult> {
    let path_str = path.display().to_string();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let detected_format = parsers::detect_format(path);

    // JSONL files get special per-line handling.
    if detected_format == Some(FileFormat::Jsonl) {
        return process_jsonl_file(
            path,
            &path_str,
            file_name,
            &content,
            config,
            config_dir,
            compiled_catalogs,
        );
    }

    // For unrecognized extensions, only proceed if a catalog or config mapping matches.
    if detected_format.is_none() {
        let has_match = config.find_schema_mapping(&path_str, file_name).is_some()
            || compiled_catalogs
                .iter()
                .any(|cat| cat.find_schema(&path_str, file_name).is_some());
        if !has_match {
            return vec![FileResult::Skip];
        }
    }

    // Parse the file content.
    let (parser, instance): (Box<dyn Parser>, Value) = if let Some(fmt) = detected_format {
        let parser = parsers::parser_for(fmt);
        match parser.parse(&content, &path_str) {
            Ok(val) => (parser, val),
            Err(parse_err) => return vec![FileResult::Error(parse_err)],
        }
    } else {
        match try_parse_all(&content, &path_str) {
            Some((fmt, val)) => (parsers::parser_for(fmt), val),
            None => return vec![FileResult::Skip],
        }
    };

    // Skip markdown files with no frontmatter
    if instance.is_null() {
        return vec![FileResult::Skip];
    }

    // Schema resolution priority:
    // 1. Inline $schema / YAML modeline (always wins)
    // 2. Custom schema mappings from lintel.toml [schemas]
    // 3. Catalog matching (custom registries > Lintel catalog > SchemaStore)
    //
    // Track whether the URI came from inline $schema (resolve relative to file)
    // or from config/catalog (resolve relative to config dir).
    let inline_uri = parser.extract_schema_uri(&content, &instance);
    let from_inline = inline_uri.is_some();
    let schema_uri = inline_uri
        .or_else(|| {
            config
                .find_schema_mapping(&path_str, file_name)
                .map(str::to_string)
        })
        .or_else(|| {
            compiled_catalogs
                .iter()
                .find_map(|cat| cat.find_schema(&path_str, file_name))
                .map(str::to_string)
        });

    let Some(schema_uri) = schema_uri else {
        return vec![FileResult::Skip];
    };

    // Keep original URI for override matching (before rewrites)
    let original_schema_uri = schema_uri.clone();

    // Apply rewrite rules, then resolve // paths relative to lintel.toml
    let schema_uri = lintel_config::apply_rewrites(&schema_uri, &config.rewrite);
    let schema_uri = lintel_config::resolve_double_slash(&schema_uri, config_dir);

    // Resolve relative local paths:
    // - Inline $schema: relative to the file's parent directory
    // - Config/catalog: relative to the config directory (where lintel.toml lives)
    let schema_uri = resolve_local_schema_path(
        &schema_uri,
        if from_inline {
            path.parent()
        } else {
            Some(config_dir)
        },
    );

    vec![FileResult::Parsed {
        schema_uri,
        parsed: ParsedFile {
            path: path_str,
            content,
            instance,
            original_schema_uri,
        },
    }]
}

/// Process a JSONL file: parse each line independently and resolve schemas.
///
/// Each non-empty line becomes its own [`FileResult::Parsed`]. Schema resolution
/// priority per line: inline `$schema` on the line > config mapping > catalog.
///
/// Also checks schema consistency across lines — mismatches are emitted as
/// [`FileResult::Error`] so they flow through the normal Reporter pipeline.
#[allow(clippy::too_many_arguments)]
fn process_jsonl_file(
    path: &Path,
    path_str: &str,
    file_name: &str,
    content: &str,
    config: &lintel_config::Config,
    config_dir: &Path,
    compiled_catalogs: &[CompiledCatalog],
) -> Vec<FileResult> {
    let lines = match parsers::jsonl::parse_jsonl(content, path_str) {
        Ok(lines) => lines,
        Err(parse_err) => return vec![FileResult::Error(parse_err)],
    };

    if lines.is_empty() {
        return vec![FileResult::Skip];
    }

    let mut results = Vec::with_capacity(lines.len());

    // Check schema consistency before consuming lines.
    if let Some(mismatches) = parsers::jsonl::check_schema_consistency(&lines) {
        for m in mismatches {
            results.push(FileResult::Error(LintelDiagnostic::SchemaMismatch {
                path: path_str.to_string(),
                line_number: m.line_number,
                message: format!("expected consistent $schema but found {}", m.schema_uri),
            }));
        }
    }

    for line in lines {
        // Schema resolution: inline $schema on line > config > catalog
        // Track source to resolve relative paths correctly.
        let inline_uri = parsers::jsonl::extract_schema_uri(&line.value);
        let from_inline = inline_uri.is_some();
        let schema_uri = inline_uri
            .or_else(|| {
                config
                    .find_schema_mapping(path_str, file_name)
                    .map(str::to_string)
            })
            .or_else(|| {
                compiled_catalogs
                    .iter()
                    .find_map(|cat| cat.find_schema(path_str, file_name))
                    .map(str::to_string)
            });

        let Some(schema_uri) = schema_uri else {
            continue;
        };

        let original_schema_uri = schema_uri.clone();

        let schema_uri = lintel_config::apply_rewrites(&schema_uri, &config.rewrite);
        let schema_uri = lintel_config::resolve_double_slash(&schema_uri, config_dir);

        // Inline $schema: relative to file's parent. Config/catalog: relative to config dir.
        let schema_uri = resolve_local_schema_path(
            &schema_uri,
            if from_inline {
                path.parent()
            } else {
                Some(config_dir)
            },
        );

        let line_path = format!("{path_str}:{}", line.line_number);

        results.push(FileResult::Parsed {
            schema_uri,
            parsed: ParsedFile {
                path: line_path,
                content: line.raw,
                instance: line.value,
                original_schema_uri,
            },
        });
    }

    if results.is_empty() {
        vec![FileResult::Skip]
    } else {
        results
    }
}

/// Read files concurrently with tokio, using a semaphore to avoid exhausting
/// file descriptors. I/O errors are pushed as `LintelDiagnostic::Io`.
///
/// # Panics
///
/// Panics if the internal semaphore is unexpectedly closed (should not happen).
#[tracing::instrument(skip_all, fields(file_count = files.len()))]
pub async fn read_files(
    files: &[PathBuf],
    errors: &mut Vec<LintelDiagnostic>,
) -> Vec<(PathBuf, String)> {
    let semaphore = alloc::sync::Arc::new(tokio::sync::Semaphore::new(FD_CONCURRENCY_LIMIT));
    let mut read_set = tokio::task::JoinSet::new();
    for path in files {
        let path = path.clone();
        let sem = semaphore.clone();
        read_set.spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            let result = tokio::fs::read_to_string(&path).await;
            (path, result)
        });
    }

    let mut file_contents = Vec::with_capacity(files.len());
    while let Some(result) = read_set.join_next().await {
        match result {
            Ok((path, Ok(content))) => file_contents.push((path, content)),
            Ok((path, Err(e))) => {
                errors.push(LintelDiagnostic::Io {
                    path: path.display().to_string(),
                    message: format!("failed to read: {e}"),
                });
            }
            Err(e) => tracing::warn!("file read task panicked: {e}"),
        }
    }

    file_contents
}

/// Parse pre-read file contents, extract schema URIs, apply rewrites, and
/// group by resolved schema URI.
#[tracing::instrument(skip_all, fields(file_count = file_contents.len()))]
#[allow(clippy::too_many_arguments)]
fn parse_and_group_contents(
    file_contents: Vec<(PathBuf, String)>,
    config: &lintel_config::Config,
    config_dir: &Path,
    compiled_catalogs: &[CompiledCatalog],
    errors: &mut Vec<LintelDiagnostic>,
) -> BTreeMap<String, Vec<ParsedFile>> {
    let mut schema_groups: BTreeMap<String, Vec<ParsedFile>> = BTreeMap::new();
    for (path, content) in file_contents {
        let results = process_one_file(&path, content, config, config_dir, compiled_catalogs);
        for result in results {
            match result {
                FileResult::Parsed { schema_uri, parsed } => {
                    schema_groups.entry(schema_uri).or_default().push(parsed);
                }
                FileResult::Error(e) => errors.push(e),
                FileResult::Skip => {}
            }
        }
    }

    schema_groups
}

// ---------------------------------------------------------------------------
// Phase 2: Schema fetching, compilation, and instance validation
// ---------------------------------------------------------------------------

/// Fetch a schema by URI, returning its parsed JSON and cache status.
///
/// For remote URIs, checks the prefetched map first; for local URIs, reads
/// from disk (with in-memory caching to avoid redundant I/O for shared schemas).
#[allow(clippy::too_many_arguments)]
async fn fetch_schema_from_prefetched(
    schema_uri: &str,
    prefetched: &HashMap<String, Result<(Value, CacheStatus), String>>,
    local_cache: &mut HashMap<String, Value>,
    group: &[ParsedFile],
    errors: &mut Vec<LintelDiagnostic>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) -> Option<(Value, Option<CacheStatus>)> {
    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");

    let result: Result<(Value, Option<CacheStatus>), String> = if is_remote {
        match prefetched.get(schema_uri) {
            Some(Ok((v, status))) => Ok((v.clone(), Some(*status))),
            Some(Err(e)) => Err(format!("failed to fetch schema: {schema_uri}: {e}")),
            None => Err(format!("schema not prefetched: {schema_uri}")),
        }
    } else if let Some(cached) = local_cache.get(schema_uri) {
        Ok((cached.clone(), None))
    } else {
        tokio::fs::read_to_string(schema_uri)
            .await
            .map_err(|e| format!("failed to read local schema {schema_uri}: {e}"))
            .and_then(|content| {
                serde_json::from_str::<Value>(&content)
                    .map(|v| {
                        local_cache.insert(schema_uri.to_string(), v.clone());
                        (v, None)
                    })
                    .map_err(|e| format!("failed to parse local schema {schema_uri}: {e}"))
            })
    };

    match result {
        Ok(value) => Some(value),
        Err(message) => {
            report_group_error(
                |path| LintelDiagnostic::SchemaFetch {
                    path: path.to_string(),
                    message: message.clone(),
                },
                schema_uri,
                None,
                group,
                errors,
                checked,
                on_check,
            );
            None
        }
    }
}

/// Report the same error for every file in a schema group.
#[allow(clippy::too_many_arguments)]
fn report_group_error<P: alloc::borrow::Borrow<ParsedFile>>(
    make_error: impl Fn(&str) -> LintelDiagnostic,
    schema_uri: &str,
    cache_status: Option<CacheStatus>,
    group: &[P],
    errors: &mut Vec<LintelDiagnostic>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for item in group {
        let pf = item.borrow();
        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
            validation_cache_status: None,
        };
        on_check(&cf);
        checked.push(cf);
        errors.push(make_error(&pf.path));
    }
}

/// Mark every file in a group as checked (no errors).
#[allow(clippy::too_many_arguments)]
fn mark_group_checked<P: alloc::borrow::Borrow<ParsedFile>>(
    schema_uri: &str,
    cache_status: Option<CacheStatus>,
    validation_cache_status: Option<ValidationCacheStatus>,
    group: &[P],
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for item in group {
        let pf = item.borrow();
        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
            validation_cache_status,
        };
        on_check(&cf);
        checked.push(cf);
    }
}

/// Clean up error messages from the `jsonschema` crate.
///
/// For `anyOf`/`oneOf` failures the crate dumps the entire JSON value into the
/// message (e.g. `{...} is not valid under any of the schemas listed in the 'oneOf' keyword`).
/// The source snippet already shows the value, so we strip the redundant prefix
/// and keep only `"not valid under any of the schemas listed in the 'oneOf' keyword"`.
///
/// All other messages are returned unchanged.
fn clean_error_message(msg: String) -> String {
    const MARKER: &str = " is not valid under any of the schemas listed in the '";
    if let Some(pos) = msg.find(MARKER) {
        // pos points to " is not valid...", skip " is " (4 chars) to get "not valid..."
        return msg[pos + 4..].to_string();
    }
    msg
}

/// Convert [`ValidationError`]s into [`LintelDiagnostic::Validation`] diagnostics.
fn push_validation_errors(
    pf: &ParsedFile,
    schema_url: &str,
    validation_errors: &[ValidationError],
    errors: &mut Vec<LintelDiagnostic>,
) {
    for ve in validation_errors {
        let span = find_instance_path_span(&pf.content, &ve.instance_path);
        let instance_path = if ve.instance_path.is_empty() {
            DEFAULT_LABEL.to_string()
        } else {
            ve.instance_path.clone()
        };
        let label = format_label(&instance_path, &ve.schema_path);
        let source_span: miette::SourceSpan = span.into();
        errors.push(LintelDiagnostic::Validation {
            src: miette::NamedSource::new(&pf.path, pf.content.clone()),
            span: source_span,
            schema_span: source_span,
            path: pf.path.clone(),
            instance_path,
            label,
            message: ve.message.clone(),
            schema_url: schema_url.to_string(),
            schema_path: ve.schema_path.clone(),
        });
    }
}

/// Validate all files in a group against an already-compiled validator and store
/// results in the validation cache.
#[tracing::instrument(skip_all, fields(schema_uri, file_count = group.len()))]
#[allow(clippy::too_many_arguments)]
async fn validate_group<P: alloc::borrow::Borrow<ParsedFile>>(
    validator: &jsonschema::Validator,
    schema_uri: &str,
    schema_hash: &str,
    validate_formats: bool,
    cache_status: Option<CacheStatus>,
    group: &[P],
    vcache: &lintel_validation_cache::ValidationCache,
    errors: &mut Vec<LintelDiagnostic>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for item in group {
        let pf = item.borrow();
        let file_errors: Vec<ValidationError> = validator
            .iter_errors(&pf.instance)
            .map(|error| ValidationError {
                instance_path: error.instance_path().to_string(),
                message: clean_error_message(error.to_string()),
                schema_path: error.schema_path().to_string(),
            })
            .collect();

        vcache
            .store(
                &lintel_validation_cache::CacheKey {
                    file_content: &pf.content,
                    schema_hash,
                    validate_formats,
                },
                &file_errors,
            )
            .await;
        push_validation_errors(pf, schema_uri, &file_errors, errors);

        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
            validation_cache_status: Some(ValidationCacheStatus::Miss),
        };
        on_check(&cf);
        checked.push(cf);
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch and compile all schema catalogs (default, `SchemaStore`, and custom registries).
///
/// Returns a list of compiled catalogs, printing warnings for any that fail to fetch.
pub async fn fetch_compiled_catalogs(
    retriever: &SchemaCache,
    config: &lintel_config::Config,
    no_catalog: bool,
) -> Vec<CompiledCatalog> {
    let mut compiled_catalogs = Vec::new();

    if !no_catalog {
        let catalog_span = tracing::info_span!("fetch_catalogs").entered();

        // Catalogs are fetched concurrently but sorted by priority so that
        // the Lintel catalog wins over custom registries, which win over
        // SchemaStore.  The `order` field encodes this precedence.
        #[allow(clippy::items_after_statements)]
        type CatalogResult = (
            usize, // priority (lower = higher precedence)
            String,
            Result<CompiledCatalog, Box<dyn core::error::Error + Send + Sync>>,
        );
        let mut catalog_tasks: tokio::task::JoinSet<CatalogResult> = tokio::task::JoinSet::new();

        // Custom registries from lintel.toml (highest precedence among catalogs)
        for (i, registry_url) in config.registries.iter().enumerate() {
            let r = retriever.clone();
            let url = registry_url.clone();
            let label = format!("registry {url}");
            catalog_tasks.spawn(async move {
                let result = registry::fetch(&r, &url)
                    .await
                    .map(|cat| CompiledCatalog::compile(&cat));
                (i, label, result)
            });
        }

        // Lintel catalog
        let lintel_order = config.registries.len();
        if !config.no_default_catalog {
            let r = retriever.clone();
            let label = format!("default catalog {}", registry::DEFAULT_REGISTRY);
            catalog_tasks.spawn(async move {
                let result = registry::fetch(&r, registry::DEFAULT_REGISTRY)
                    .await
                    .map(|cat| CompiledCatalog::compile(&cat));
                (lintel_order, label, result)
            });
        }

        // SchemaStore catalog (lowest precedence)
        let schemastore_order = config.registries.len() + 1;
        let r = retriever.clone();
        catalog_tasks.spawn(async move {
            let result = catalog::fetch_catalog(&r)
                .await
                .map(|cat| CompiledCatalog::compile(&cat));
            (schemastore_order, "SchemaStore catalog".to_string(), result)
        });

        let mut results: Vec<(usize, CompiledCatalog)> = Vec::new();
        while let Some(result) = catalog_tasks.join_next().await {
            match result {
                Ok((order, _, Ok(compiled))) => results.push((order, compiled)),
                Ok((_, label, Err(e))) => eprintln!("warning: failed to fetch {label}: {e}"),
                Err(e) => eprintln!("warning: catalog fetch task failed: {e}"),
            }
        }
        results.sort_by_key(|(order, _)| *order);
        compiled_catalogs.extend(results.into_iter().map(|(_, cat)| cat));

        drop(catalog_span);
    }

    compiled_catalogs
}

/// # Errors
///
/// Returns an error if file collection or schema validation encounters an I/O error.
pub async fn run(args: &ValidateArgs) -> Result<CheckResult> {
    run_with(args, None, |_| {}).await
}

/// Like [`run`], but calls `on_check` each time a file is checked, allowing
/// callers to stream progress (e.g. verbose output) as files are processed.
///
/// # Errors
///
/// Returns an error if file collection or schema validation encounters an I/O error.
#[tracing::instrument(skip_all, name = "validate")]
pub async fn run_with(
    args: &ValidateArgs,
    cache: Option<SchemaCache>,
    mut on_check: impl FnMut(&CheckedFile),
) -> Result<CheckResult> {
    let retriever = build_retriever(args, cache);
    let (config, config_dir, _config_path) = load_config(args.config_dir.as_deref());
    let files = collect_files(&args.globs, &args.exclude)?;
    tracing::info!(file_count = files.len(), "collected files");

    let compiled_catalogs = fetch_compiled_catalogs(&retriever, &config, args.no_catalog).await;

    let mut errors: Vec<LintelDiagnostic> = Vec::new();
    let file_contents = read_files(&files, &mut errors).await;

    run_with_contents_inner(
        file_contents,
        args,
        retriever,
        config,
        &config_dir,
        compiled_catalogs,
        errors,
        &mut on_check,
    )
    .await
}

/// Like [`run_with`], but accepts pre-read file contents instead of reading
/// from disk. Use this when the caller has already read files (e.g. to share
/// reads between format checking and validation).
///
/// # Errors
///
/// Returns an error if schema validation encounters an I/O or network error.
pub async fn run_with_contents(
    args: &ValidateArgs,
    file_contents: Vec<(PathBuf, String)>,
    cache: Option<SchemaCache>,
    mut on_check: impl FnMut(&CheckedFile),
) -> Result<CheckResult> {
    let retriever = build_retriever(args, cache);
    let (config, config_dir, _config_path) = load_config(args.config_dir.as_deref());
    let compiled_catalogs = fetch_compiled_catalogs(&retriever, &config, args.no_catalog).await;
    let errors: Vec<LintelDiagnostic> = Vec::new();

    run_with_contents_inner(
        file_contents,
        args,
        retriever,
        config,
        &config_dir,
        compiled_catalogs,
        errors,
        &mut on_check,
    )
    .await
}

fn build_retriever(args: &ValidateArgs, cache: Option<SchemaCache>) -> SchemaCache {
    if let Some(c) = cache {
        return c;
    }
    let mut builder = SchemaCache::builder().force_fetch(args.force_schema_fetch);
    if let Some(dir) = &args.cache_dir {
        let path = PathBuf::from(dir);
        let _ = fs::create_dir_all(&path);
        builder = builder.cache_dir(path);
    }
    if let Some(ttl) = args.schema_cache_ttl {
        builder = builder.ttl(ttl);
    }
    builder.build()
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
async fn run_with_contents_inner(
    file_contents: Vec<(PathBuf, String)>,
    args: &ValidateArgs,
    retriever: SchemaCache,
    config: lintel_config::Config,
    config_dir: &Path,
    compiled_catalogs: Vec<CompiledCatalog>,
    mut errors: Vec<LintelDiagnostic>,
    on_check: &mut impl FnMut(&CheckedFile),
) -> Result<CheckResult> {
    let mut checked: Vec<CheckedFile> = Vec::new();

    // Phase 1: Parse files and resolve schema URIs
    let schema_groups = parse_and_group_contents(
        file_contents,
        &config,
        config_dir,
        &compiled_catalogs,
        &mut errors,
    );
    tracing::info!(
        schema_count = schema_groups.len(),
        total_files = schema_groups.values().map(Vec::len).sum::<usize>(),
        "grouped files by schema"
    );

    // Create validation cache
    let vcache = lintel_validation_cache::ValidationCache::new(
        lintel_validation_cache::ensure_cache_dir(),
        args.force_validation,
    );

    // Prefetch all remote schemas in parallel
    let remote_uris: Vec<&String> = schema_groups
        .keys()
        .filter(|uri| uri.starts_with("http://") || uri.starts_with("https://"))
        .collect();

    let prefetched = {
        let _prefetch_span =
            tracing::info_span!("prefetch_schemas", count = remote_uris.len()).entered();

        let mut schema_tasks = tokio::task::JoinSet::new();
        for uri in remote_uris {
            let r = retriever.clone();
            let u = uri.clone();
            schema_tasks.spawn(async move {
                let result = r.fetch(&u).await;
                (u, result)
            });
        }

        let mut prefetched: HashMap<String, Result<(Value, CacheStatus), String>> = HashMap::new();
        while let Some(result) = schema_tasks.join_next().await {
            match result {
                Ok((uri, fetch_result)) => {
                    prefetched.insert(uri, fetch_result.map_err(|e| e.to_string()));
                }
                Err(e) => eprintln!("warning: schema prefetch task failed: {e}"),
            }
        }

        prefetched
    };

    // Phase 2: Compile each schema once and validate all matching files
    let mut local_schema_cache: HashMap<String, Value> = HashMap::new();
    let mut fetch_time = core::time::Duration::ZERO;
    let mut hash_time = core::time::Duration::ZERO;
    let mut vcache_time = core::time::Duration::ZERO;
    let mut compile_time = core::time::Duration::ZERO;
    let mut validate_time = core::time::Duration::ZERO;

    for (schema_uri, group) in &schema_groups {
        let _group_span = tracing::debug_span!(
            "schema_group",
            schema = schema_uri.as_str(),
            files = group.len(),
        )
        .entered();

        // If ANY file in the group matches a `validate_formats = false` override,
        // disable format validation for the whole group (they share one compiled validator).
        let validate_formats = group.iter().all(|pf| {
            config
                .should_validate_formats(&pf.path, &[&pf.original_schema_uri, schema_uri.as_str()])
        });

        // Remote schemas were prefetched in parallel above; local schemas are
        // read from disk here (with in-memory caching).
        let t = std::time::Instant::now();
        let Some((schema_value, cache_status)) = fetch_schema_from_prefetched(
            schema_uri,
            &prefetched,
            &mut local_schema_cache,
            group,
            &mut errors,
            &mut checked,
            on_check,
        )
        .await
        else {
            fetch_time += t.elapsed();
            continue;
        };
        fetch_time += t.elapsed();

        // Pre-compute schema hash once for the entire group.
        let t = std::time::Instant::now();
        let schema_hash = lintel_validation_cache::schema_hash(&schema_value);
        hash_time += t.elapsed();

        // Split the group into validation cache hits and misses.
        let mut cache_misses: Vec<&ParsedFile> = Vec::new();

        let t = std::time::Instant::now();
        for pf in group {
            let (cached, vcache_status) = vcache
                .lookup(&lintel_validation_cache::CacheKey {
                    file_content: &pf.content,
                    schema_hash: &schema_hash,
                    validate_formats,
                })
                .await;

            if let Some(cached_errors) = cached {
                push_validation_errors(pf, schema_uri, &cached_errors, &mut errors);
                let cf = CheckedFile {
                    path: pf.path.clone(),
                    schema: schema_uri.clone(),
                    cache_status,
                    validation_cache_status: Some(vcache_status),
                };
                on_check(&cf);
                checked.push(cf);
            } else {
                cache_misses.push(pf);
            }
        }
        vcache_time += t.elapsed();

        tracing::debug!(
            cache_hits = group.len() - cache_misses.len(),
            cache_misses = cache_misses.len(),
            "validation cache"
        );

        // If all files hit the validation cache, skip schema compilation entirely.
        if cache_misses.is_empty() {
            continue;
        }

        // Compile the schema for cache misses.
        let t = std::time::Instant::now();
        let validator = {
            // Set base URI so relative $ref values (e.g. "./rule.json") resolve
            // correctly. Remote schemas use the HTTP URI directly; local schemas
            // get a file:// URI derived from the canonical absolute path.
            let is_remote_schema =
                schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
            let local_retriever = LocalRetriever {
                http: retriever.clone(),
            };
            let opts = jsonschema::async_options()
                .with_retriever(local_retriever)
                .should_validate_formats(validate_formats);
            let base_uri = if is_remote_schema {
                // Strip fragment (e.g. "#") — base URIs must not contain fragments.
                let uri = match schema_uri.find('#') {
                    Some(pos) => schema_uri[..pos].to_string(),
                    None => schema_uri.clone(),
                };
                Some(uri)
            } else {
                std::fs::canonicalize(schema_uri)
                    .ok()
                    .map(|p| format!("file://{}", p.display()))
            };
            let opts = if let Some(uri) = base_uri {
                opts.with_base_uri(uri)
            } else {
                opts
            };
            match opts.build(&schema_value).await {
                Ok(v) => v,
                Err(e) => {
                    compile_time += t.elapsed();
                    // When format validation is disabled and the compilation error
                    // is a uri-reference issue (e.g. Rust-style $ref paths in
                    // vector.json), skip validation silently.
                    if !validate_formats && e.to_string().contains("uri-reference") {
                        mark_group_checked(
                            schema_uri,
                            cache_status,
                            Some(ValidationCacheStatus::Miss),
                            &cache_misses,
                            &mut checked,
                            on_check,
                        );
                        continue;
                    }
                    let msg = format!("failed to compile schema: {e}");
                    report_group_error(
                        |path| LintelDiagnostic::SchemaCompile {
                            path: path.to_string(),
                            message: msg.clone(),
                        },
                        schema_uri,
                        cache_status,
                        &cache_misses,
                        &mut errors,
                        &mut checked,
                        on_check,
                    );
                    continue;
                }
            }
        };
        compile_time += t.elapsed();

        let t = std::time::Instant::now();
        validate_group(
            &validator,
            schema_uri,
            &schema_hash,
            validate_formats,
            cache_status,
            &cache_misses,
            &vcache,
            &mut errors,
            &mut checked,
            on_check,
        )
        .await;
        validate_time += t.elapsed();
    }

    #[allow(clippy::cast_possible_truncation)]
    {
        tracing::info!(
            fetch_ms = fetch_time.as_millis() as u64,
            hash_ms = hash_time.as_millis() as u64,
            vcache_ms = vcache_time.as_millis() as u64,
            compile_ms = compile_time.as_millis() as u64,
            validate_ms = validate_time.as_millis() as u64,
            "phase2 breakdown"
        );
    }

    // Sort errors for deterministic output (by path, then by span offset)
    errors.sort_by(|a, b| {
        a.path()
            .cmp(b.path())
            .then_with(|| a.offset().cmp(&b.offset()))
    });

    Ok(CheckResult { errors, checked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lintel_schema_cache::SchemaCache;
    use std::path::Path;

    fn mock(entries: &[(&str, &str)]) -> SchemaCache {
        let cache = SchemaCache::memory();
        for (uri, body) in entries {
            cache.insert(
                uri,
                serde_json::from_str(body).expect("test mock: invalid JSON"),
            );
        }
        cache
    }

    fn testdata() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
    }

    /// Build glob patterns that scan one or more testdata directories for all supported file types.
    fn scenario_globs(dirs: &[&str]) -> Vec<String> {
        dirs.iter()
            .flat_map(|dir| {
                let base = testdata().join(dir);
                vec![
                    base.join("*.json").to_string_lossy().to_string(),
                    base.join("*.yaml").to_string_lossy().to_string(),
                    base.join("*.yml").to_string_lossy().to_string(),
                    base.join("*.json5").to_string_lossy().to_string(),
                    base.join("*.jsonc").to_string_lossy().to_string(),
                    base.join("*.toml").to_string_lossy().to_string(),
                ]
            })
            .collect()
    }

    fn args_for_dirs(dirs: &[&str]) -> ValidateArgs {
        ValidateArgs {
            globs: scenario_globs(dirs),
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        }
    }

    const SCHEMA: &str =
        r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;

    fn schema_mock() -> SchemaCache {
        mock(&[("https://example.com/schema.json", SCHEMA)])
    }

    // --- Directory scanning tests ---

    #[tokio::test]
    async fn no_matching_files() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn dir_all_valid() -> anyhow::Result<()> {
        let c = args_for_dirs(&["positive_tests"]);
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn dir_all_invalid() -> anyhow::Result<()> {
        let c = args_for_dirs(&["negative_tests"]);
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn dir_mixed_valid_and_invalid() -> anyhow::Result<()> {
        let c = args_for_dirs(&["positive_tests", "negative_tests"]);
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn dir_no_schemas_skipped() -> anyhow::Result<()> {
        let c = args_for_dirs(&["no_schema"]);
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn dir_valid_with_no_schema_files() -> anyhow::Result<()> {
        let c = args_for_dirs(&["positive_tests", "no_schema"]);
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Directory as positional arg ---

    #[tokio::test]
    async fn directory_arg_discovers_files() -> anyhow::Result<()> {
        let dir = testdata().join("positive_tests");
        let c = ValidateArgs {
            globs: vec![dir.to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        assert!(result.files_checked() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn multiple_directory_args() -> anyhow::Result<()> {
        let pos_dir = testdata().join("positive_tests");
        let no_schema_dir = testdata().join("no_schema");
        let c = ValidateArgs {
            globs: vec![
                pos_dir.to_string_lossy().to_string(),
                no_schema_dir.to_string_lossy().to_string(),
            ],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn mix_directory_and_glob_args() -> anyhow::Result<()> {
        let dir = testdata().join("positive_tests");
        let glob_pattern = testdata()
            .join("no_schema")
            .join("*.json")
            .to_string_lossy()
            .to_string();
        let c = ValidateArgs {
            globs: vec![dir.to_string_lossy().to_string(), glob_pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn malformed_json_parse_error() -> anyhow::Result<()> {
        let base = testdata().join("malformed");
        let c = ValidateArgs {
            globs: vec![base.join("*.json").to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn malformed_yaml_parse_error() -> anyhow::Result<()> {
        let base = testdata().join("malformed");
        let c = ValidateArgs {
            globs: vec![base.join("*.yaml").to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    // --- Exclude filter ---

    #[tokio::test]
    async fn exclude_filters_files_in_dir() -> anyhow::Result<()> {
        let base = testdata().join("negative_tests");
        let c = ValidateArgs {
            globs: scenario_globs(&["positive_tests", "negative_tests"]),
            exclude: vec![
                base.join("missing_name.json").to_string_lossy().to_string(),
                base.join("missing_name.toml").to_string_lossy().to_string(),
                base.join("missing_name.yaml").to_string_lossy().to_string(),
            ],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Cache options ---

    #[tokio::test]
    async fn custom_cache_dir() -> anyhow::Result<()> {
        let c = ValidateArgs {
            globs: scenario_globs(&["positive_tests"]),
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(schema_mock()), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Local schema ---

    #[tokio::test]
    async fn json_valid_with_local_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("valid.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","name":"hello"}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn yaml_valid_with_local_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("valid.yaml");
        fs::write(
            &f,
            format!(
                "# yaml-language-server: $schema={}\nname: hello\n",
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.yaml").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn missing_local_schema_errors() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let f = tmp.path().join("ref.json");
        fs::write(&f, r#"{"$schema":"/nonexistent/schema.json"}"#)?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    // --- JSON5 / JSONC tests ---

    #[tokio::test]
    async fn json5_valid_with_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("config.json5");
        fs::write(
            &f,
            format!(
                r#"{{
  // JSON5 comment
  "$schema": "{}",
  name: "hello",
}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.json5").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn jsonc_valid_with_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("config.jsonc");
        fs::write(
            &f,
            format!(
                r#"{{
  /* JSONC comment */
  "$schema": "{}",
  "name": "hello"
}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.jsonc").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Catalog-based schema matching ---

    const GH_WORKFLOW_SCHEMA: &str = r#"{
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "on": {},
            "jobs": { "type": "object" }
        },
        "required": ["on", "jobs"]
    }"#;

    fn gh_catalog_json() -> String {
        r#"{"version":1,"schemas":[{
            "name": "GitHub Workflow",
            "description": "GitHub Actions workflow",
            "url": "https://www.schemastore.org/github-workflow.json",
            "fileMatch": [
                "**/.github/workflows/*.yml",
                "**/.github/workflows/*.yaml"
            ]
        }]}"#
            .to_string()
    }

    #[tokio::test]
    async fn catalog_matches_github_workflow_valid() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir)?;
        fs::write(
            wf_dir.join("ci.yml"),
            "name: CI\non: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
        )?;

        let pattern = wf_dir.join("*.yml").to_string_lossy().to_string();
        let client = mock(&[
            (
                "https://www.schemastore.org/api/json/catalog.json",
                &gh_catalog_json(),
            ),
            (
                "https://www.schemastore.org/github-workflow.json",
                GH_WORKFLOW_SCHEMA,
            ),
        ]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn catalog_matches_github_workflow_invalid() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir)?;
        fs::write(wf_dir.join("bad.yml"), "name: Broken\n")?;

        let pattern = wf_dir.join("*.yml").to_string_lossy().to_string();
        let client = mock(&[
            (
                "https://www.schemastore.org/api/json/catalog.json",
                &gh_catalog_json(),
            ),
            (
                "https://www.schemastore.org/github-workflow.json",
                GH_WORKFLOW_SCHEMA,
            ),
        ]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(result.has_errors());
        Ok(())
    }

    #[tokio::test]
    async fn auto_discover_finds_github_workflows() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir)?;
        fs::write(
            wf_dir.join("ci.yml"),
            "name: CI\non: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
        )?;

        let client = mock(&[
            (
                "https://www.schemastore.org/api/json/catalog.json",
                &gh_catalog_json(),
            ),
            (
                "https://www.schemastore.org/github-workflow.json",
                GH_WORKFLOW_SCHEMA,
            ),
        ]);
        let c = ValidateArgs {
            globs: vec![],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: None,
            schema_cache_ttl: None,
        };

        let orig_dir = std::env::current_dir()?;
        std::env::set_current_dir(tmp.path())?;
        let result = run_with(&c, Some(client), |_| {}).await?;
        std::env::set_current_dir(orig_dir)?;

        assert!(!result.has_errors());
        Ok(())
    }

    // --- TOML tests ---

    #[tokio::test]
    async fn toml_valid_with_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("config.toml");
        fs::write(
            &f,
            format!(
                "# :schema {}\nname = \"hello\"\n",
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.toml").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Rewrite rules + // resolution ---

    #[tokio::test]
    async fn rewrite_rule_with_double_slash_resolves_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        let schemas_dir = tmp.path().join("schemas");
        fs::create_dir_all(&schemas_dir)?;
        fs::write(schemas_dir.join("test.json"), SCHEMA)?;

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://localhost:9000/" = "//schemas/"
"#,
        )?;

        let f = tmp.path().join("config.json");
        fs::write(
            &f,
            r#"{"$schema":"http://localhost:9000/test.json","name":"hello"}"#,
        )?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };

        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn double_slash_schema_resolves_relative_to_config() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        let schemas_dir = tmp.path().join("schemas");
        fs::create_dir_all(&schemas_dir)?;
        fs::write(schemas_dir.join("test.json"), SCHEMA)?;

        fs::write(tmp.path().join("lintel.toml"), "")?;

        let sub = tmp.path().join("deeply/nested");
        fs::create_dir_all(&sub)?;
        let f = sub.join("config.json");
        fs::write(&f, r#"{"$schema":"//schemas/test.json","name":"hello"}"#)?;

        let pattern = sub.join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };

        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        Ok(())
    }

    // --- Format validation override ---

    const FORMAT_SCHEMA: &str = r#"{
        "type": "object",
        "properties": {
            "link": { "type": "string", "format": "uri-reference" }
        }
    }"#;

    #[tokio::test]
    async fn format_errors_reported_without_override() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, FORMAT_SCHEMA)?;

        let f = tmp.path().join("data.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","link":"not a valid {{uri}}"}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("data.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(
            result.has_errors(),
            "expected format error without override"
        );
        Ok(())
    }

    #[tokio::test]
    async fn format_errors_suppressed_with_override() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, FORMAT_SCHEMA)?;

        let f = tmp.path().join("data.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","link":"not a valid {{uri}}"}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        // Use **/data.json to match the absolute path from the tempdir.
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["**/data.json"]
validate_formats = false
"#,
        )?;

        let pattern = tmp.path().join("data.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(
            !result.has_errors(),
            "expected no errors with validate_formats = false override"
        );
        Ok(())
    }

    // --- Unrecognized extension handling ---

    #[tokio::test]
    async fn unrecognized_extension_skipped_without_catalog() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("config.nix"), r#"{"name":"hello"}"#)?;

        let pattern = tmp.path().join("config.nix").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(mock(&[])), |_| {}).await?;
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn unrecognized_extension_parsed_when_catalog_matches() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        // File has .cfg extension (unrecognized) but content is valid JSON
        fs::write(
            tmp.path().join("myapp.cfg"),
            r#"{"name":"hello","on":"push","jobs":{"build":{}}}"#,
        )?;

        let catalog_json = r#"{"version":1,"schemas":[{
            "name": "MyApp Config",
            "description": "MyApp configuration",
            "url": "https://example.com/myapp.schema.json",
            "fileMatch": ["*.cfg"]
        }]}"#;
        let schema =
            r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;

        let pattern = tmp.path().join("myapp.cfg").to_string_lossy().to_string();
        let client = mock(&[
            (
                "https://www.schemastore.org/api/json/catalog.json",
                catalog_json,
            ),
            ("https://example.com/myapp.schema.json", schema),
        ]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn unrecognized_extension_unparseable_skipped() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        // File matches catalog but content isn't parseable by any format
        fs::write(
            tmp.path().join("myapp.cfg"),
            "{ pkgs, ... }: { packages = [ pkgs.git ]; }",
        )?;

        let catalog_json = r#"{"version":1,"schemas":[{
            "name": "MyApp Config",
            "description": "MyApp configuration",
            "url": "https://example.com/myapp.schema.json",
            "fileMatch": ["*.cfg"]
        }]}"#;

        let pattern = tmp.path().join("myapp.cfg").to_string_lossy().to_string();
        let client = mock(&[(
            "https://www.schemastore.org/api/json/catalog.json",
            catalog_json,
        )]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn unrecognized_extension_invalid_against_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_tmp = tempfile::tempdir()?;
        // File has .cfg extension, content is valid JSON but fails schema validation
        fs::write(tmp.path().join("myapp.cfg"), r#"{"wrong":"field"}"#)?;

        let catalog_json = r#"{"version":1,"schemas":[{
            "name": "MyApp Config",
            "description": "MyApp configuration",
            "url": "https://example.com/myapp.schema.json",
            "fileMatch": ["*.cfg"]
        }]}"#;
        let schema =
            r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;

        let pattern = tmp.path().join("myapp.cfg").to_string_lossy().to_string();
        let client = mock(&[
            (
                "https://www.schemastore.org/api/json/catalog.json",
                catalog_json,
            ),
            ("https://example.com/myapp.schema.json", schema),
        ]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: false,
            config_dir: Some(tmp.path().to_path_buf()),
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(result.has_errors());
        assert_eq!(result.files_checked(), 1);
        Ok(())
    }

    // --- Validation cache ---

    #[tokio::test]
    async fn validation_cache_hit_skips_revalidation() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA)?;

        let f = tmp.path().join("valid.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","name":"hello"}}"#,
                schema_path.to_string_lossy()
            ),
        )?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();

        // First run: force_validation = false so results get cached
        let c = ValidateArgs {
            globs: vec![pattern.clone()],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: false,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let mut first_statuses = Vec::new();
        let result = run_with(&c, Some(mock(&[])), |cf| {
            first_statuses.push(cf.validation_cache_status);
        })
        .await?;
        assert!(!result.has_errors());
        assert!(result.files_checked() > 0);

        // Verify the first run recorded a validation cache miss
        assert!(
            first_statuses.contains(&Some(ValidationCacheStatus::Miss)),
            "expected at least one validation cache miss on first run"
        );

        // Second run: same file, same schema — should hit validation cache
        let mut second_statuses = Vec::new();
        let result = run_with(&c, Some(mock(&[])), |cf| {
            second_statuses.push(cf.validation_cache_status);
        })
        .await?;
        assert!(!result.has_errors());

        // Verify the second run got a validation cache hit
        assert!(
            second_statuses.contains(&Some(ValidationCacheStatus::Hit)),
            "expected at least one validation cache hit on second run"
        );
        Ok(())
    }

    // --- clean_error_message ---

    #[test]
    fn clean_strips_anyof_value() {
        let msg =
            r#"{"type":"bad"} is not valid under any of the schemas listed in the 'anyOf' keyword"#;
        assert_eq!(
            clean_error_message(msg.to_string()),
            "not valid under any of the schemas listed in the 'anyOf' keyword"
        );
    }

    #[test]
    fn clean_strips_oneof_value() {
        let msg = r#"{"runs-on":"ubuntu-latest","steps":[]} is not valid under any of the schemas listed in the 'oneOf' keyword"#;
        assert_eq!(
            clean_error_message(msg.to_string()),
            "not valid under any of the schemas listed in the 'oneOf' keyword"
        );
    }

    #[test]
    fn clean_strips_long_value() {
        let long_value = "x".repeat(5000);
        let suffix = " is not valid under any of the schemas listed in the 'anyOf' keyword";
        let msg = format!("{long_value}{suffix}");
        assert_eq!(
            clean_error_message(msg),
            "not valid under any of the schemas listed in the 'anyOf' keyword"
        );
    }

    #[test]
    fn clean_preserves_type_error() {
        let msg = r#"12345 is not of types "null", "string""#;
        assert_eq!(clean_error_message(msg.to_string()), msg);
    }

    #[test]
    fn clean_preserves_required_property() {
        let msg = "\"name\" is a required property";
        assert_eq!(clean_error_message(msg.to_string()), msg);
    }

    /// Schemas whose URI contains a fragment (e.g. `…/draft-07/schema#`)
    /// must compile without error — the fragment is stripped before being
    /// used as the base URI for `$ref` resolution.
    #[tokio::test]
    async fn schema_uri_with_fragment_compiles() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        // A minimal draft-07 schema whose `$schema` ends with `#`.
        let schema_body = r#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        }"#;

        let schema_url = "http://json-schema.org/draft-07/schema#";

        let f = tmp.path().join("data.json");
        fs::write(
            &f,
            format!(r#"{{ "$schema": "{schema_url}", "name": "hello" }}"#),
        )?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let client = mock(&[(
            // The schema URI with fragment — exactly as the `$schema` value appears.
            schema_url,
            schema_body,
        )]);
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&c, Some(client), |_| {}).await?;
        assert!(
            !result.has_errors(),
            "schema URI with fragment should not cause compilation error"
        );
        assert_eq!(result.files_checked(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn relative_ref_in_local_schema() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;

        // Referenced schema with a "name" string definition
        std::fs::write(tmp.path().join("defs.json"), r#"{"type": "string"}"#)?;

        // Main schema that uses a relative $ref
        let schema_path = tmp.path().join("schema.json");
        std::fs::write(
            &schema_path,
            r#"{
                "type": "object",
                "properties": {
                    "name": { "$ref": "./defs.json" }
                },
                "required": ["name"]
            }"#,
        )?;

        // Valid data file pointing to the local schema
        let schema_uri = schema_path.to_string_lossy();
        std::fs::write(
            tmp.path().join("data.json"),
            format!(r#"{{ "$schema": "{schema_uri}", "name": "hello" }}"#),
        )?;

        // Invalid data file (name should be a string per defs.json)
        std::fs::write(
            tmp.path().join("bad.json"),
            format!(r#"{{ "$schema": "{schema_uri}", "name": 42 }}"#),
        )?;

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let args = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            force_schema_fetch: true,
            force_validation: true,
            no_catalog: true,
            config_dir: None,
            schema_cache_ttl: None,
        };
        let result = run_with(&args, Some(mock(&[])), |_| {}).await?;

        // The invalid file should produce an error (name is 42, not a string)
        assert!(result.has_errors());
        // Exactly one file should have errors (bad.json), the other (data.json) should pass
        assert_eq!(result.errors.len(), 1);
        Ok(())
    }
}
