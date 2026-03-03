use std::collections::HashMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Deserialize;

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

fn example_ignore_patterns() -> Vec<String> {
    vec!["vendor/**".into(), "testdata/**".into()]
}

fn example_registry() -> Vec<String> {
    vec!["https://example.com/custom-catalog.json".into()]
}

/// File selection configuration.
///
/// Controls which files Lintel processes. Patterns in `ignore-patterns`
/// exclude matching files (like `.gitignore`). Files matching any pattern
/// are skipped. When the list is empty, all files pass through.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Files")]
pub struct FilesConfig {
    /// Glob patterns for files to exclude from processing.
    ///
    /// Each pattern works like a `.gitignore` entry — files matching any
    /// pattern are skipped. For example, `vendor/**` excludes everything
    /// under the `vendor` directory.
    #[schemars(title = "Ignore Patterns", example = example_ignore_patterns())]
    #[serde(default, rename = "ignore-patterns")]
    pub ignore_patterns: Vec<String>,
}

/// Formatting configuration.
///
/// Controls how `lintel format` behaves. The `dprint` field passes
/// configuration through to the dprint-based formatters (JSON, TOML,
/// Markdown) and `pretty_yaml`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Format")]
pub struct Format {
    /// dprint formatter configuration.
    ///
    /// Global fields (`lineWidth`, `indentWidth`, `useTabs`, `newLineKind`)
    /// apply to all formatters. Per-plugin sections (`json`, `toml`,
    /// `markdown`) override the global defaults for that plugin.
    pub dprint: Option<dprint_config::DprintConfig>,
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

    /// File selection configuration.
    ///
    /// Controls which files Lintel processes via include/exclude glob patterns.
    #[schemars(title = "Files")]
    #[serde(default)]
    pub files: Option<FilesConfig>,

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

    /// Formatting configuration for `lintel format`.
    #[schemars(title = "Format")]
    #[serde(default)]
    pub format: Option<Format>,
}

impl Config {
    /// Merge a parent config into this one.  Child values take priority:
    /// - `files.ignore_patterns`: parent entries are appended (child entries come first)
    /// - `schemas`: parent entries are added only if the key is not already present
    /// - `registries`: parent entries are appended (deduped)
    /// - `rewrite`: parent entries are added only if the key is not already present
    /// - `root` is not inherited
    pub(crate) fn merge_parent(&mut self, parent: Config) {
        // Merge files.ignore_patterns: child first, parent appended.
        match (&mut self.files, parent.files) {
            (Some(child), Some(parent_files)) => {
                child.ignore_patterns.extend(parent_files.ignore_patterns);
            }
            (None, some_parent) => {
                self.files = some_parent;
            }
            (Some(_), None) => {}
        }
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
        // Child format takes priority; fall back to parent's.
        if self.format.is_none() {
            self.format = parent.format;
        }
    }

    /// Find a custom schema mapping for the given file path.
    ///
    /// Matches against the `[schemas]` table using glob patterns.
    /// Returns the schema URL if a match is found.
    pub fn find_schema_mapping(&self, path: &str, file_name: &str) -> Option<&str> {
        let path = path.strip_prefix("./").unwrap_or(path);
        for (pattern, url) in &self.schemas {
            if glob_matcher::glob_match(pattern, path)
                || glob_matcher::glob_match(pattern, file_name)
            {
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
                && ov
                    .files
                    .iter()
                    .any(|pat| glob_matcher::glob_match(pat, path));
            let schema_match = !ov.schemas.is_empty()
                && schema_uris.iter().any(|uri| {
                    ov.schemas
                        .iter()
                        .any(|pat| glob_matcher::glob_match(pat, uri))
                });
            if (file_match || schema_match)
                && let Some(val) = ov.validate_formats
            {
                return val;
            }
        }
        true
    }

    /// Collect files matching the given globs, filtering with the provided ignore set.
    ///
    /// The `filter` predicate controls which files are included during directory walks
    /// (e.g. filtering by file extension). When `globs` is empty, auto-discovers from `"."`.
    ///
    /// # Errors
    ///
    /// Returns an error if a glob pattern is invalid or a directory cannot be walked.
    pub fn collect_files(
        &self,
        globs: &[String],
        ignore_set: &glob_set::GlobSet,
        filter: impl Fn(&Path) -> bool,
    ) -> anyhow::Result<Vec<PathBuf>> {
        crate::discover::collect_files(globs, ignore_set, filter)
    }
}
