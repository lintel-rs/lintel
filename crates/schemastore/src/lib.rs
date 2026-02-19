#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

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

/// Compiled catalog for fast filename matching.
pub struct CompiledCatalog {
    /// Each entry is (list of glob patterns, schema URL).
    entries: Vec<(Vec<String>, String)>,
}

impl CompiledCatalog {
    /// Compile a catalog into a matcher.
    ///
    /// Entries with no `fileMatch` patterns are skipped.
    /// Bare filename patterns (no `/` or `*`) are expanded to also match
    /// as `**/{pattern}` so they work with full paths.
    pub fn compile(catalog: &Catalog) -> Self {
        let mut entries = Vec::new();
        for schema in &catalog.schemas {
            if schema.file_match.is_empty() {
                continue;
            }
            let mut patterns = Vec::new();
            for pattern in &schema.file_match {
                if pattern.starts_with('!') {
                    continue;
                }
                patterns.push(pattern.clone());
                // Bare filenames: also match nested paths
                if !pattern.contains('/') && !pattern.contains('*') {
                    patterns.push(format!("**/{pattern}"));
                }
            }
            if !patterns.is_empty() {
                entries.push((patterns, schema.url.clone()));
            }
        }
        Self { entries }
    }

    /// Find the schema URL for a given file path.
    ///
    /// `path` is the full path string, `file_name` is the basename.
    /// Returns the first matching schema URL, or `None`.
    pub fn find_schema(&self, path: &str, file_name: &str) -> Option<&str> {
        for (patterns, url) in &self.entries {
            for pattern in patterns {
                if glob_match::glob_match(pattern, path)
                    || glob_match::glob_match(pattern, file_name)
                {
                    return Some(url);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::boxed::Box;
    use std::error::Error;

    fn test_catalog() -> Catalog {
        Catalog {
            schemas: alloc::vec![
                SchemaEntry {
                    name: "tsconfig".into(),
                    url: "https://json.schemastore.org/tsconfig.json".into(),
                    file_match: alloc::vec!["tsconfig.json".into(), "tsconfig.*.json".into(),],
                },
                SchemaEntry {
                    name: "package.json".into(),
                    url: "https://json.schemastore.org/package.json".into(),
                    file_match: alloc::vec!["package.json".into()],
                },
                SchemaEntry {
                    name: "no-match".into(),
                    url: "https://example.com/no-match.json".into(),
                    file_match: alloc::vec![],
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
            schemas: alloc::vec![SchemaEntry {
                name: "GitHub Workflow".into(),
                url: "https://www.schemastore.org/github-workflow.json".into(),
                file_match: alloc::vec![
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
    fn parse_catalog_from_json() -> Result<(), Box<dyn Error>> {
        let json = r#"{"schemas":[{"name":"test","url":"https://example.com/s.json","fileMatch":["*.json"]}]}"#;
        let value: serde_json::Value = serde_json::from_str(json)?;
        let catalog = parse_catalog(value)?;
        assert_eq!(catalog.schemas.len(), 1);
        assert_eq!(catalog.schemas[0].name, "test");
        Ok(())
    }
}
