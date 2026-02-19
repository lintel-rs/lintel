use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;
use serde_json::Value;

use crate::catalog::{self, CompiledCatalog};
use crate::config;
use crate::diagnostics::{
    find_instance_path_offset, FileDiagnostic, ParseDiagnostic, ValidationDiagnostic,
};
use crate::discover;
use crate::parsers::{self, FileFormat, JsoncParser, Parser};
use crate::registry;
use crate::retriever::{default_cache_dir, CacheStatus, HttpClient, SchemaCache};

pub struct ValidateArgs {
    /// Glob patterns to find files (empty = auto-discover)
    pub globs: Vec<String>,

    /// Exclude files matching these globs (repeatable)
    pub exclude: Vec<String>,

    /// Cache directory for remote schemas
    pub cache_dir: Option<String>,

    /// Disable schema caching
    pub no_cache: bool,

    /// Disable SchemaStore catalog matching
    pub no_catalog: bool,

    /// Force file format for all inputs
    pub format: Option<parsers::FileFormat>,

    /// Directory to search for `lintel.toml` (defaults to cwd)
    pub config_dir: Option<PathBuf>,
}

/// A single lint error produced during validation.
pub enum LintError {
    Parse(ParseDiagnostic),
    Validation(ValidationDiagnostic),
    File(FileDiagnostic),
}

impl LintError {
    /// File path associated with this error.
    pub fn path(&self) -> &str {
        match self {
            LintError::Parse(d) => d.src.name(),
            LintError::Validation(d) => &d.path,
            LintError::File(d) => &d.path,
        }
    }

    /// Human-readable error message.
    pub fn message(&self) -> &str {
        match self {
            LintError::Parse(d) => &d.message,
            LintError::Validation(d) => &d.message,
            LintError::File(d) => &d.message,
        }
    }

    /// Byte offset in the source file (for sorting).
    fn offset(&self) -> usize {
        match self {
            LintError::Parse(d) => d.span.offset(),
            LintError::Validation(d) => d.span.offset(),
            LintError::File(_) => 0,
        }
    }

    /// Convert into a boxed miette Diagnostic for rich rendering.
    pub fn into_diagnostic(self) -> Box<dyn miette::Diagnostic + Send + Sync> {
        match self {
            LintError::Parse(d) => Box::new(d),
            LintError::Validation(d) => Box::new(d),
            LintError::File(d) => Box::new(d),
        }
    }
}

/// A file that was checked and the schema it resolved to.
pub struct CheckedFile {
    pub path: String,
    pub schema: String,
    /// `None` for local schemas and builtins; `Some` for remote schemas.
    pub cache_status: Option<CacheStatus>,
}

/// Result of a validation run.
pub struct ValidateResult {
    pub errors: Vec<LintError>,
    pub checked: Vec<CheckedFile>,
}

impl ValidateResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn files_checked(&self) -> usize {
        self.checked.len()
    }
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
fn load_config(search_dir: Option<&Path>) -> (config::Config, PathBuf, Option<PathBuf>) {
    let start_dir = match search_dir {
        Some(d) => d.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return (config::Config::default(), PathBuf::from("."), None),
        },
    };

    let Some(config_path) = config::find_config_path(&start_dir) else {
        return (config::Config::default(), start_dir, None);
    };

    let dir = config_path.parent().unwrap_or(&start_dir).to_path_buf();
    let cfg = config::find_and_load(&start_dir)
        .ok()
        .flatten()
        .unwrap_or_default();
    (cfg, dir, Some(config_path))
}

// ---------------------------------------------------------------------------
// File collection
// ---------------------------------------------------------------------------

/// Collect input files from globs/directories, applying exclude filters.
fn collect_files(globs: &[String], exclude: &[String]) -> Result<Vec<PathBuf>> {
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
// lintel.toml self-validation
// ---------------------------------------------------------------------------

/// Validate `lintel.toml` against its built-in schema.
fn validate_config(
    config_path: &Path,
    errors: &mut Vec<LintError>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let config_value: Value = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", config_path.display()))?;
    let schema_value = config::schema();
    if let Ok(validator) = jsonschema::options().build(&schema_value) {
        let path_str = config_path.display().to_string();
        for error in validator.iter_errors(&config_value) {
            let ip = error.instance_path().to_string();
            let offset = find_instance_path_offset(&content, &ip);
            errors.push(LintError::Validation(ValidationDiagnostic {
                src: miette::NamedSource::new(&path_str, content.clone()),
                span: offset.into(),
                path: path_str.clone(),
                instance_path: ip,
                message: error.to_string(),
            }));
        }
        let cf = CheckedFile {
            path: path_str,
            schema: "(builtin)".to_string(),
            cache_status: None,
        };
        on_check(&cf);
        checked.push(cf);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 1: Parse files and resolve schema URIs
// ---------------------------------------------------------------------------

/// Try parsing content with each known format, returning the first success.
///
/// JSONC is tried first (superset of JSON, handles comments), then YAML and
/// TOML which cover the most common config formats, followed by the rest.
fn try_parse_all(content: &str, file_name: &str) -> Option<(parsers::FileFormat, Value)> {
    use parsers::FileFormat::*;
    const FORMATS: [parsers::FileFormat; 6] = [Jsonc, Yaml, Toml, Json, Json5, Markdown];

    for fmt in FORMATS {
        let parser = parsers::parser_for(fmt);
        if let Ok(val) = parser.parse(content, file_name) {
            return Some((fmt, val));
        }
    }
    None
}

/// Parse each file, extract its schema URI, apply rewrites, and group by
/// resolved schema URI.
fn parse_and_group_files(
    files: &[PathBuf],
    args: &ValidateArgs,
    config: &config::Config,
    config_dir: &Path,
    compiled_catalogs: &[CompiledCatalog],
    errors: &mut Vec<LintError>,
) -> BTreeMap<String, Vec<ParsedFile>> {
    let mut schema_groups: BTreeMap<String, Vec<ParsedFile>> = BTreeMap::new();

    for path in files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(LintError::File(FileDiagnostic {
                    path: path.display().to_string(),
                    message: format!("failed to read: {e}"),
                }));
                continue;
            }
        };

        let path_str = path.display().to_string();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path_str);

        let detected_format = args.format.or_else(|| parsers::detect_format(path));

        // For unrecognized extensions, only proceed if a catalog or config mapping matches.
        if detected_format.is_none() {
            let has_match = config.find_schema_mapping(&path_str, file_name).is_some()
                || compiled_catalogs
                    .iter()
                    .any(|cat| cat.find_schema(&path_str, file_name).is_some());
            if !has_match {
                continue;
            }
        }

        // Parse the file content.
        let (parser, instance): (Box<dyn Parser>, Value) = if let Some(fmt) = detected_format {
            // Known format — parse with detected/overridden format.
            let parser = parsers::parser_for(fmt);
            match parser.parse(&content, &path_str) {
                Ok(val) => (parser, val),
                Err(parse_err) => {
                    // JSONC fallback for .json files that match a catalog entry.
                    if fmt == FileFormat::Json
                        && compiled_catalogs
                            .iter()
                            .any(|cat| cat.find_schema(&path_str, file_name).is_some())
                    {
                        match JsoncParser.parse(&content, &path_str) {
                            Ok(val) => (parsers::parser_for(FileFormat::Jsonc), val),
                            Err(jsonc_err) => {
                                errors.push(LintError::Parse(jsonc_err));
                                continue;
                            }
                        }
                    } else {
                        errors.push(LintError::Parse(parse_err));
                        continue;
                    }
                }
            }
        } else {
            // Unrecognized extension with catalog/config match — try all parsers.
            match try_parse_all(&content, &path_str) {
                Some((fmt, val)) => (parsers::parser_for(fmt), val),
                None => continue,
            }
        };

        // Skip markdown files with no frontmatter
        if instance.is_null() {
            continue;
        }

        // Schema resolution priority:
        // 1. Inline $schema / YAML modeline (always wins)
        // 2. Custom schema mappings from lintel.toml [schemas]
        // 3. Catalog matching (SchemaStore + additional registries)
        let schema_uri = parser
            .extract_schema_uri(&content, &instance)
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
            continue;
        };

        // Keep original URI for override matching (before rewrites)
        let original_schema_uri = schema_uri.clone();

        // Apply rewrite rules, then resolve // paths relative to lintel.toml
        let schema_uri = config::apply_rewrites(&schema_uri, &config.rewrite);
        let schema_uri = config::resolve_double_slash(&schema_uri, config_dir);

        // Resolve relative local paths against the file's parent directory.
        let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
        let schema_uri = if !is_remote {
            path.parent()
                .map(|parent| parent.join(&schema_uri).to_string_lossy().to_string())
                .unwrap_or(schema_uri)
        } else {
            schema_uri
        };

        schema_groups
            .entry(schema_uri)
            .or_default()
            .push(ParsedFile {
                path: path_str,
                content,
                instance,
                original_schema_uri,
            });
    }

    schema_groups
}

// ---------------------------------------------------------------------------
// Phase 2: Schema fetching, compilation, and instance validation
// ---------------------------------------------------------------------------

/// Fetch a schema by URI, returning its parsed JSON and cache status.
fn fetch_schema<C: HttpClient>(
    schema_uri: &str,
    retriever: &SchemaCache<C>,
    group: &[ParsedFile],
    errors: &mut Vec<LintError>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) -> Option<(Value, Option<CacheStatus>)> {
    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");

    let result: Result<(Value, Option<CacheStatus>), String> = if is_remote {
        retriever
            .fetch(schema_uri)
            .map(|(v, status)| (v, Some(status)))
            .map_err(|e| format!("failed to fetch schema: {schema_uri}: {e}"))
    } else {
        fs::read_to_string(schema_uri)
            .map_err(|e| format!("failed to read local schema {schema_uri}: {e}"))
            .and_then(|content| {
                serde_json::from_str::<Value>(&content)
                    .map(|v| (v, None))
                    .map_err(|e| format!("failed to parse local schema {schema_uri}: {e}"))
            })
    };

    match result {
        Ok(value) => Some(value),
        Err(message) => {
            report_group_error(&message, schema_uri, None, group, errors, checked, on_check);
            None
        }
    }
}

/// Report the same error for every file in a schema group.
fn report_group_error(
    message: &str,
    schema_uri: &str,
    cache_status: Option<CacheStatus>,
    group: &[ParsedFile],
    errors: &mut Vec<LintError>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for pf in group {
        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
        };
        on_check(&cf);
        checked.push(cf);
        errors.push(LintError::File(FileDiagnostic {
            path: pf.path.clone(),
            message: message.to_string(),
        }));
    }
}

/// Mark every file in a group as checked (no errors).
fn mark_group_checked(
    schema_uri: &str,
    cache_status: Option<CacheStatus>,
    group: &[ParsedFile],
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for pf in group {
        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
        };
        on_check(&cf);
        checked.push(cf);
    }
}

/// Validate all files in a group against an already-compiled validator.
fn validate_group(
    validator: &jsonschema::Validator,
    schema_uri: &str,
    cache_status: Option<CacheStatus>,
    group: &[ParsedFile],
    errors: &mut Vec<LintError>,
    checked: &mut Vec<CheckedFile>,
    on_check: &mut impl FnMut(&CheckedFile),
) {
    for pf in group {
        let cf = CheckedFile {
            path: pf.path.clone(),
            schema: schema_uri.to_string(),
            cache_status,
        };
        on_check(&cf);
        checked.push(cf);

        for error in validator.iter_errors(&pf.instance) {
            let ip = error.instance_path().to_string();
            let offset = find_instance_path_offset(&pf.content, &ip);
            errors.push(LintError::Validation(ValidationDiagnostic {
                src: miette::NamedSource::new(&pf.path, pf.content.clone()),
                span: offset.into(),
                path: pf.path.clone(),
                instance_path: ip,
                message: error.to_string(),
            }));
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub async fn run<C: HttpClient>(args: &ValidateArgs, client: C) -> Result<ValidateResult> {
    run_with(args, client, |_| {}).await
}

/// Like [`run`], but calls `on_check` each time a file is checked, allowing
/// callers to stream progress (e.g. verbose output) as files are processed.
pub async fn run_with<C: HttpClient>(
    args: &ValidateArgs,
    client: C,
    mut on_check: impl FnMut(&CheckedFile),
) -> Result<ValidateResult> {
    let cache_dir = if args.no_cache {
        None
    } else {
        Some(
            args.cache_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(default_cache_dir),
        )
    };
    let retriever = SchemaCache::new(cache_dir, client.clone());

    let (config, config_dir, config_path) = load_config(args.config_dir.as_deref());
    let files = collect_files(&args.globs, &args.exclude)?;

    let mut compiled_catalogs = Vec::new();

    if !args.no_catalog {
        // Default Lintel catalog (github:lintel-rs/catalog)
        match registry::fetch(&retriever, registry::DEFAULT_REGISTRY) {
            Ok(cat) => compiled_catalogs.push(CompiledCatalog::compile(&cat)),
            Err(e) => {
                eprintln!(
                    "warning: failed to fetch default catalog {}: {e}",
                    registry::DEFAULT_REGISTRY
                );
            }
        }
        // SchemaStore catalog
        match catalog::fetch_catalog(&retriever) {
            Ok(cat) => compiled_catalogs.push(CompiledCatalog::compile(&cat)),
            Err(e) => {
                eprintln!("warning: failed to fetch SchemaStore catalog: {e}");
            }
        }
        // Additional registries from lintel.toml
        for registry_url in &config.registries {
            match registry::fetch(&retriever, registry_url) {
                Ok(cat) => compiled_catalogs.push(CompiledCatalog::compile(&cat)),
                Err(e) => {
                    eprintln!("warning: failed to fetch registry {registry_url}: {e}");
                }
            }
        }
    }

    let mut errors: Vec<LintError> = Vec::new();
    let mut checked: Vec<CheckedFile> = Vec::new();

    // Validate lintel.toml against its own schema
    if let Some(config_path) = config_path {
        validate_config(&config_path, &mut errors, &mut checked, &mut on_check)?;
    }

    // Phase 1: Parse files and resolve schema URIs
    let schema_groups = parse_and_group_files(
        &files,
        args,
        &config,
        &config_dir,
        &compiled_catalogs,
        &mut errors,
    );

    // Phase 2: Compile each schema once and validate all matching files
    for (schema_uri, group) in &schema_groups {
        let Some((schema_value, cache_status)) = fetch_schema(
            schema_uri,
            &retriever,
            group,
            &mut errors,
            &mut checked,
            &mut on_check,
        ) else {
            continue;
        };

        // If ANY file in the group matches a `validate_formats = false` override,
        // disable format validation for the whole group (they share one compiled validator).
        let validate_formats = group.iter().all(|pf| {
            config
                .should_validate_formats(&pf.path, &[&pf.original_schema_uri, schema_uri.as_str()])
        });

        let validator = match jsonschema::async_options()
            .with_retriever(retriever.clone())
            .should_validate_formats(validate_formats)
            .build(&schema_value)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                // When format validation is disabled and the compilation error
                // is a uri-reference issue (e.g. Rust-style $ref paths in
                // vector.json), skip validation silently.
                if !validate_formats && e.to_string().contains("uri-reference") {
                    mark_group_checked(
                        schema_uri,
                        cache_status,
                        group,
                        &mut checked,
                        &mut on_check,
                    );
                    continue;
                }
                report_group_error(
                    &format!("failed to compile schema: {e}"),
                    schema_uri,
                    cache_status,
                    group,
                    &mut errors,
                    &mut checked,
                    &mut on_check,
                );
                continue;
            }
        };

        validate_group(
            &validator,
            schema_uri,
            cache_status,
            group,
            &mut errors,
            &mut checked,
            &mut on_check,
        );
    }

    // Sort errors for deterministic output (by path, then by span offset)
    errors.sort_by(|a, b| {
        a.path()
            .cmp(b.path())
            .then_with(|| a.offset().cmp(&b.offset()))
    });

    Ok(ValidateResult { errors, checked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retriever::HttpClient;
    use std::collections::HashMap;
    use std::error::Error;
    use std::path::Path;

    #[derive(Clone)]
    struct MockClient(HashMap<String, String>);

    impl HttpClient for MockClient {
        fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
            self.0
                .get(uri)
                .cloned()
                .ok_or_else(|| format!("mock: no response for {uri}").into())
        }
    }

    fn mock(entries: &[(&str, &str)]) -> MockClient {
        MockClient(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
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
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        }
    }

    const SCHEMA: &str =
        r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#;

    fn schema_mock() -> MockClient {
        mock(&[("https://example.com/schema.json", SCHEMA)])
    }

    // --- Directory scanning tests ---

    #[tokio::test]
    async fn no_matching_files() {
        let tmp = tempfile::tempdir().unwrap();
        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn dir_all_valid() {
        let c = args_for_dirs(&["positive_tests"]);
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn dir_all_invalid() {
        let c = args_for_dirs(&["negative_tests"]);
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(result.has_errors());
    }

    #[tokio::test]
    async fn dir_mixed_valid_and_invalid() {
        let c = args_for_dirs(&["positive_tests", "negative_tests"]);
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(result.has_errors());
    }

    #[tokio::test]
    async fn dir_no_schemas_skipped() {
        let c = args_for_dirs(&["no_schema"]);
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn dir_valid_with_no_schema_files() {
        let c = args_for_dirs(&["positive_tests", "no_schema"]);
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
    }

    // --- Directory as positional arg ---

    #[tokio::test]
    async fn directory_arg_discovers_files() {
        let dir = testdata().join("positive_tests");
        let c = ValidateArgs {
            globs: vec![dir.to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
        assert!(result.files_checked() > 0);
    }

    #[tokio::test]
    async fn multiple_directory_args() {
        let pos_dir = testdata().join("positive_tests");
        let no_schema_dir = testdata().join("no_schema");
        let c = ValidateArgs {
            globs: vec![
                pos_dir.to_string_lossy().to_string(),
                no_schema_dir.to_string_lossy().to_string(),
            ],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn mix_directory_and_glob_args() {
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
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn malformed_json_parse_error() {
        let base = testdata().join("malformed");
        let c = ValidateArgs {
            globs: vec![base.join("*.json").to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(result.has_errors());
    }

    #[tokio::test]
    async fn malformed_yaml_parse_error() {
        let base = testdata().join("malformed");
        let c = ValidateArgs {
            globs: vec![base.join("*.yaml").to_string_lossy().to_string()],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(result.has_errors());
    }

    // --- Exclude filter ---

    #[tokio::test]
    async fn exclude_filters_files_in_dir() {
        let base = testdata().join("negative_tests");
        let c = ValidateArgs {
            globs: scenario_globs(&["positive_tests", "negative_tests"]),
            exclude: vec![
                base.join("missing_name.json").to_string_lossy().to_string(),
                base.join("missing_name.toml").to_string_lossy().to_string(),
                base.join("missing_name.yaml").to_string_lossy().to_string(),
            ],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());
    }

    // --- Cache options ---

    #[tokio::test]
    async fn custom_cache_dir() {
        let cache_tmp = tempfile::tempdir().unwrap();
        let c = ValidateArgs {
            globs: scenario_globs(&["positive_tests"]),
            exclude: vec![],
            cache_dir: Some(cache_tmp.path().to_string_lossy().to_string()),
            no_cache: false,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, schema_mock()).await.unwrap();
        assert!(!result.has_errors());

        // Schema was fetched once and cached
        let entries: Vec<_> = fs::read_dir(cache_tmp.path()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    // --- Local schema ---

    #[tokio::test]
    async fn json_valid_with_local_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA).unwrap();

        let f = tmp.path().join("valid.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","name":"hello"}}"#,
                schema_path.to_string_lossy()
            ),
        )
        .unwrap();

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn yaml_valid_with_local_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA).unwrap();

        let f = tmp.path().join("valid.yaml");
        fs::write(
            &f,
            format!(
                "# yaml-language-server: $schema={}\nname: hello\n",
                schema_path.to_string_lossy()
            ),
        )
        .unwrap();

        let pattern = tmp.path().join("*.yaml").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn missing_local_schema_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("ref.json");
        fs::write(&f, r#"{"$schema":"/nonexistent/schema.json"}"#).unwrap();

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(result.has_errors());
    }

    // --- JSON5 / JSONC tests ---

    #[tokio::test]
    async fn json5_valid_with_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA).unwrap();

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
        )
        .unwrap();

        let pattern = tmp.path().join("*.json5").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn jsonc_valid_with_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA).unwrap();

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
        )
        .unwrap();

        let pattern = tmp.path().join("*.jsonc").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
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
        r#"{"schemas":[{
            "name": "GitHub Workflow",
            "url": "https://www.schemastore.org/github-workflow.json",
            "fileMatch": [
                "**/.github/workflows/*.yml",
                "**/.github/workflows/*.yaml"
            ]
        }]}"#
            .to_string()
    }

    #[tokio::test]
    async fn catalog_matches_github_workflow_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("ci.yml"),
            "name: CI\non: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
        )
        .unwrap();

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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: None,
        };
        let result = run(&c, client).await.unwrap();
        assert!(!result.has_errors());
    }

    #[tokio::test]
    async fn catalog_matches_github_workflow_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(wf_dir.join("bad.yml"), "name: Broken\n").unwrap();

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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: None,
        };
        let result = run(&c, client).await.unwrap();
        assert!(result.has_errors());
    }

    #[tokio::test]
    async fn auto_discover_finds_github_workflows() {
        let tmp = tempfile::tempdir().unwrap();
        let wf_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(
            wf_dir.join("ci.yml"),
            "name: CI\non: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
        )
        .unwrap();

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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: None,
        };

        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = run(&c, client).await.unwrap();
        std::env::set_current_dir(orig_dir).unwrap();

        assert!(!result.has_errors());
    }

    // --- TOML tests ---

    #[tokio::test]
    async fn toml_valid_with_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, SCHEMA).unwrap();

        let f = tmp.path().join("config.toml");
        fs::write(
            &f,
            format!(
                "# $schema: {}\nname = \"hello\"\n",
                schema_path.to_string_lossy()
            ),
        )
        .unwrap();

        let pattern = tmp.path().join("*.toml").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: None,
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    // --- Rewrite rules + // resolution ---

    #[tokio::test]
    async fn rewrite_rule_with_double_slash_resolves_schema() {
        let tmp = tempfile::tempdir().unwrap();

        let schemas_dir = tmp.path().join("schemas");
        fs::create_dir_all(&schemas_dir).unwrap();
        fs::write(&schemas_dir.join("test.json"), SCHEMA).unwrap();

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://localhost:9000/" = "//schemas/"
"#,
        )
        .unwrap();

        let f = tmp.path().join("config.json");
        fs::write(
            &f,
            r#"{"$schema":"http://localhost:9000/test.json","name":"hello"}"#,
        )
        .unwrap();

        let pattern = tmp.path().join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };

        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 2); // lintel.toml + config.json
    }

    #[tokio::test]
    async fn double_slash_schema_resolves_relative_to_config() {
        let tmp = tempfile::tempdir().unwrap();

        let schemas_dir = tmp.path().join("schemas");
        fs::create_dir_all(&schemas_dir).unwrap();
        fs::write(&schemas_dir.join("test.json"), SCHEMA).unwrap();

        fs::write(tmp.path().join("lintel.toml"), "").unwrap();

        let sub = tmp.path().join("deeply/nested");
        fs::create_dir_all(&sub).unwrap();
        let f = sub.join("config.json");
        fs::write(&f, r#"{"$schema":"//schemas/test.json","name":"hello"}"#).unwrap();

        let pattern = sub.join("*.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };

        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
    }

    // --- Format validation override ---

    const FORMAT_SCHEMA: &str = r#"{
        "type": "object",
        "properties": {
            "link": { "type": "string", "format": "uri-reference" }
        }
    }"#;

    #[tokio::test]
    async fn format_errors_reported_without_override() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, FORMAT_SCHEMA).unwrap();

        let f = tmp.path().join("data.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","link":"not a valid {{uri}}"}}"#,
                schema_path.to_string_lossy()
            ),
        )
        .unwrap();

        let pattern = tmp.path().join("data.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(
            result.has_errors(),
            "expected format error without override"
        );
    }

    #[tokio::test]
    async fn format_errors_suppressed_with_override() {
        let tmp = tempfile::tempdir().unwrap();
        let schema_path = tmp.path().join("schema.json");
        fs::write(&schema_path, FORMAT_SCHEMA).unwrap();

        let f = tmp.path().join("data.json");
        fs::write(
            &f,
            format!(
                r#"{{"$schema":"{}","link":"not a valid {{uri}}"}}"#,
                schema_path.to_string_lossy()
            ),
        )
        .unwrap();

        // Use **/data.json to match the absolute path from the tempdir.
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["**/data.json"]
validate_formats = false
"#,
        )
        .unwrap();

        let pattern = tmp.path().join("data.json").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(
            !result.has_errors(),
            "expected no errors with validate_formats = false override"
        );
    }

    // --- Unrecognized extension handling ---

    #[tokio::test]
    async fn unrecognized_extension_skipped_without_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("config.nix"), r#"{"name":"hello"}"#).unwrap();

        let pattern = tmp.path().join("config.nix").to_string_lossy().to_string();
        let c = ValidateArgs {
            globs: vec![pattern],
            exclude: vec![],
            cache_dir: None,
            no_cache: true,
            no_catalog: true,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, mock(&[])).await.unwrap();
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 0);
    }

    #[tokio::test]
    async fn unrecognized_extension_parsed_when_catalog_matches() {
        let tmp = tempfile::tempdir().unwrap();
        // File has .cfg extension (unrecognized) but content is valid JSON
        fs::write(
            tmp.path().join("myapp.cfg"),
            r#"{"name":"hello","on":"push","jobs":{"build":{}}}"#,
        )
        .unwrap();

        let catalog_json = r#"{"schemas":[{
            "name": "MyApp Config",
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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, client).await.unwrap();
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 1);
    }

    #[tokio::test]
    async fn unrecognized_extension_unparseable_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        // File matches catalog but content isn't parseable by any format
        fs::write(
            tmp.path().join("myapp.cfg"),
            "{ pkgs, ... }: { packages = [ pkgs.git ]; }",
        )
        .unwrap();

        let catalog_json = r#"{"schemas":[{
            "name": "MyApp Config",
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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, client).await.unwrap();
        assert!(!result.has_errors());
        assert_eq!(result.files_checked(), 0);
    }

    #[tokio::test]
    async fn unrecognized_extension_invalid_against_schema() {
        let tmp = tempfile::tempdir().unwrap();
        // File has .cfg extension, content is valid JSON but fails schema validation
        fs::write(tmp.path().join("myapp.cfg"), r#"{"wrong":"field"}"#).unwrap();

        let catalog_json = r#"{"schemas":[{
            "name": "MyApp Config",
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
            cache_dir: None,
            no_cache: true,
            no_catalog: false,
            format: None,
            config_dir: Some(tmp.path().to_path_buf()),
        };
        let result = run(&c, client).await.unwrap();
        assert!(result.has_errors());
        assert_eq!(result.files_checked(), 1);
    }
}
