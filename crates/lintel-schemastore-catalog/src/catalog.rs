use std::collections::HashMap;

/// Slugify a schema name into a filename.
///
/// Lowercases, replaces non-alphanumeric characters with hyphens,
/// collapses consecutive hyphens, trims leading/trailing hyphens,
/// and appends `.json`.
///
/// Examples:
/// - `"Releasaurus Config"` → `"releasaurus-config.json"`
/// - `"GitHub Workflow"` → `"github-workflow.json"`
/// - `"tsconfig.json"` → `"tsconfig-json.json"`
pub fn slugify_name(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens and trim
    let mut result = String::new();
    let mut prev_hyphen = true; // treat start as hyphen to trim leading
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen
    let trimmed = result.trim_end_matches('-');
    format!("{trimmed}.json")
}

/// Build a mapping from schema URL → deduplicated filename, using the schema
/// name to derive each filename via [`slugify_name`].
///
/// When multiple schemas would produce the same filename, a numeric suffix
/// is appended (e.g. `foo.json`, `foo-2.json`, `foo-3.json`).
pub fn build_filename_map(schemas: &[schema_catalog::SchemaEntry]) -> HashMap<String, String> {
    let mut url_to_filename: HashMap<String, String> = HashMap::new();
    let mut filename_counts: HashMap<String, usize> = HashMap::new();
    let mut seen_urls = std::collections::HashSet::new();

    for entry in schemas {
        if !seen_urls.insert(&entry.url) {
            continue;
        }

        let base = slugify_name(&entry.name);
        let count = filename_counts.entry(base.clone()).or_insert(0);
        *count += 1;

        let filename = if *count == 1 {
            base
        } else {
            // Insert suffix before .json: "foo.json" → "foo-2.json"
            format!("{}-{count}.json", base.trim_end_matches(".json"))
        };

        url_to_filename.insert(entry.url.clone(), filename);
    }

    url_to_filename
}

/// Rewrite all `url` fields in a catalog `serde_json::Value` so they point
/// to `{base_url}/{filename}` for every schema we successfully downloaded.
///
/// `url_to_filename` maps original URLs to their local filenames.
/// `downloaded` is the set of filenames that were written to disk.
/// URLs not in the map or whose file was not downloaded are left untouched
/// (graceful fallback to the original `SchemaStore` URL).
pub fn rewrite_catalog_urls(
    catalog: &mut serde_json::Value,
    base_url: &str,
    url_to_filename: &HashMap<String, String>,
    downloaded: &std::collections::HashSet<String>,
) {
    let Some(schemas) = catalog.get_mut("schemas").and_then(|s| s.as_array_mut()) else {
        return;
    };

    let base = base_url.trim_end_matches('/');

    for entry in schemas {
        let Some(url_val) = entry.get_mut("url") else {
            continue;
        };
        let Some(original) = url_val.as_str().map(String::from) else {
            continue;
        };
        if let Some(filename) = url_to_filename.get(&original)
            && downloaded.contains(filename)
        {
            *url_val = serde_json::Value::String(format!("{base}/{filename}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn slugify_simple_name() {
        assert_eq!(slugify_name("tsconfig"), "tsconfig.json");
    }

    #[test]
    fn slugify_name_with_spaces() {
        assert_eq!(
            slugify_name("Releasaurus Config"),
            "releasaurus-config.json"
        );
    }

    #[test]
    fn slugify_name_with_dots() {
        assert_eq!(slugify_name("package.json"), "package-json.json");
    }

    #[test]
    fn slugify_name_with_special_chars() {
        assert_eq!(
            slugify_name("GitHub Workflow (CI/CD)"),
            "github-workflow-ci-cd.json"
        );
    }

    #[test]
    fn slugify_collapses_hyphens() {
        assert_eq!(slugify_name("foo - bar"), "foo-bar.json");
    }

    #[test]
    fn slugify_trims_leading_trailing() {
        assert_eq!(slugify_name("  hello  "), "hello.json");
    }

    #[test]
    fn build_map_deduplicates_same_url() {
        let schemas = vec![
            schema_catalog::SchemaEntry {
                name: "Foo".into(),
                url: "https://example.com/foo.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![],
                versions: BTreeMap::default(),
            },
            schema_catalog::SchemaEntry {
                name: "Foo Again".into(),
                url: "https://example.com/foo.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![],
                versions: BTreeMap::default(),
            },
        ];
        let map = build_filename_map(&schemas);
        assert_eq!(map.len(), 1);
        assert_eq!(map["https://example.com/foo.json"], "foo.json");
    }

    #[test]
    fn build_map_handles_name_collisions() {
        let schemas = vec![
            schema_catalog::SchemaEntry {
                name: "Foo".into(),
                url: "https://example.com/a.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![],
                versions: BTreeMap::default(),
            },
            schema_catalog::SchemaEntry {
                name: "Foo".into(),
                url: "https://example.com/b.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![],
                versions: BTreeMap::default(),
            },
            schema_catalog::SchemaEntry {
                name: "Foo".into(),
                url: "https://example.com/c.json".into(),
                description: String::new(),
                source_url: None,
                file_match: vec![],
                versions: BTreeMap::default(),
            },
        ];
        let map = build_filename_map(&schemas);
        assert_eq!(map.len(), 3);
        assert_eq!(map["https://example.com/a.json"], "foo.json");
        assert_eq!(map["https://example.com/b.json"], "foo-2.json");
        assert_eq!(map["https://example.com/c.json"], "foo-3.json");
    }

    #[test]
    fn rewrite_replaces_downloaded_urls() {
        let mut catalog = serde_json::json!({
            "schemas": [
                {"name": "tsconfig", "url": "https://json.schemastore.org/tsconfig.json"},
                {"name": "missing", "url": "https://json.schemastore.org/missing.json"},
            ]
        });

        let url_to_filename: HashMap<String, String> = [
            (
                "https://json.schemastore.org/tsconfig.json".into(),
                "tsconfig.json".into(),
            ),
            (
                "https://json.schemastore.org/missing.json".into(),
                "missing.json".into(),
            ),
        ]
        .into_iter()
        .collect();

        let downloaded: std::collections::HashSet<String> =
            ["tsconfig.json".to_string()].into_iter().collect();

        rewrite_catalog_urls(
            &mut catalog,
            "https://example.com/schemas",
            &url_to_filename,
            &downloaded,
        );

        let schemas = catalog["schemas"].as_array().expect("schemas array");
        assert_eq!(
            schemas[0]["url"],
            "https://example.com/schemas/tsconfig.json"
        );
        // missing.json was not downloaded, so URL is unchanged
        assert_eq!(
            schemas[1]["url"],
            "https://json.schemastore.org/missing.json"
        );
    }

    #[test]
    fn rewrite_trims_trailing_slash() {
        let mut catalog = serde_json::json!({
            "schemas": [
                {"name": "test", "url": "https://json.schemastore.org/test.json"},
            ]
        });
        let url_to_filename: HashMap<String, String> = [(
            "https://json.schemastore.org/test.json".into(),
            "test.json".into(),
        )]
        .into_iter()
        .collect();
        let downloaded: std::collections::HashSet<String> =
            ["test.json".to_string()].into_iter().collect();

        rewrite_catalog_urls(
            &mut catalog,
            "https://example.com/schemas/",
            &url_to_filename,
            &downloaded,
        );

        assert_eq!(
            catalog["schemas"][0]["url"],
            "https://example.com/schemas/test.json"
        );
    }
}
