use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use glob_set::{Glob, GlobMap, GlobMapBuilder};

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

/// A glob entry stored in the `GlobMap`, carrying the schema URL and the original pattern.
struct GlobEntry {
    url: String,
    pattern: String,
}

/// Compiled catalog for fast filename matching.
///
/// Uses a single [`GlobMap`] with an optimized `MatchEngine` that automatically
/// dispatches to the fastest strategy per pattern (literal hash, extension hash,
/// prefix/suffix tries, Aho-Corasick pre-filter).
pub struct CompiledCatalog {
    map: GlobMap<GlobEntry>,
    url_to_entry: BTreeMap<String, CatalogEntryInfo>,
}

impl CompiledCatalog {
    /// Compile a catalog into a matcher.
    ///
    /// Entries with no `fileMatch` patterns are skipped.
    /// Negation patterns (starting with `!`) are skipped.
    /// Patterns without `/` are prepended with `**/` so they match at any depth.
    ///
    /// # Panics
    ///
    /// Panics if an empty `GlobMap` cannot be constructed (should never happen).
    pub fn compile(catalog: &crate::Catalog) -> Self {
        let mut builder = GlobMapBuilder::new();
        let mut url_to_entry: BTreeMap<String, CatalogEntryInfo> = BTreeMap::new();

        for schema in &catalog.schemas {
            let description = if schema.description.is_empty() {
                None
            } else {
                Some(schema.description.clone())
            };

            url_to_entry
                .entry(schema.url.clone())
                .or_insert_with(|| CatalogEntryInfo {
                    name: schema.name.clone(),
                    description,
                    file_match: schema.file_match.clone(),
                });

            for pattern in &schema.file_match {
                if pattern.starts_with('!') {
                    continue;
                }

                let normalized = if pattern.contains('/') {
                    pattern.clone()
                } else {
                    format!("**/{pattern}")
                };

                if let Ok(glob) = Glob::new(&normalized) {
                    builder.insert(
                        glob,
                        GlobEntry {
                            url: schema.url.clone(),
                            pattern: pattern.clone(),
                        },
                    );
                }
            }
        }

        Self {
            map: builder
                .build()
                .unwrap_or_else(|_| GlobMapBuilder::new().build().expect("empty map builds")),
            url_to_entry,
        }
    }

    /// Find the schema URL for a given file path.
    ///
    /// `path` is the full path string, `file_name` is the basename.
    /// Returns the first matching schema URL, or `None`.
    pub fn find_schema(&self, path: &str, _file_name: &str) -> Option<&str> {
        let path = path.strip_prefix("./").unwrap_or(path);
        self.map.get(path).map(|e| e.url.as_str())
    }

    /// Find the schema for a given file path, returning detailed match info.
    ///
    /// Returns the URL, the matched pattern, all `fileMatch` globs, the schema
    /// name, and the description from the catalog entry.
    pub fn find_schema_detailed<'a>(
        &'a self,
        path: &str,
        _file_name: &'a str,
    ) -> Option<SchemaMatch<'a>> {
        let path = path.strip_prefix("./").unwrap_or(path);
        let entry = self.map.get(path)?;
        let info = self.url_to_entry.get(&entry.url)?;
        Some(SchemaMatch {
            url: &entry.url,
            matched_pattern: &entry.pattern,
            file_match: &info.file_match,
            name: &info.name,
            description: info.description.as_deref(),
        })
    }

    /// Look up the human-readable schema name for a given URL.
    pub fn schema_name(&self, url: &str) -> Option<&str> {
        self.url_to_entry.get(url).map(|e| e.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::collections::BTreeMap;
    use alloc::vec;

    use super::*;
    use crate::{Catalog, SchemaEntry};

    fn test_catalog() -> Catalog {
        Catalog {
            version: 1,
            title: None,
            schemas: vec![
                SchemaEntry {
                    name: "tsconfig".into(),
                    url: "https://json.schemastore.org/tsconfig.json".into(),
                    description: String::new(),
                    source_url: None,
                    file_match: vec!["tsconfig.json".into(), "tsconfig.*.json".into()],
                    versions: BTreeMap::new(),
                },
                SchemaEntry {
                    name: "package.json".into(),
                    url: "https://json.schemastore.org/package.json".into(),
                    description: String::new(),
                    source_url: None,
                    file_match: vec!["package.json".into()],
                    versions: BTreeMap::new(),
                },
                SchemaEntry {
                    name: "no-match".into(),
                    url: "https://example.com/no-match.json".into(),
                    description: String::new(),
                    source_url: None,
                    file_match: vec![],
                    versions: BTreeMap::new(),
                },
            ],
            groups: vec![],
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
            version: 1,
            title: None,
            schemas: vec![SchemaEntry {
                name: "GitHub Workflow".into(),
                url: "https://www.schemastore.org/github-workflow.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![
                    "**/.github/workflows/*.yml".into(),
                    "**/.github/workflows/*.yaml".into(),
                ],
                versions: BTreeMap::new(),
            }],
            groups: vec![],
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
    fn empty_description_becomes_none() {
        let catalog = Catalog {
            version: 1,
            title: None,
            schemas: vec![SchemaEntry {
                name: "test".into(),
                url: "https://example.com/test.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec!["test.json".into()],
                versions: BTreeMap::new(),
            }],
            groups: vec![],
        };
        let compiled = CompiledCatalog::compile(&catalog);
        let m = compiled
            .find_schema_detailed("test.json", "test.json")
            .expect("should match");
        assert!(m.description.is_none());
    }

    #[test]
    fn non_empty_description_preserved() {
        let catalog = Catalog {
            version: 1,
            title: None,
            schemas: vec![SchemaEntry {
                name: "test".into(),
                url: "https://example.com/test.json".into(),
                description: "A test schema".into(),
                source_url: None,
                file_match: vec!["test.json".into()],
                versions: BTreeMap::new(),
            }],
            groups: vec![],
        };
        let compiled = CompiledCatalog::compile(&catalog);
        let m = compiled
            .find_schema_detailed("test.json", "test.json")
            .expect("should match");
        assert_eq!(m.description, Some("A test schema"));
    }
}
