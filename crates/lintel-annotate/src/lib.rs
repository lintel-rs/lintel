#![doc = include_str!("../README.md")]

use core::time::Duration;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bpaf::{Bpaf, Parser};
use glob::glob;

use lintel_check::catalog::{self, CompiledCatalog};
use lintel_check::config;
use lintel_check::discover;
use lintel_check::parsers;
use lintel_check::registry;
use lintel_check::retriever::{HttpClient, SchemaCache, ensure_cache_dir};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(annotate_args_inner))]
pub struct AnnotateArgs {
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,

    #[bpaf(external(schema_cache_ttl))]
    pub schema_cache_ttl: Option<Duration>,

    /// Update existing annotations with latest catalog resolutions
    #[bpaf(long("update"), switch)]
    pub update: bool,

    #[bpaf(positional("PATH"))]
    pub globs: Vec<String>,
}

fn schema_cache_ttl() -> impl bpaf::Parser<Option<Duration>> {
    bpaf::long("schema-cache-ttl")
        .help("Schema cache TTL (e.g. \"12h\", \"30m\", \"1d\"); default 12h")
        .argument::<String>("DURATION")
        .parse(|s: String| {
            humantime::parse_duration(&s).map_err(|e| format!("invalid duration '{s}': {e}"))
        })
        .optional()
}

/// Construct the bpaf parser for `AnnotateArgs`.
pub fn annotate_args() -> impl bpaf::Parser<AnnotateArgs> {
    annotate_args_inner()
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

pub struct AnnotatedFile {
    pub path: String,
    pub schema_url: String,
}

pub struct AnnotateResult {
    pub annotated: Vec<AnnotatedFile>,
    pub updated: Vec<AnnotatedFile>,
    pub skipped: usize,
    pub errors: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Config loading (mirrors validate.rs)
// ---------------------------------------------------------------------------

fn load_config(search_dir: Option<&Path>) -> (config::Config, PathBuf) {
    let start_dir = match search_dir {
        Some(d) => d.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return (config::Config::default(), PathBuf::from(".")),
        },
    };

    let cfg = config::find_and_load(&start_dir)
        .ok()
        .flatten()
        .unwrap_or_default();
    (cfg, start_dir)
}

// ---------------------------------------------------------------------------
// File collection (mirrors validate.rs)
// ---------------------------------------------------------------------------

fn collect_files(globs_arg: &[String], exclude: &[String]) -> Result<Vec<PathBuf>> {
    if globs_arg.is_empty() {
        return discover::discover_files(".", exclude);
    }

    let mut result = Vec::new();
    for pattern in globs_arg {
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
// Catalog fetching
// ---------------------------------------------------------------------------

async fn fetch_catalogs<C: HttpClient>(
    retriever: &SchemaCache<C>,
    registries: &[String],
) -> Vec<CompiledCatalog> {
    type CatalogResult = (
        String,
        Result<CompiledCatalog, Box<dyn core::error::Error + Send + Sync>>,
    );
    let mut catalog_tasks: tokio::task::JoinSet<CatalogResult> = tokio::task::JoinSet::new();

    // Lintel catalog
    let r = retriever.clone();
    let label = format!("default catalog {}", registry::DEFAULT_REGISTRY);
    catalog_tasks.spawn(async move {
        let result = registry::fetch(&r, registry::DEFAULT_REGISTRY)
            .await
            .map(|cat| CompiledCatalog::compile(&cat));
        (label, result)
    });

    // SchemaStore catalog
    let r = retriever.clone();
    catalog_tasks.spawn(async move {
        let result = catalog::fetch_catalog(&r)
            .await
            .map(|cat| CompiledCatalog::compile(&cat));
        ("SchemaStore catalog".to_string(), result)
    });

    // Additional registries
    for registry_url in registries {
        let r = retriever.clone();
        let url = registry_url.clone();
        let label = format!("registry {url}");
        catalog_tasks.spawn(async move {
            let result = registry::fetch(&r, &url)
                .await
                .map(|cat| CompiledCatalog::compile(&cat));
            (label, result)
        });
    }

    let mut compiled = Vec::new();
    while let Some(result) = catalog_tasks.join_next().await {
        match result {
            Ok((_, Ok(catalog))) => compiled.push(catalog),
            Ok((label, Err(e))) => eprintln!("warning: failed to fetch {label}: {e}"),
            Err(e) => eprintln!("warning: catalog fetch task failed: {e}"),
        }
    }
    compiled
}

// ---------------------------------------------------------------------------
// Per-file processing
// ---------------------------------------------------------------------------

enum FileOutcome {
    Annotated(AnnotatedFile),
    Updated(AnnotatedFile),
    Skipped,
    Error(String, String),
}

fn process_file(
    file_path: &Path,
    config: &config::Config,
    catalogs: &[CompiledCatalog],
    update: bool,
) -> FileOutcome {
    let path_str = file_path.display().to_string();
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_str);

    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => return FileOutcome::Error(path_str, format!("failed to read: {e}")),
    };

    let Some(fmt) = parsers::detect_format(file_path) else {
        return FileOutcome::Skipped;
    };

    let parser = parsers::parser_for(fmt);
    let Ok(instance) = parser.parse(&content, &path_str) else {
        return FileOutcome::Skipped;
    };

    let existing_schema = parser.extract_schema_uri(&content, &instance);
    if existing_schema.is_some() && !update {
        return FileOutcome::Skipped;
    }

    let schema_url = config
        .find_schema_mapping(&path_str, file_name)
        .map(str::to_string)
        .or_else(|| {
            catalogs
                .iter()
                .find_map(|cat| cat.find_schema(&path_str, file_name))
                .map(str::to_string)
        });

    let Some(schema_url) = schema_url else {
        return FileOutcome::Skipped;
    };

    let is_update = existing_schema.is_some();
    if existing_schema.is_some_and(|existing| existing == schema_url) {
        return FileOutcome::Skipped;
    }

    let content = if is_update {
        parser.strip_annotation(&content)
    } else {
        content
    };

    let Some(new_content) = parser.annotate(&content, &schema_url) else {
        return FileOutcome::Skipped;
    };

    match fs::write(file_path, &new_content) {
        Ok(()) => {
            let file = AnnotatedFile {
                path: path_str,
                schema_url,
            };
            if is_update {
                FileOutcome::Updated(file)
            } else {
                FileOutcome::Annotated(file)
            }
        }
        Err(e) => FileOutcome::Error(path_str, format!("failed to write: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Core logic
// ---------------------------------------------------------------------------

/// Run the annotate command.
///
/// # Errors
///
/// Returns an error if file collection or catalog fetching fails fatally.
///
/// # Panics
///
/// Panics if `--schema-cache-ttl` is provided with an unparseable duration.
#[tracing::instrument(skip_all, name = "annotate")]
pub async fn run<C: HttpClient>(args: &AnnotateArgs, client: C) -> Result<AnnotateResult> {
    let config_dir = args
        .globs
        .iter()
        .find(|g| Path::new(g).is_dir())
        .map(PathBuf::from);

    let schema_cache_ttl = args.schema_cache_ttl;

    let cache_dir_path = args
        .cache_dir
        .as_ref()
        .map_or_else(ensure_cache_dir, PathBuf::from);
    let retriever = SchemaCache::new(
        Some(cache_dir_path),
        client,
        false, // don't force schema fetch
        schema_cache_ttl,
    );

    let (mut config, _config_dir) = load_config(config_dir.as_deref());
    config.exclude.extend(args.exclude.clone());

    let files = collect_files(&args.globs, &config.exclude)?;
    tracing::info!(file_count = files.len(), "collected files");

    let catalogs = if args.no_catalog {
        Vec::new()
    } else {
        fetch_catalogs(&retriever, &config.registries).await
    };

    let mut result = AnnotateResult {
        annotated: Vec::new(),
        updated: Vec::new(),
        skipped: 0,
        errors: Vec::new(),
    };

    for file_path in &files {
        match process_file(file_path, &config, &catalogs, args.update) {
            FileOutcome::Annotated(f) => result.annotated.push(f),
            FileOutcome::Updated(f) => result.updated.push(f),
            FileOutcome::Skipped => result.skipped += 1,
            FileOutcome::Error(path, msg) => result.errors.push((path, msg)),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use lintel_check::parsers::{
        Json5Parser, JsonParser, JsoncParser, Parser, TomlParser, YamlParser,
    };

    // --- JSON annotation ---

    #[test]
    fn json_compact() {
        let result = JsonParser
            .annotate(r#"{"name":"hello"}"#, "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(
            result,
            r#"{"$schema":"https://example.com/schema.json","name":"hello"}"#
        );
    }

    #[test]
    fn json_pretty() {
        let result = JsonParser
            .annotate(
                "{\n  \"name\": \"hello\"\n}\n",
                "https://example.com/schema.json",
            )
            .expect("annotate failed");
        assert_eq!(
            result,
            "{\n  \"$schema\": \"https://example.com/schema.json\",\n  \"name\": \"hello\"\n}\n"
        );
    }

    #[test]
    fn json_pretty_4_spaces() {
        let result = JsonParser
            .annotate(
                "{\n    \"name\": \"hello\"\n}\n",
                "https://example.com/schema.json",
            )
            .expect("annotate failed");
        assert_eq!(
            result,
            "{\n    \"$schema\": \"https://example.com/schema.json\",\n    \"name\": \"hello\"\n}\n"
        );
    }

    #[test]
    fn json_pretty_tabs() {
        let result = JsonParser
            .annotate(
                "{\n\t\"name\": \"hello\"\n}\n",
                "https://example.com/schema.json",
            )
            .expect("annotate failed");
        assert_eq!(
            result,
            "{\n\t\"$schema\": \"https://example.com/schema.json\",\n\t\"name\": \"hello\"\n}\n"
        );
    }

    #[test]
    fn json_empty_object() {
        let result = JsonParser
            .annotate("{}", "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(result, r#"{"$schema":"https://example.com/schema.json",}"#);
    }

    #[test]
    fn json_empty_object_pretty() {
        let result = JsonParser
            .annotate("{\n}\n", "https://example.com/schema.json")
            .expect("annotate failed");
        assert!(result.contains("\"$schema\": \"https://example.com/schema.json\""));
    }

    // --- JSON5 annotation delegates to same logic ---

    #[test]
    fn json5_compact() {
        let result = Json5Parser
            .annotate(r#"{"name":"hello"}"#, "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(
            result,
            r#"{"$schema":"https://example.com/schema.json","name":"hello"}"#
        );
    }

    // --- JSONC annotation delegates to same logic ---

    #[test]
    fn jsonc_compact() {
        let result = JsoncParser
            .annotate(r#"{"name":"hello"}"#, "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(
            result,
            r#"{"$schema":"https://example.com/schema.json","name":"hello"}"#
        );
    }

    // --- YAML annotation ---

    #[test]
    fn yaml_prepends_modeline() {
        let result = YamlParser
            .annotate("name: hello\n", "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(
            result,
            "# yaml-language-server: $schema=https://example.com/schema.json\nname: hello\n"
        );
    }

    #[test]
    fn yaml_preserves_existing_comments() {
        let result = YamlParser
            .annotate(
                "# existing comment\nname: hello\n",
                "https://example.com/schema.json",
            )
            .expect("annotate failed");
        assert_eq!(
            result,
            "# yaml-language-server: $schema=https://example.com/schema.json\n# existing comment\nname: hello\n"
        );
    }

    // --- TOML annotation ---

    #[test]
    fn toml_prepends_schema_comment() {
        let result = TomlParser
            .annotate("name = \"hello\"\n", "https://example.com/schema.json")
            .expect("annotate failed");
        assert_eq!(
            result,
            "# :schema https://example.com/schema.json\nname = \"hello\"\n"
        );
    }

    #[test]
    fn toml_preserves_existing_comments() {
        let result = TomlParser
            .annotate(
                "# existing comment\nname = \"hello\"\n",
                "https://example.com/schema.json",
            )
            .expect("annotate failed");
        assert_eq!(
            result,
            "# :schema https://example.com/schema.json\n# existing comment\nname = \"hello\"\n"
        );
    }

    // --- JSON strip_annotation ---

    #[test]
    fn json_strip_compact_first_property() {
        let input = r#"{"$schema":"https://old.com/s.json","name":"hello"}"#;
        assert_eq!(JsonParser.strip_annotation(input), r#"{"name":"hello"}"#);
    }

    #[test]
    fn json_strip_pretty_first_property() {
        let input = "{\n  \"$schema\": \"https://old.com/s.json\",\n  \"name\": \"hello\"\n}\n";
        assert_eq!(
            JsonParser.strip_annotation(input),
            "{\n  \"name\": \"hello\"\n}\n"
        );
    }

    #[test]
    fn json_strip_only_property() {
        let input = r#"{"$schema":"https://old.com/s.json"}"#;
        assert_eq!(JsonParser.strip_annotation(input), "{}");
    }

    #[test]
    fn json_strip_last_property() {
        let input = r#"{"name":"hello","$schema":"https://old.com/s.json"}"#;
        assert_eq!(JsonParser.strip_annotation(input), r#"{"name":"hello"}"#);
    }

    #[test]
    fn json_strip_no_schema() {
        let input = r#"{"name":"hello"}"#;
        assert_eq!(JsonParser.strip_annotation(input), input);
    }

    // --- YAML strip_annotation ---

    #[test]
    fn yaml_strip_modeline() {
        let input = "# yaml-language-server: $schema=https://old.com/s.json\nname: hello\n";
        assert_eq!(YamlParser.strip_annotation(input), "name: hello\n");
    }

    #[test]
    fn yaml_strip_modeline_preserves_other_comments() {
        let input =
            "# yaml-language-server: $schema=https://old.com/s.json\n# other\nname: hello\n";
        assert_eq!(YamlParser.strip_annotation(input), "# other\nname: hello\n");
    }

    #[test]
    fn yaml_strip_no_modeline() {
        let input = "name: hello\n";
        assert_eq!(YamlParser.strip_annotation(input), input);
    }

    // --- TOML strip_annotation ---

    #[test]
    fn toml_strip_schema_comment() {
        let input = "# :schema https://old.com/s.json\nname = \"hello\"\n";
        assert_eq!(TomlParser.strip_annotation(input), "name = \"hello\"\n");
    }

    #[test]
    fn toml_strip_legacy_schema_comment() {
        let input = "# $schema: https://old.com/s.json\nname = \"hello\"\n";
        assert_eq!(TomlParser.strip_annotation(input), "name = \"hello\"\n");
    }

    #[test]
    fn toml_strip_preserves_other_comments() {
        let input = "# :schema https://old.com/s.json\n# other\nname = \"hello\"\n";
        assert_eq!(
            TomlParser.strip_annotation(input),
            "# other\nname = \"hello\"\n"
        );
    }

    #[test]
    fn toml_strip_no_schema() {
        let input = "name = \"hello\"\n";
        assert_eq!(TomlParser.strip_annotation(input), input);
    }

    // --- Round-trip: strip then re-annotate ---

    #[test]
    fn json_update_round_trip() {
        let original = "{\n  \"$schema\": \"https://old.com/s.json\",\n  \"name\": \"hello\"\n}\n";
        let stripped = JsonParser.strip_annotation(original);
        let updated = JsonParser
            .annotate(&stripped, "https://new.com/s.json")
            .expect("annotate failed");
        assert_eq!(
            updated,
            "{\n  \"$schema\": \"https://new.com/s.json\",\n  \"name\": \"hello\"\n}\n"
        );
    }

    #[test]
    fn yaml_update_round_trip() {
        let original = "# yaml-language-server: $schema=https://old.com/s.json\nname: hello\n";
        let stripped = YamlParser.strip_annotation(original);
        let updated = YamlParser
            .annotate(&stripped, "https://new.com/s.json")
            .expect("annotate failed");
        assert_eq!(
            updated,
            "# yaml-language-server: $schema=https://new.com/s.json\nname: hello\n"
        );
    }

    #[test]
    fn toml_update_round_trip() {
        let original = "# :schema https://old.com/s.json\nname = \"hello\"\n";
        let stripped = TomlParser.strip_annotation(original);
        let updated = TomlParser
            .annotate(&stripped, "https://new.com/s.json")
            .expect("annotate failed");
        assert_eq!(
            updated,
            "# :schema https://new.com/s.json\nname = \"hello\"\n"
        );
    }
}
