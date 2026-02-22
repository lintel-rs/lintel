use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bpaf::Bpaf;
use lintel_cli_common::CLIGlobalOptions;

use lintel_validate::config;
use lintel_validate::parsers;
use lintel_validate::retriever::{CacheStatus, SchemaCache};
use lintel_validate::validate;
use lintel_validate::validation_cache;

#[derive(Debug, Clone, Bpaf)]
pub enum CacheCommand {
    #[bpaf(command("inspect-schema"))]
    /// Show cache file info for a schema URL
    InspectSchema(#[bpaf(external(inspect_schema_args))] InspectSchemaArgs),

    #[bpaf(command("trace"))]
    /// Trace cache involvement for a file's validation
    Trace(#[bpaf(external(trace_args))] TraceArgs),
}

#[derive(Debug, Clone, Bpaf)]
pub struct InspectSchemaArgs {
    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    /// Schema URL to inspect
    #[bpaf(positional("URL"))]
    pub url: String,
}

#[derive(Debug, Clone, Bpaf)]
pub struct TraceArgs {
    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    /// Schema cache TTL (e.g. "12h", "30m", "1d"); default 12h
    #[bpaf(long("schema-cache-ttl"), argument("DURATION"))]
    pub schema_cache_ttl: Option<String>,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,

    /// File to trace
    #[bpaf(positional("FILE"))]
    pub file: String,
}

pub async fn run(cmd: CacheCommand, _global: &CLIGlobalOptions) -> Result<bool> {
    match cmd {
        CacheCommand::InspectSchema(args) => {
            inspect_schema(args)?;
            Ok(false)
        }
        CacheCommand::Trace(args) => {
            trace(args).await?;
            Ok(false)
        }
    }
}

fn inspect_schema(args: InspectSchemaArgs) -> Result<()> {
    let hash = SchemaCache::hash_uri(&args.url);
    let cache_dir = args
        .cache_dir
        .map_or_else(lintel_validate::retriever::ensure_cache_dir, PathBuf::from);
    let cache_path = cache_dir.join(format!("{hash}.json"));

    println!("URL:        {}", args.url);
    println!("Hash:       {hash}");
    println!("Cache file: {}", cache_path.display());

    if !cache_path.exists() {
        println!("Status:     not cached");
        return Ok(());
    }

    let meta = fs::metadata(&cache_path)?;
    println!("Size:       {} bytes", meta.len());
    print_modified_age(&meta);

    let content = fs::read_to_string(&cache_path)
        .with_context(|| format!("failed to read cache file: {}", cache_path.display()))?;

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
        let preview = format_json_preview(&value);
        println!("Valid JSON: yes");
        println!("Preview:    {preview}");
    } else {
        println!("Valid JSON: no");
        let first_line = content.lines().next().unwrap_or("");
        if first_line.len() > 80 {
            println!("First line: {}...", &first_line[..80]);
        } else {
            println!("First line: {first_line}");
        }
    }

    Ok(())
}

async fn trace(args: TraceArgs) -> Result<()> {
    let file_path = Path::new(&args.file);
    if !file_path.exists() {
        bail!("file not found: {}", args.file);
    }

    let content =
        fs::read_to_string(file_path).with_context(|| format!("failed to read {}", args.file))?;

    let path_str = file_path.display().to_string();
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    println!("file: {path_str}");

    // Set up schema cache
    let mut builder = SchemaCache::builder();
    if let Some(dir) = &args.cache_dir {
        builder = builder.cache_dir(PathBuf::from(dir));
    }
    if let Some(ref s) = args.schema_cache_ttl {
        let ttl = humantime::parse_duration(s)
            .unwrap_or_else(|e| panic!("invalid --schema-cache-ttl value '{s}': {e}"));
        builder = builder.ttl(ttl);
    }
    let schema_cache_dir = builder.cache_dir_or_default();
    let retriever = builder.build();

    // Load config
    let config_search_dir = file_path.parent().map(Path::to_path_buf);
    let (cfg, config_dir, _config_path) = validate::load_config(config_search_dir.as_deref());

    let compiled_catalogs =
        trace_catalog(&retriever, &cfg, args.no_catalog, &schema_cache_dir).await;

    // Parse file and resolve schema
    let detected_format = parsers::detect_format(file_path);
    let (parser, instance) = parse_file(detected_format, &content, &path_str);

    let Some((schema_uri, is_remote)) = trace_schema_resolution(
        parser.as_ref(),
        &content,
        &instance,
        &cfg,
        &config_dir,
        &compiled_catalogs,
        &path_str,
        file_name,
        file_path,
    ) else {
        return Ok(());
    };

    trace_schema_cache(&retriever, &schema_uri, is_remote, &schema_cache_dir).await;
    trace_validation_cache(
        &retriever,
        &schema_uri,
        is_remote,
        &cfg,
        &path_str,
        &content,
    )
    .await;

    Ok(())
}

async fn trace_catalog(
    retriever: &SchemaCache,
    cfg: &config::Config,
    no_catalog: bool,
    schema_cache_dir: &Path,
) -> Vec<schemastore::CompiledCatalog> {
    println!();
    println!("catalog:");
    let compiled_catalogs = validate::fetch_compiled_catalogs(retriever, cfg, no_catalog).await;
    if no_catalog {
        println!("  status: disabled (--no-catalog)");
    } else {
        let catalog_url = lintel_validate::catalog::CATALOG_URL;
        let catalog_hash = SchemaCache::hash_uri(catalog_url);
        let catalog_cache_path = schema_cache_dir.join(format!("{catalog_hash}.json"));
        println!("  url: {catalog_url}");
        println!("  hash: {catalog_hash}");
        if catalog_cache_path.exists() {
            print_cache_file_info(&catalog_cache_path, "  ");
        } else {
            println!("  cache: miss (not on disk)");
        }
    }
    compiled_catalogs
}

#[allow(clippy::too_many_arguments)]
fn trace_schema_resolution(
    parser: &dyn parsers::Parser,
    content: &str,
    instance: &serde_json::Value,
    cfg: &config::Config,
    config_dir: &Path,
    compiled_catalogs: &[schemastore::CompiledCatalog],
    path_str: &str,
    file_name: &str,
    file_path: &Path,
) -> Option<(String, bool)> {
    println!();
    println!("schema resolution:");
    let schema_uri = if let Some(uri) = parser.extract_schema_uri(content, instance) {
        println!("  source: inline ($schema / modeline)");
        uri
    } else if let Some((pattern, url)) = cfg
        .schemas
        .iter()
        .find(|(pattern, _)| {
            let p = path_str.strip_prefix("./").unwrap_or(path_str);
            glob_match::glob_match(pattern, p) || glob_match::glob_match(pattern, file_name)
        })
        .map(|(pattern, url)| (pattern.as_str(), url.as_str()))
    {
        println!("  source: config mapping");
        println!("  pattern: {pattern}");
        url.to_string()
    } else if let Some(schema_match) = compiled_catalogs
        .iter()
        .find_map(|cat| cat.find_schema_detailed(path_str, file_name))
    {
        println!("  source: catalog");
        println!("  matched: {}", schema_match.matched_pattern);
        println!("  name: {}", schema_match.name);
        schema_match.url.to_string()
    } else {
        println!("  result: no schema found");
        return None;
    };

    // Apply rewrites
    let schema_uri = config::apply_rewrites(&schema_uri, &cfg.rewrite);
    let schema_uri = config::resolve_double_slash(&schema_uri, config_dir);
    let is_remote = schema_uri.starts_with("http://") || schema_uri.starts_with("https://");
    let schema_uri = if is_remote {
        schema_uri
    } else {
        file_path
            .parent()
            .map(|parent| parent.join(&schema_uri).to_string_lossy().to_string())
            .unwrap_or(schema_uri)
    };
    println!("  uri: {schema_uri}");

    Some((schema_uri, is_remote))
}

async fn trace_schema_cache(
    retriever: &SchemaCache,
    schema_uri: &str,
    is_remote: bool,
    schema_cache_dir: &Path,
) {
    println!();
    println!("schema cache:");
    if is_remote {
        let schema_hash = SchemaCache::hash_uri(schema_uri);
        let schema_cache_path = schema_cache_dir.join(format!("{schema_hash}.json"));
        println!("  hash: {schema_hash}");
        println!("  path: {}", schema_cache_path.display());
        if schema_cache_path.exists() {
            print_cache_file_info(&schema_cache_path, "  ");
        } else {
            println!("  cache: miss (not on disk)");
        }

        match retriever.fetch(schema_uri).await {
            Ok((_value, status)) => {
                let label = match status {
                    CacheStatus::Hit => "hit",
                    CacheStatus::Miss => "miss (fetched from network)",
                    CacheStatus::Disabled => "disabled",
                };
                println!("  fetch status: {label}");
            }
            Err(e) => {
                println!("  fetch error: {e}");
            }
        }
    } else {
        println!("  local schema: {schema_uri}");
        if let Ok(meta) = fs::metadata(schema_uri) {
            println!("  size: {} bytes", meta.len());
        } else {
            println!("  warning: file not found");
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn trace_validation_cache(
    retriever: &SchemaCache,
    schema_uri: &str,
    is_remote: bool,
    cfg: &config::Config,
    path_str: &str,
    content: &str,
) {
    println!();
    println!("validation cache:");
    let vcache_dir = validation_cache::ensure_cache_dir();
    println!("  dir: {}", vcache_dir.display());

    let schema_value = if is_remote {
        match retriever.fetch(schema_uri).await {
            Ok((val, _)) => Some(val),
            Err(_) => None,
        }
    } else if let Ok(schema_content) = fs::read_to_string(schema_uri) {
        serde_json::from_str(&schema_content).ok()
    } else {
        None
    };

    if let Some(schema_value) = schema_value {
        let schema_hash = validation_cache::schema_hash(&schema_value);
        let vcache = validation_cache::ValidationCache::new(vcache_dir, false);
        let validate_formats = cfg.should_validate_formats(path_str, &[schema_uri]);
        let ck = validation_cache::CacheKey {
            file_content: content,
            schema_hash: &schema_hash,
            validate_formats,
        };
        let cache_key = validation_cache::ValidationCache::cache_key(&ck);
        println!("  key: {cache_key}");
        let (_cached_errors, vcache_status) = vcache.lookup(&ck).await;
        let label = match vcache_status {
            validation_cache::ValidationCacheStatus::Hit => "hit",
            validation_cache::ValidationCacheStatus::Miss => "miss",
        };
        println!("  status: {label}");
    } else {
        println!("  status: unavailable (could not load schema for hash)");
    }
}

fn format_json_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let keys: Vec<&str> = map.keys().take(5).map(core::ops::Deref::deref).collect();
            if map.len() > 5 {
                format!("{{{},...}} ({} keys)", keys.join(", "), map.len())
            } else {
                format!("{{{}}} ({} keys)", keys.join(", "), map.len())
            }
        }
        serde_json::Value::Array(arr) => {
            format!("[...] ({} items)", arr.len())
        }
        other => {
            let s = other.to_string();
            if s.len() > 80 {
                format!("{}...", &s[..80])
            } else {
                s
            }
        }
    }
}

fn print_modified_age(meta: &fs::Metadata) {
    if let Ok(modified) = meta.modified()
        && let Ok(age) = modified.elapsed()
    {
        let duration = humantime::format_duration(core::time::Duration::from_secs(age.as_secs()));
        println!("Modified:   {duration} ago");
    }
}

fn print_cache_file_info(path: &Path, indent: &str) {
    if let Ok(meta) = fs::metadata(path) {
        println!("{indent}size: {} bytes", meta.len());
        if let Ok(modified) = meta.modified()
            && let Ok(age) = modified.elapsed()
        {
            let duration =
                humantime::format_duration(core::time::Duration::from_secs(age.as_secs()));
            println!("{indent}modified: {duration} ago");
        }
    }
}

/// Parse the file content, trying the detected format first, then all parsers as fallback.
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
        if let Some((fmt, val)) = validate::try_parse_all(content, path_str) {
            return (parsers::parser_for(fmt), val);
        }
        eprintln!("error: could not parse {path_str}");
        std::process::exit(2);
    }

    if let Some((fmt, val)) = validate::try_parse_all(content, path_str) {
        return (parsers::parser_for(fmt), val);
    }

    eprintln!("error: unrecognized format for {path_str}");
    std::process::exit(2);
}
