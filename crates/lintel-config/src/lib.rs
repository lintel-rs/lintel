#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;

const CONFIG_FILENAME: &str = "lintel.toml";

fn example_file_pattern() -> Vec<String> {
    vec!["schemas/vector.json".into()]
}

fn example_file_glob() -> Vec<String> {
    vec!["schemas/**/*.json".into()]
}

fn example_file_config() -> Vec<String> {
    vec!["config/*.yaml".into()]
}

fn example_schema_url() -> Vec<String> {
    vec!["https://json.schemastore.org/vector.json".into()]
}

fn example_schema_glob() -> Vec<String> {
    vec!["https://json.schemastore.org/*.json".into()]
}

fn example_exclude() -> Vec<String> {
    vec![
        "vendor/**".into(),
        "testdata/**".into(),
        "*.generated.json".into(),
    ]
}

fn example_registry() -> Vec<String> {
    vec!["https://example.com/custom-catalog.json".into()]
}

/// Conditional settings applied to files or schemas matching specific patterns.
///
/// Each `[[override]]` block targets files by path glob, schemas by URI glob,
/// or both. When a file matches, the settings in that block override the
/// top-level defaults. Earlier entries (from child configs) take priority over
/// later entries (from parent configs).
///
/// In TOML, override blocks are written as `[[override]]` (double brackets) to
/// create an array of tables.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Override Rule")]
pub struct Override {
    /// Glob patterns matched against instance file paths (relative to the
    /// working directory).
    ///
    /// Use standard glob syntax: `*` matches any single path component, `**`
    /// matches zero or more path components, and `?` matches a single
    /// character.
    #[schemars(
        title = "File Patterns",
        example = example_file_pattern(),
        example = example_file_glob(),
        example = example_file_config(),
    )]
    #[serde(default)]
    pub files: Vec<String>,

    /// Glob patterns matched against schema URIs.
    ///
    /// Each pattern is tested against both the original URI (before rewrite
    /// rules) and the resolved URI (after rewrites and `//` prefix
    /// resolution), so you can match on either form.
    #[schemars(
        title = "Schema Patterns",
        example = example_schema_url(),
        example = example_schema_glob(),
    )]
    #[serde(default)]
    pub schemas: Vec<String>,

    /// Enable or disable JSON Schema `format` keyword validation for matching
    /// files.
    ///
    /// When `true`, string values are validated against built-in formats such
    /// as `date-time`, `email`, `uri`, etc. When `false`, format annotations
    /// are ignored during validation. When omitted, this override does not
    /// affect the format validation setting and the next matching override (or
    /// the default of `true`) applies.
    #[schemars(title = "Validate Formats")]
    #[serde(default)]
    pub validate_formats: Option<bool>,
}

/// Configuration file for the Lintel JSON/YAML schema validator.
///
/// Lintel walks up the directory tree from the validated file looking for
/// `lintel.toml` files and merges them together. Settings in child directories
/// take priority over parent directories. Set `root = true` to stop the upward
/// search.
///
/// Place `lintel.toml` at your project root (or any subdirectory that needs
/// different settings).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "lintel.toml")]
pub struct Config {
    /// Mark this configuration file as the project root.
    ///
    /// When `true`, Lintel stops walking up the directory tree and will not
    /// merge any `lintel.toml` files from parent directories. Use this at your
    /// repository root to prevent inheriting settings from enclosing
    /// directories.
    #[serde(default)]
    pub root: bool,

    /// Glob patterns for files to exclude from validation.
    ///
    /// Matched against file paths relative to the working directory. Standard
    /// glob syntax is supported: `*` matches within a single directory, `**`
    /// matches across directory boundaries.
    #[schemars(title = "Exclude Patterns", example = example_exclude())]
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Custom schema-to-file mappings.
    ///
    /// Keys are glob patterns matched against file paths; values are schema
    /// URLs (or `//`-prefixed local paths) to apply. These mappings take
    /// priority over catalog auto-detection but are overridden by inline
    /// `$schema` properties and YAML modeline comments.
    ///
    /// Example:
    /// ```toml
    /// [schemas]
    /// "config/*.yaml" = "https://json.schemastore.org/github-workflow.json"
    /// "myschema.json" = "//schemas/custom.json"
    /// ```
    #[schemars(title = "Schema Mappings")]
    #[serde(default)]
    pub schemas: HashMap<String, String>,

    /// Disable the built-in Lintel catalog.
    ///
    /// When `true`, only `SchemaStore` and any additional registries listed in
    /// `registries` are used for schema auto-detection. The default Lintel
    /// catalog (which provides curated schema mappings) is skipped.
    #[schemars(title = "No Default Catalog")]
    #[serde(default, rename = "no-default-catalog")]
    pub no_default_catalog: bool,

    /// Additional schema catalog URLs to fetch alongside `SchemaStore`.
    ///
    /// Each entry should be a URL pointing to a JSON file in `SchemaStore`
    /// catalog format (`{"schemas": [...]}`).
    ///
    /// Registries from child configs appear first, followed by parent
    /// registries (duplicates are removed). This lets child directories add
    /// project-specific catalogs while inheriting organization-wide ones.
    #[schemars(title = "Additional Registries", example = example_registry())]
    #[serde(default)]
    pub registries: Vec<String>,

    /// Schema URI rewrite rules.
    ///
    /// Keys are URI prefixes to match; values are replacement prefixes. The
    /// longest matching prefix wins. Use `//` as a value prefix to reference
    /// paths relative to the directory containing `lintel.toml`.
    ///
    /// Example:
    /// ```toml
    /// [rewrite]
    /// "http://localhost:8000/" = "//schemas/"
    /// ```
    /// This rewrites `http://localhost:8000/foo.json` to
    /// `//schemas/foo.json`, which then resolves to a local file relative to
    /// the config directory.
    #[schemars(title = "Rewrite Rules")]
    #[serde(default)]
    pub rewrite: HashMap<String, String>,

    /// Per-file or per-schema override rules.
    ///
    /// In TOML, each override is written as a `[[override]]` block (double
    /// brackets). Earlier entries take priority; child config overrides come
    /// before parent config overrides after merging.
    #[serde(default, rename = "override")]
    pub overrides: Vec<Override>,
}

impl Config {
    /// Merge a parent config into this one.  Child values take priority:
    /// - `exclude`: parent entries are appended (child entries come first)
    /// - `schemas`: parent entries are added only if the key is not already present
    /// - `registries`: parent entries are appended (deduped)
    /// - `rewrite`: parent entries are added only if the key is not already present
    /// - `root` is not inherited
    fn merge_parent(&mut self, parent: Config) {
        self.exclude.extend(parent.exclude);
        for (k, v) in parent.schemas {
            self.schemas.entry(k).or_insert(v);
        }
        for url in parent.registries {
            if !self.registries.contains(&url) {
                self.registries.push(url);
            }
        }
        for (k, v) in parent.rewrite {
            self.rewrite.entry(k).or_insert(v);
        }
        // Child overrides come first (higher priority), then parent overrides.
        self.overrides.extend(parent.overrides);
    }

    /// Find a custom schema mapping for the given file path.
    ///
    /// Matches against the `[schemas]` table using glob patterns.
    /// Returns the schema URL if a match is found.
    pub fn find_schema_mapping(&self, path: &str, file_name: &str) -> Option<&str> {
        let path = path.strip_prefix("./").unwrap_or(path);
        for (pattern, url) in &self.schemas {
            if glob_match::glob_match(pattern, path) || glob_match::glob_match(pattern, file_name) {
                return Some(url);
            }
        }
        None
    }

    /// Check whether format validation should be enabled for a given file.
    ///
    /// `path` is the instance file path.  `schema_uris` is a slice of schema
    /// URIs to match against (typically the original URI before rewrites and
    /// the resolved URI after rewrites + `//` resolution).
    ///
    /// Returns `false` if any matching `[[override]]` sets
    /// `validate_formats = false`.  Defaults to `true` when no override matches.
    pub fn should_validate_formats(&self, path: &str, schema_uris: &[&str]) -> bool {
        let path = path.strip_prefix("./").unwrap_or(path);
        for ov in &self.overrides {
            let file_match = !ov.files.is_empty()
                && ov.files.iter().any(|pat| glob_match::glob_match(pat, path));
            let schema_match = !ov.schemas.is_empty()
                && schema_uris.iter().any(|uri| {
                    ov.schemas
                        .iter()
                        .any(|pat| glob_match::glob_match(pat, uri))
                });
            if (file_match || schema_match)
                && let Some(val) = ov.validate_formats
            {
                return val;
            }
        }
        true
    }
}

/// Apply rewrite rules to a schema URI. If the URI starts with any key in
/// `rewrites`, that prefix is replaced with the corresponding value.
/// The longest matching prefix wins.
pub fn apply_rewrites<S: ::core::hash::BuildHasher>(
    uri: &str,
    rewrites: &HashMap<String, String, S>,
) -> String {
    let mut best_match: Option<(&str, &str)> = None;
    for (from, to) in rewrites {
        if uri.starts_with(from.as_str())
            && best_match.is_none_or(|(prev_from, _)| from.len() > prev_from.len())
        {
            best_match = Some((from.as_str(), to.as_str()));
        }
    }
    match best_match {
        Some((from, to)) => format!("{to}{}", &uri[from.len()..]),
        None => uri.to_string(),
    }
}

/// Resolve a `//`-prefixed path relative to the given root directory (the
/// directory containing `lintel.toml`). Non-`//` paths are returned unchanged.
pub fn resolve_double_slash(uri: &str, config_dir: &Path) -> String {
    if let Some(rest) = uri.strip_prefix("//") {
        config_dir.join(rest).to_string_lossy().to_string()
    } else {
        uri.to_string()
    }
}

/// Generate the JSON Schema for `lintel.toml` as a `serde_json::Value`.
///
/// # Panics
///
/// Panics if the schema cannot be serialized to JSON (should never happen).
pub fn schema() -> Value {
    serde_json::to_value(schema_for!(Config)).expect("schema serialization cannot fail")
}

/// Find the nearest `lintel.toml` starting from `start_dir`, walking upward.
/// Returns the path to `lintel.toml`, or `None` if not found.
pub fn find_config_path(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Search for `lintel.toml` files starting from `start_dir`, walking up.
/// Merges all configs found until one with `root = true` is hit (inclusive).
/// Returns the merged config, or `None` if no config file was found.
///
/// # Errors
///
/// Returns an error if a config file exists but cannot be read or parsed.
pub fn find_and_load(start_dir: &Path) -> Result<Option<Config>, anyhow::Error> {
    let mut configs: Vec<Config> = Vec::new();
    let mut dir = start_dir.to_path_buf();

    loop {
        let candidate = dir.join(CONFIG_FILENAME);
        if candidate.is_file() {
            let content = std::fs::read_to_string(&candidate)?;
            let cfg: Config = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", candidate.display()))?;
            let is_root = cfg.root;
            configs.push(cfg);
            if is_root {
                break;
            }
        }
        if !dir.pop() {
            break;
        }
    }

    if configs.is_empty() {
        return Ok(None);
    }

    // configs[0] is the closest (child), last is the farthest (root-most parent)
    let mut merged = configs.remove(0);
    for parent in configs {
        merged.merge_parent(parent);
    }
    Ok(Some(merged))
}

/// Load config from the current working directory (walking upward).
///
/// # Errors
///
/// Returns an error if a config file exists but cannot be read or parsed.
pub fn load() -> Result<Config, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    Ok(find_and_load(&cwd)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_config_from_directory() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"exclude = ["testdata/**"]"#,
        )?;

        let config = find_and_load(tmp.path())?.expect("config should exist");
        assert_eq!(config.exclude, vec!["testdata/**"]);
        Ok(())
    }

    #[test]
    fn walks_up_to_find_config() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("a/b/c");
        fs::create_dir_all(&sub)?;
        fs::write(tmp.path().join("lintel.toml"), r#"exclude = ["vendor/**"]"#)?;

        let config = find_and_load(&sub)?.expect("config should exist");
        assert_eq!(config.exclude, vec!["vendor/**"]);
        Ok(())
    }

    #[test]
    fn returns_none_when_no_config() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let config = find_and_load(tmp.path())?;
        assert!(config.is_none());
        Ok(())
    }

    #[test]
    fn empty_config_is_valid() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("lintel.toml"), "")?;

        let config = find_and_load(tmp.path())?.expect("config should exist");
        assert!(config.exclude.is_empty());
        assert!(config.rewrite.is_empty());
        Ok(())
    }

    #[test]
    fn rejects_unknown_fields() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("lintel.toml"), "bogus = true")?;

        let result = find_and_load(tmp.path());
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn loads_rewrite_rules() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://localhost:8000/" = "//schemastore/src/"
"#,
        )?;

        let config = find_and_load(tmp.path())?.expect("config should exist");
        assert_eq!(
            config.rewrite.get("http://localhost:8000/"),
            Some(&"//schemastore/src/".to_string())
        );
        Ok(())
    }

    // --- root = true ---

    #[test]
    fn root_true_stops_walk() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub)?;

        // Parent config
        fs::write(tmp.path().join("lintel.toml"), r#"exclude = ["parent/**"]"#)?;

        // Child config with root = true
        fs::write(
            sub.join("lintel.toml"),
            "root = true\nexclude = [\"child/**\"]",
        )?;

        let config = find_and_load(&sub)?.expect("config should exist");
        assert_eq!(config.exclude, vec!["child/**"]);
        // Parent's "parent/**" should NOT be included
        Ok(())
    }

    #[test]
    fn merges_parent_without_root() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub)?;

        // Parent config
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
exclude = ["parent/**"]

[rewrite]
"http://parent/" = "//parent/"
"#,
        )?;

        // Child config (no root = true)
        fs::write(
            sub.join("lintel.toml"),
            r#"
exclude = ["child/**"]

[rewrite]
"http://child/" = "//child/"
"#,
        )?;

        let config = find_and_load(&sub)?.expect("config should exist");
        // Child excludes come first, then parent
        assert_eq!(config.exclude, vec!["child/**", "parent/**"]);
        // Both rewrite rules present
        assert_eq!(
            config.rewrite.get("http://child/"),
            Some(&"//child/".to_string())
        );
        assert_eq!(
            config.rewrite.get("http://parent/"),
            Some(&"//parent/".to_string())
        );
        Ok(())
    }

    #[test]
    fn child_rewrite_wins_on_conflict() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub)?;

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://example/" = "//parent-value/"
"#,
        )?;

        fs::write(
            sub.join("lintel.toml"),
            r#"
[rewrite]
"http://example/" = "//child-value/"
"#,
        )?;

        let config = find_and_load(&sub)?.expect("config should exist");
        assert_eq!(
            config.rewrite.get("http://example/"),
            Some(&"//child-value/".to_string())
        );
        Ok(())
    }

    // --- apply_rewrites ---

    #[test]
    fn rewrite_matching_prefix() {
        let mut rewrites = HashMap::new();
        rewrites.insert(
            "http://localhost:8000/".to_string(),
            "//schemastore/src/".to_string(),
        );
        let result = apply_rewrites("http://localhost:8000/schemas/foo.json", &rewrites);
        assert_eq!(result, "//schemastore/src/schemas/foo.json");
    }

    #[test]
    fn rewrite_no_match() {
        let mut rewrites = HashMap::new();
        rewrites.insert(
            "http://localhost:8000/".to_string(),
            "//schemastore/src/".to_string(),
        );
        let result = apply_rewrites("https://example.com/schema.json", &rewrites);
        assert_eq!(result, "https://example.com/schema.json");
    }

    #[test]
    fn rewrite_longest_prefix_wins() {
        let mut rewrites = HashMap::new();
        rewrites.insert("http://localhost/".to_string(), "//short/".to_string());
        rewrites.insert(
            "http://localhost/api/v2/".to_string(),
            "//long/".to_string(),
        );
        let result = apply_rewrites("http://localhost/api/v2/schema.json", &rewrites);
        assert_eq!(result, "//long/schema.json");
    }

    // --- resolve_double_slash ---

    #[test]
    fn resolve_double_slash_prefix() {
        let config_dir = Path::new("/home/user/project");
        let result = resolve_double_slash("//schemas/foo.json", config_dir);
        assert_eq!(result, "/home/user/project/schemas/foo.json");
    }

    #[test]
    fn resolve_double_slash_no_prefix() {
        let config_dir = Path::new("/home/user/project");
        let result = resolve_double_slash("https://example.com/s.json", config_dir);
        assert_eq!(result, "https://example.com/s.json");
    }

    #[test]
    fn resolve_double_slash_relative_path_unchanged() {
        let config_dir = Path::new("/home/user/project");
        let result = resolve_double_slash("./schemas/foo.json", config_dir);
        assert_eq!(result, "./schemas/foo.json");
    }

    // --- Override parsing ---

    #[test]
    fn parses_override_blocks() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/vector.json"]
validate_formats = false

[[override]]
files = ["schemas/other.json"]
validate_formats = true
"#,
        )?;

        let config = find_and_load(tmp.path())?.expect("config should exist");
        assert_eq!(config.overrides.len(), 2);
        assert_eq!(config.overrides[0].files, vec!["schemas/vector.json"]);
        assert_eq!(config.overrides[0].validate_formats, Some(false));
        assert_eq!(config.overrides[1].validate_formats, Some(true));
        Ok(())
    }

    #[test]
    fn override_validate_formats_defaults_to_none() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/vector.json"]
"#,
        )?;

        let config = find_and_load(tmp.path())?.expect("config should exist");
        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].validate_formats, None);
        Ok(())
    }

    // --- should_validate_formats ---

    #[test]
    fn should_validate_formats_default_true() {
        let config = Config::default();
        assert!(config.should_validate_formats("anything.json", &[]));
    }

    #[test]
    fn should_validate_formats_matching_file_override() {
        let config = Config {
            overrides: vec![Override {
                files: vec!["schemas/vector.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!config.should_validate_formats("schemas/vector.json", &[]));
        assert!(config.should_validate_formats("schemas/other.json", &[]));
    }

    #[test]
    fn should_validate_formats_matching_schema_override() {
        let config = Config {
            overrides: vec![Override {
                schemas: vec!["https://json.schemastore.org/vector.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        // Matches via schema URI
        assert!(!config.should_validate_formats(
            "some/file.toml",
            &["https://json.schemastore.org/vector.json"]
        ));
        // No match
        assert!(config.should_validate_formats(
            "some/file.toml",
            &["https://json.schemastore.org/other.json"]
        ));
    }

    #[test]
    fn should_validate_formats_schema_glob() {
        let config = Config {
            overrides: vec![Override {
                schemas: vec!["https://json.schemastore.org/*.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(
            !config
                .should_validate_formats("any.toml", &["https://json.schemastore.org/vector.json"])
        );
    }

    #[test]
    fn should_validate_formats_matches_resolved_uri() {
        let config = Config {
            overrides: vec![Override {
                schemas: vec!["/local/schemas/vector.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        // original doesn't match, but resolved does
        assert!(!config.should_validate_formats(
            "any.toml",
            &[
                "https://json.schemastore.org/vector.json",
                "/local/schemas/vector.json"
            ]
        ));
    }

    #[test]
    fn should_validate_formats_glob_pattern() {
        let config = Config {
            overrides: vec![Override {
                files: vec!["schemas/**/*.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!config.should_validate_formats("schemas/deep/nested.json", &[]));
        assert!(config.should_validate_formats("other/file.json", &[]));
    }

    #[test]
    fn should_validate_formats_strips_dot_slash() {
        let config = Config {
            overrides: vec![Override {
                files: vec!["schemas/vector.json".to_string()],
                validate_formats: Some(false),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!config.should_validate_formats("./schemas/vector.json", &[]));
    }

    #[test]
    fn should_validate_formats_first_match_wins() {
        let config = Config {
            overrides: vec![
                Override {
                    files: vec!["schemas/vector.json".to_string()],
                    validate_formats: Some(false),
                    ..Default::default()
                },
                Override {
                    files: vec!["schemas/**".to_string()],
                    validate_formats: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        // First override matches, returns false
        assert!(!config.should_validate_formats("schemas/vector.json", &[]));
        // Second override matches for other files, returns true
        assert!(config.should_validate_formats("schemas/other.json", &[]));
    }

    #[test]
    fn should_validate_formats_skips_none_override() {
        let config = Config {
            overrides: vec![
                Override {
                    files: vec!["schemas/vector.json".to_string()],
                    validate_formats: None, // no opinion
                    ..Default::default()
                },
                Override {
                    files: vec!["schemas/**".to_string()],
                    validate_formats: Some(false),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        // First override matches but has None, so falls through to second
        assert!(!config.should_validate_formats("schemas/vector.json", &[]));
    }

    // --- Override merge behavior ---

    #[test]
    fn merge_overrides_child_first() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub)?;

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/**"]
validate_formats = true
"#,
        )?;

        fs::write(
            sub.join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/vector.json"]
validate_formats = false
"#,
        )?;

        let config = find_and_load(&sub)?.expect("config should exist");
        // Child override comes first, then parent
        assert_eq!(config.overrides.len(), 2);
        assert_eq!(config.overrides[0].files, vec!["schemas/vector.json"]);
        assert_eq!(config.overrides[0].validate_formats, Some(false));
        assert_eq!(config.overrides[1].files, vec!["schemas/**"]);
        assert_eq!(config.overrides[1].validate_formats, Some(true));
        Ok(())
    }
}
