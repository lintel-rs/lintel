use std::collections::HashMap;
use std::path::{Path, PathBuf};

use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;

const CONFIG_FILENAME: &str = "lintel.toml";

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Glob patterns for instance file paths this override applies to.
    #[serde(default)]
    pub files: Vec<String>,

    /// Glob patterns for schema URIs this override applies to.
    /// Matched against the original URI (before rewrites) and the resolved
    /// URI (after rewrites and `//` resolution).
    #[serde(default)]
    pub schemas: Vec<String>,

    /// Whether to enable JSON Schema format validation for matched files.
    /// When `None`, the override does not affect format validation.
    #[serde(default)]
    pub validate_formats: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// If true, stop walking up the directory tree. No parent `lintel.toml`
    /// files will be merged.
    #[serde(default)]
    pub root: bool,

    /// Glob patterns for files to exclude from validation.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Custom schema mappings. Keys are glob patterns matching file paths;
    /// values are schema URLs to use for those files.
    ///
    /// These take priority over catalog matching but are overridden by
    /// inline `$schema` properties and YAML modeline comments.
    #[serde(default)]
    pub schemas: HashMap<String, String>,

    /// Additional schema catalog URLs to fetch alongside SchemaStore.
    /// Each URL should point to a JSON file with the same format as
    /// the SchemaStore catalog (`{"schemas": [...]}`).
    #[serde(default)]
    pub registries: Vec<String>,

    /// Schema URI rewrite rules. Keys are prefixes to match; values are
    /// replacements. For example, `"http://localhost:8000/" = "//schemas/"`
    /// rewrites any schema URI starting with `http://localhost:8000/` so that
    /// prefix becomes `//schemas/`.
    #[serde(default)]
    pub rewrite: HashMap<String, String>,

    /// Per-file overrides. Earlier entries (child configs) take priority.
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
            if file_match || schema_match {
                if let Some(val) = ov.validate_formats {
                    return val;
                }
            }
        }
        true
    }
}

/// Apply rewrite rules to a schema URI. If the URI starts with any key in
/// `rewrites`, that prefix is replaced with the corresponding value.
/// The longest matching prefix wins.
pub fn apply_rewrites(uri: &str, rewrites: &HashMap<String, String>) -> String {
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
pub fn load() -> Result<Config, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    Ok(find_and_load(&cwd)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loads_config_from_directory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"exclude = ["testdata/**"]"#,
        )
        .unwrap();

        let config = find_and_load(tmp.path()).unwrap().unwrap();
        assert_eq!(config.exclude, vec!["testdata/**"]);
    }

    #[test]
    fn walks_up_to_find_config() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a/b/c");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.path().join("lintel.toml"), r#"exclude = ["vendor/**"]"#).unwrap();

        let config = find_and_load(&sub).unwrap().unwrap();
        assert_eq!(config.exclude, vec!["vendor/**"]);
    }

    #[test]
    fn returns_none_when_no_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config = find_and_load(tmp.path()).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn empty_config_is_valid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lintel.toml"), "").unwrap();

        let config = find_and_load(tmp.path()).unwrap().unwrap();
        assert!(config.exclude.is_empty());
        assert!(config.rewrite.is_empty());
    }

    #[test]
    fn rejects_unknown_fields() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lintel.toml"), "bogus = true").unwrap();

        let result = find_and_load(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn loads_rewrite_rules() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://localhost:8000/" = "//schemastore/src/"
"#,
        )
        .unwrap();

        let config = find_and_load(tmp.path()).unwrap().unwrap();
        assert_eq!(
            config.rewrite.get("http://localhost:8000/"),
            Some(&"//schemastore/src/".to_string())
        );
    }

    // --- root = true ---

    #[test]
    fn root_true_stops_walk() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub).unwrap();

        // Parent config
        fs::write(tmp.path().join("lintel.toml"), r#"exclude = ["parent/**"]"#).unwrap();

        // Child config with root = true
        fs::write(
            sub.join("lintel.toml"),
            "root = true\nexclude = [\"child/**\"]",
        )
        .unwrap();

        let config = find_and_load(&sub).unwrap().unwrap();
        assert_eq!(config.exclude, vec!["child/**"]);
        // Parent's "parent/**" should NOT be included
    }

    #[test]
    fn merges_parent_without_root() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub).unwrap();

        // Parent config
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
exclude = ["parent/**"]

[rewrite]
"http://parent/" = "//parent/"
"#,
        )
        .unwrap();

        // Child config (no root = true)
        fs::write(
            sub.join("lintel.toml"),
            r#"
exclude = ["child/**"]

[rewrite]
"http://child/" = "//child/"
"#,
        )
        .unwrap();

        let config = find_and_load(&sub).unwrap().unwrap();
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
    }

    #[test]
    fn child_rewrite_wins_on_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub).unwrap();

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[rewrite]
"http://example/" = "//parent-value/"
"#,
        )
        .unwrap();

        fs::write(
            sub.join("lintel.toml"),
            r#"
[rewrite]
"http://example/" = "//child-value/"
"#,
        )
        .unwrap();

        let config = find_and_load(&sub).unwrap().unwrap();
        assert_eq!(
            config.rewrite.get("http://example/"),
            Some(&"//child-value/".to_string())
        );
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
    fn parses_override_blocks() {
        let tmp = tempfile::tempdir().unwrap();
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
        )
        .unwrap();

        let config = find_and_load(tmp.path()).unwrap().unwrap();
        assert_eq!(config.overrides.len(), 2);
        assert_eq!(config.overrides[0].files, vec!["schemas/vector.json"]);
        assert_eq!(config.overrides[0].validate_formats, Some(false));
        assert_eq!(config.overrides[1].validate_formats, Some(true));
    }

    #[test]
    fn override_validate_formats_defaults_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/vector.json"]
"#,
        )
        .unwrap();

        let config = find_and_load(tmp.path()).unwrap().unwrap();
        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].validate_formats, None);
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
        assert!(!config
            .should_validate_formats("any.toml", &["https://json.schemastore.org/vector.json"]));
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
    fn merge_overrides_child_first() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("child");
        fs::create_dir_all(&sub).unwrap();

        fs::write(
            tmp.path().join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/**"]
validate_formats = true
"#,
        )
        .unwrap();

        fs::write(
            sub.join("lintel.toml"),
            r#"
[[override]]
files = ["schemas/vector.json"]
validate_formats = false
"#,
        )
        .unwrap();

        let config = find_and_load(&sub).unwrap().unwrap();
        // Child override comes first, then parent
        assert_eq!(config.overrides.len(), 2);
        assert_eq!(config.overrides[0].files, vec!["schemas/vector.json"]);
        assert_eq!(config.overrides[0].validate_formats, Some(false));
        assert_eq!(config.overrides[1].files, vec!["schemas/**"]);
        assert_eq!(config.overrides[1].validate_formats, Some(true));
    }
}
