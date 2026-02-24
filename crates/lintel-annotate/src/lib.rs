#![doc = include_str!("../README.md")]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use bpaf::Bpaf;

use lintel_cli_common::CliCacheOptions;
use lintel_schema_cache::SchemaCache;
use lintel_validate::parsers;
use lintel_validate::validate;
use schema_catalog::CompiledCatalog;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(annotate_args_inner))]
pub struct AnnotateArgs {
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(external(lintel_cli_common::cli_cache_options))]
    pub cache: CliCacheOptions,

    /// Update existing annotations with latest catalog resolutions
    #[bpaf(long("update"), switch)]
    pub update: bool,

    #[bpaf(positional("PATH"))]
    pub globs: Vec<String>,
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
    config: &lintel_config::Config,
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
pub async fn run(args: &AnnotateArgs) -> Result<AnnotateResult> {
    let config_dir = args
        .globs
        .iter()
        .find(|g| Path::new(g).is_dir())
        .map(PathBuf::from);

    let mut builder = SchemaCache::builder();
    if let Some(dir) = &args.cache.cache_dir {
        builder = builder.cache_dir(PathBuf::from(dir));
    }
    if let Some(ttl) = args.cache.schema_cache_ttl {
        builder = builder.ttl(ttl);
    }
    let retriever = builder.build();

    let (mut config, _, _) = validate::load_config(config_dir.as_deref());
    config.exclude.extend(args.exclude.clone());

    let files = validate::collect_files(&args.globs, &config.exclude)?;
    tracing::info!(file_count = files.len(), "collected files");

    let catalogs =
        validate::fetch_compiled_catalogs(&retriever, &config, args.cache.no_catalog).await;

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
    use lintel_validate::parsers::{
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
