#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::collections::BTreeMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

/// The URL of the `SchemaStore` catalog.
pub const CATALOG_URL: &str = "https://www.schemastore.org/api/json/catalog.json";

/// The deserialized `SchemaStore` catalog.
#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub schemas: Vec<SchemaEntry>,
}

/// A single schema entry from the catalog.
#[derive(Debug, Deserialize)]
pub struct SchemaEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "fileMatch")]
    pub file_match: Vec<String>,
}

/// Parse a `SchemaStore` catalog from a `serde_json::Value`.
///
/// # Errors
///
/// Returns an error if the value does not match the expected catalog schema.
pub fn parse_catalog(value: serde_json::Value) -> Result<Catalog, serde_json::Error> {
    serde_json::from_value(value)
}

/// A compiled `GlobSet` paired with the schema URL and original pattern for each index.
struct CompiledGlobSet {
    set: GlobSet,
    /// `(url, original_pattern)` for each compiled glob, in index order.
    entries: Vec<(String, String)>,
}

impl CompiledGlobSet {
    /// Build from a list of `(pattern, url)` pairs.
    /// Invalid patterns are silently skipped.
    fn build(patterns: &[(String, String)]) -> Self {
        let mut builder = GlobSetBuilder::new();
        let mut entries = Vec::new();
        for (pattern, url) in patterns {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
                entries.push((url.clone(), pattern.clone()));
            }
        }
        Self {
            set: builder.build().unwrap_or_else(|_| GlobSet::empty()),
            entries,
        }
    }

    /// Return the URL of the first matching pattern, or `None`.
    fn find_match(&self, path: &str, file_name: &str) -> Option<&str> {
        let matches = self.set.matches(path);
        if let Some(&idx) = matches.first() {
            return Some(&self.entries[idx].0);
        }
        let matches = self.set.matches(file_name);
        if let Some(&idx) = matches.first() {
            return Some(&self.entries[idx].0);
        }
        None
    }

    /// Return the `(url, matched_pattern)` for the first matching pattern, or `None`.
    fn find_match_detailed(&self, path: &str, file_name: &str) -> Option<(&str, &str)> {
        let matches = self.set.matches(path);
        if let Some(&idx) = matches.first() {
            let (url, pat) = &self.entries[idx];
            return Some((url, pat));
        }
        let matches = self.set.matches(file_name);
        if let Some(&idx) = matches.first() {
            let (url, pat) = &self.entries[idx];
            return Some((url, pat));
        }
        None
    }
}

/// Details about a catalog entry, stored for detailed match lookups.
#[derive(Debug, Clone)]
struct CatalogEntryInfo {
    name: String,
    description: Option<String>,
    file_match: Vec<String>,
}

/// Information about how a schema was matched from a catalog.
#[derive(Debug)]
pub struct SchemaMatch<'a> {
    /// The schema URL.
    pub url: &'a str,
    /// The specific glob pattern (or exact filename) that matched.
    pub matched_pattern: &'a str,
    /// All `fileMatch` globs from the catalog entry.
    pub file_match: &'a [String],
    /// Human-readable schema name from the catalog.
    pub name: &'a str,
    /// Description from the catalog entry, if present.
    pub description: Option<&'a str>,
}

/// Compiled catalog for fast filename matching.
///
/// Uses a three-tier lookup to avoid brute-force glob matching:
/// 1. **Exact filename** — O(log n) `BTreeMap` lookup for bare filenames
/// 2. **Extension-indexed `GlobSet`s** — compiled automaton per extension
/// 3. **Fallback `GlobSet`** — compiled automaton for patterns that can't be indexed
pub struct CompiledCatalog {
    /// Tier 1: exact filename → schema URL.
    exact_filename: BTreeMap<String, String>,
    /// Tier 2: file extension → compiled glob set with URLs.
    extension_sets: BTreeMap<String, CompiledGlobSet>,
    /// Tier 3: patterns that can't be classified into the above tiers.
    fallback_set: CompiledGlobSet,
    /// Reverse lookup: schema URL → schema name.
    url_to_name: BTreeMap<String, String>,
    /// Reverse lookup: schema URL → catalog entry info.
    url_to_entry: BTreeMap<String, CatalogEntryInfo>,
}

/// Returns `true` if the pattern is a bare filename (no glob meta-characters or path separators).
fn is_bare_filename(pattern: &str) -> bool {
    !pattern.contains('/')
        && !pattern.contains('*')
        && !pattern.contains('?')
        && !pattern.contains('[')
}

/// Try to extract a file extension from a glob pattern.
///
/// Looks for the last `.ext` segment where `ext` is alphanumeric (e.g. `.yml`, `.json`).
/// Returns `None` for patterns like `*` or `Dockerfile` with no extension.
fn extract_extension(pattern: &str) -> Option<&str> {
    let file_part = pattern.rsplit('/').next().unwrap_or(pattern);
    let dot_pos = file_part.rfind('.')?;
    let ext = &file_part[dot_pos..];
    // Only index clean extensions (no glob chars inside the extension)
    if ext.contains('*') || ext.contains('?') || ext.contains('[') {
        return None;
    }
    // Map back to the original pattern slice
    let offset = pattern.len() - file_part.len() + dot_pos;
    Some(&pattern[offset..])
}

impl CompiledCatalog {
    /// Compile a catalog into a tiered matcher.
    ///
    /// Entries with no `fileMatch` patterns are skipped.
    /// Bare filename patterns are stored in an exact-match `BTreeMap`.
    /// Patterns with a deterministic extension are compiled into per-extension `GlobSet`s.
    /// Everything else goes into a fallback `GlobSet`.
    pub fn compile(catalog: &Catalog) -> Self {
        let mut exact_filename: BTreeMap<String, String> = BTreeMap::new();
        let mut ext_patterns: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        let mut fallback_patterns: Vec<(String, String)> = Vec::new();
        let mut url_to_name: BTreeMap<String, String> = BTreeMap::new();
        let mut url_to_entry: BTreeMap<String, CatalogEntryInfo> = BTreeMap::new();

        for schema in &catalog.schemas {
            url_to_name
                .entry(schema.url.clone())
                .or_insert_with(|| schema.name.clone());
            url_to_entry
                .entry(schema.url.clone())
                .or_insert_with(|| CatalogEntryInfo {
                    name: schema.name.clone(),
                    description: schema.description.clone(),
                    file_match: schema.file_match.clone(),
                });

            for pattern in &schema.file_match {
                if pattern.starts_with('!') {
                    continue;
                }

                if is_bare_filename(pattern) {
                    exact_filename
                        .entry(pattern.clone())
                        .or_insert_with(|| schema.url.clone());
                } else if let Some(ext) = extract_extension(pattern) {
                    ext_patterns
                        .entry(ext.to_ascii_lowercase())
                        .or_default()
                        .push((pattern.clone(), schema.url.clone()));
                } else {
                    fallback_patterns.push((pattern.clone(), schema.url.clone()));
                }
            }
        }

        let extension_sets = ext_patterns
            .into_iter()
            .map(|(ext, patterns)| (ext, CompiledGlobSet::build(&patterns)))
            .collect();

        Self {
            exact_filename,
            extension_sets,
            fallback_set: CompiledGlobSet::build(&fallback_patterns),
            url_to_name,
            url_to_entry,
        }
    }

    /// Find the schema URL for a given file path.
    ///
    /// `path` is the full path string, `file_name` is the basename.
    /// Returns the first matching schema URL, or `None`.
    pub fn find_schema(&self, path: &str, file_name: &str) -> Option<&str> {
        // Tier 1: exact filename lookup
        if let Some(url) = self.exact_filename.get(file_name) {
            return Some(url);
        }

        // Tier 2: extension-indexed GlobSet
        if let Some(dot_pos) = file_name.rfind('.') {
            let ext = &file_name[dot_pos..];
            if let Some(compiled) = self.extension_sets.get(&ext.to_ascii_lowercase())
                && let Some(url) = compiled.find_match(path, file_name)
            {
                return Some(url);
            }
        }

        // Tier 3: fallback GlobSet
        self.fallback_set.find_match(path, file_name)
    }

    /// Find the schema for a given file path, returning detailed match info.
    ///
    /// Returns the URL, the matched pattern, all `fileMatch` globs, the schema
    /// name, and the description from the catalog entry.
    pub fn find_schema_detailed<'a>(
        &'a self,
        path: &str,
        file_name: &'a str,
    ) -> Option<SchemaMatch<'a>> {
        // Tier 1: exact filename lookup
        if let Some(url) = self.exact_filename.get(file_name)
            && let Some(entry) = self.url_to_entry.get(url.as_str())
        {
            return Some(SchemaMatch {
                url,
                matched_pattern: file_name,
                file_match: &entry.file_match,
                name: &entry.name,
                description: entry.description.as_deref(),
            });
        }

        // Tier 2: extension-indexed GlobSet
        if let Some(dot_pos) = file_name.rfind('.') {
            let ext = &file_name[dot_pos..];
            if let Some(compiled) = self.extension_sets.get(&ext.to_ascii_lowercase())
                && let Some((url, pattern)) = compiled.find_match_detailed(path, file_name)
                && let Some(entry) = self.url_to_entry.get(url)
            {
                return Some(SchemaMatch {
                    url,
                    matched_pattern: pattern,
                    file_match: &entry.file_match,
                    name: &entry.name,
                    description: entry.description.as_deref(),
                });
            }
        }

        // Tier 3: fallback GlobSet
        if let Some((url, pattern)) = self.fallback_set.find_match_detailed(path, file_name)
            && let Some(entry) = self.url_to_entry.get(url)
        {
            return Some(SchemaMatch {
                url,
                matched_pattern: pattern,
                file_match: &entry.file_match,
                name: &entry.name,
                description: entry.description.as_deref(),
            });
        }

        None
    }

    /// Look up the human-readable schema name for a given URL.
    pub fn schema_name(&self, url: &str) -> Option<&str> {
        self.url_to_name.get(url).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> Catalog {
        Catalog {
            schemas: vec![
                SchemaEntry {
                    name: "tsconfig".into(),
                    url: "https://json.schemastore.org/tsconfig.json".into(),
                    description: None,
                    file_match: vec!["tsconfig.json".into(), "tsconfig.*.json".into()],
                },
                SchemaEntry {
                    name: "package.json".into(),
                    url: "https://json.schemastore.org/package.json".into(),
                    description: None,
                    file_match: vec!["package.json".into()],
                },
                SchemaEntry {
                    name: "no-match".into(),
                    url: "https://example.com/no-match.json".into(),
                    description: None,
                    file_match: vec![],
                },
            ],
        }
    }

    #[test]
    fn compile_and_match_basename() {
        let catalog = test_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema("tsconfig.json", "tsconfig.json"),
            Some("https://json.schemastore.org/tsconfig.json")
        );
    }

    #[test]
    fn compile_and_match_with_path() {
        let catalog = test_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema("project/tsconfig.json", "tsconfig.json"),
            Some("https://json.schemastore.org/tsconfig.json")
        );
    }

    #[test]
    fn compile_and_match_glob_pattern() {
        let catalog = test_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema("tsconfig.build.json", "tsconfig.build.json"),
            Some("https://json.schemastore.org/tsconfig.json")
        );
    }

    #[test]
    fn no_match_returns_none() {
        let catalog = test_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert!(
            compiled
                .find_schema("unknown.json", "unknown.json")
                .is_none()
        );
    }

    #[test]
    fn empty_file_match_skipped() {
        let catalog = test_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert!(
            compiled
                .find_schema("no-match.json", "no-match.json")
                .is_none()
        );
    }

    fn github_workflow_catalog() -> Catalog {
        Catalog {
            schemas: vec![SchemaEntry {
                name: "GitHub Workflow".into(),
                url: "https://www.schemastore.org/github-workflow.json".into(),
                description: None,
                file_match: vec![
                    "**/.github/workflows/*.yml".into(),
                    "**/.github/workflows/*.yaml".into(),
                ],
            }],
        }
    }

    #[test]
    fn github_workflow_matches_relative_path() {
        let catalog = github_workflow_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema(".github/workflows/ci.yml", "ci.yml"),
            Some("https://www.schemastore.org/github-workflow.json")
        );
    }

    #[test]
    fn github_workflow_matches_dot_slash_prefix() {
        let catalog = github_workflow_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema("./.github/workflows/ci.yml", "ci.yml"),
            Some("https://www.schemastore.org/github-workflow.json")
        );
    }

    #[test]
    fn github_workflow_matches_nested() {
        let catalog = github_workflow_catalog();
        let compiled = CompiledCatalog::compile(&catalog);

        assert_eq!(
            compiled.find_schema("myproject/.github/workflows/deploy.yaml", "deploy.yaml"),
            Some("https://www.schemastore.org/github-workflow.json")
        );
    }

    #[test]
    fn parse_catalog_from_json() -> anyhow::Result<()> {
        let json = r#"{"schemas":[{"name":"test","url":"https://example.com/s.json","fileMatch":["*.json"]}]}"#;
        let value: serde_json::Value = serde_json::from_str(json)?;
        let catalog = parse_catalog(value)?;
        assert_eq!(catalog.schemas.len(), 1);
        assert_eq!(catalog.schemas[0].name, "test");
        Ok(())
    }
}
