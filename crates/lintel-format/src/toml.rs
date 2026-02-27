//! TOML formatting with a generic sort engine and special handling for
//! `lintel-catalog.toml` and `lintel.toml`.
//!
//! The sort engine ([`sort_toml`]) is driven by a declarative
//! [`TomlSortConfig`] that specifies section ordering, child sorting, key
//! priority ordering, and semver-based key sorting.
//!
//! Configs: [`CATALOG_SORT`] for `lintel-catalog.toml`,
//! [`LINTEL_SORT`] for `lintel.toml`.

use alloc::borrow::Cow;
use core::cmp::Ordering;
use std::path::Path;

use anyhow::Result;
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

// ---------------------------------------------------------------------------
// Sort configuration
// ---------------------------------------------------------------------------

/// Declarative configuration for sorting a TOML document.
///
/// Path patterns use `.` as a separator and `*` as a single-segment wildcard.
/// For example, `groups.*.schemas` matches `groups.foo.schemas`.
pub struct TomlSortConfig {
    /// Top-level section order (string-based reordering after serialization).
    pub section_order: &'static [&'static str],
    /// Paths where children are sorted lexicographically.
    pub sort_children: &'static [&'static str],
    /// Paths where keys follow a priority order, rest alphabetically.
    pub key_order: &'static [(&'static str, &'static [&'static str])],
    /// Paths where keys are sorted by semver.
    pub semver_sort: &'static [&'static str],
}

/// Sort config for `lintel-catalog.toml`.
const CATALOG_SORT: TomlSortConfig = TomlSortConfig {
    section_order: &["catalog", "target", "sources", "groups"],
    sort_children: &["sources", "groups", "groups.*.schemas"],
    key_order: &[("groups.*.schemas.*", &["name", "description", "file-match"])],
    semver_sort: &["groups.*.schemas.*.versions"],
};

/// Sort config for `lintel.toml`.
///
/// Top-level keys: scalars and arrays first (`root`, `no-default-catalog`,
/// `exclude`, `registries`), then table sections (`schemas`, `rewrite`,
/// `override`, `format`).
const LINTEL_SORT: TomlSortConfig = TomlSortConfig {
    section_order: &["schemas", "rewrite", "override", "format"],
    sort_children: &["schemas", "rewrite", "format.dprint"],
    key_order: &[
        (
            "",
            &[
                "root",
                "no-default-catalog",
                "exclude",
                "registries",
                "schemas",
                "rewrite",
                "override",
                "format",
            ],
        ),
        ("override", &["files", "schemas", "validate_formats"]),
    ],
    semver_sort: &[],
};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Format TOML content. Applies sorting when the file is `lintel-catalog.toml`
/// or `lintel.toml`, then delegates to dprint for formatting.
pub fn format_text(
    path: &Path,
    content: &str,
    config: &dprint_plugin_toml::configuration::Configuration,
) -> Result<Option<String>> {
    let sort_config = if is_catalog_toml(path) {
        Some(&CATALOG_SORT)
    } else if is_lintel_toml(path) {
        Some(&LINTEL_SORT)
    } else {
        None
    };

    let content = if let Some(sort_config) = sort_config {
        Cow::Owned(sort_toml(content, sort_config).map_err(|e| anyhow::anyhow!("{e}"))?)
    } else {
        Cow::Borrowed(content)
    };

    dprint_plugin_toml::format_text(path, &content, config).map_err(|e| anyhow::anyhow!("{e}"))
}

/// Sort a TOML document according to the given configuration.
pub fn sort_toml(content: &str, config: &TomlSortConfig) -> Result<String, toml_edit::TomlError> {
    let (preamble, body) = split_preamble(content);

    let mut doc: DocumentMut = body.parse()?;

    apply_sort_table(doc.as_table_mut(), "", config);

    // One global reposition pass so positions are monotonically increasing
    // across the whole document. The per-table sorts above only reorder map
    // entries; positions must be consistent for toml_edit to serialize
    // sub-sections in the right place relative to their parents.
    reposition(doc.as_table_mut());

    let mut result = reorder_sections(&doc.to_string(), config.section_order);

    if !preamble.is_empty() {
        result.insert_str(0, &format!("{preamble}\n"));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// File detection
// ---------------------------------------------------------------------------

fn is_catalog_toml(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "lintel-catalog.toml")
}

fn is_lintel_toml(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "lintel.toml")
}

// ---------------------------------------------------------------------------
// Preamble handling
// ---------------------------------------------------------------------------

/// Split leading comment/blank lines (the "preamble") from the rest of the
/// TOML content. Returns `(preamble, body)` where `preamble` is trimmed and
/// may be empty.
fn split_preamble(content: &str) -> (String, &str) {
    let mut end = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            end += line.len() + 1; // +1 for newline
        } else {
            break;
        }
    }
    let preamble = content[..end].trim_end().to_string();
    let body = &content[end.min(content.len())..];
    (preamble, body)
}

// ---------------------------------------------------------------------------
// Path matching
// ---------------------------------------------------------------------------

/// Check whether a dotted path matches a pattern with `*` wildcards.
///
/// Each `*` matches exactly one segment. For example, `groups.*.schemas`
/// matches `groups.foo.schemas` but not `groups.foo.bar.schemas`.
fn path_matches(path: &str, pattern: &str) -> bool {
    let path_parts: Vec<&str> = path.split('.').collect();
    let pattern_parts: Vec<&str> = pattern.split('.').collect();

    if path_parts.len() != pattern_parts.len() {
        return false;
    }

    path_parts
        .iter()
        .zip(pattern_parts.iter())
        .all(|(p, q)| *q == "*" || p == q)
}

fn matches_any(path: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| path_matches(path, pattern))
}

fn find_key_order<'a>(path: &str, config: &'a TomlSortConfig) -> Option<&'a [&'a str]> {
    config
        .key_order
        .iter()
        .find(|(pattern, _)| path_matches(path, pattern))
        .map(|(_, order)| *order)
}

// ---------------------------------------------------------------------------
// Key ordering helpers
// ---------------------------------------------------------------------------

fn key_order_cmp(k1: &str, k2: &str, order: &[&str]) -> Ordering {
    let rank = |k: &str| -> usize { order.iter().position(|&o| o == k).unwrap_or(order.len()) };
    rank(k1).cmp(&rank(k2)).then(k1.cmp(k2))
}

fn section_rank(key: &str, order: &[&str]) -> usize {
    order.iter().position(|&k| k == key).unwrap_or(order.len())
}

// ---------------------------------------------------------------------------
// Semver comparison
// ---------------------------------------------------------------------------

fn semver_cmp(a: &str, b: &str) -> Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}

// ---------------------------------------------------------------------------
// Position management
// ---------------------------------------------------------------------------

/// Recursively assign monotonically increasing positions to all table entries
/// so `toml_edit` serializes them in the order they appear in the map.
fn assign_positions(item: &mut Item, pos: &mut isize) {
    if let Some(t) = item.as_table_mut() {
        t.set_position(Some(*pos));
        *pos += 1;
        for (_, child) in t.iter_mut() {
            assign_positions(child, pos);
        }
    } else if let Some(arr) = item.as_array_of_tables_mut() {
        for table in arr.iter_mut() {
            table.set_position(Some(*pos));
            *pos += 1;
            for (_, child) in table.iter_mut() {
                assign_positions(child, pos);
            }
        }
    }
}

/// Reassign positions for all entries in a table without changing their order.
fn reposition(table: &mut Table) {
    let mut pos = 0isize;
    for (_, item) in table.iter_mut() {
        assign_positions(item, &mut pos);
    }
}

// ---------------------------------------------------------------------------
// Recursive sort engine
// ---------------------------------------------------------------------------

fn apply_sort(item: &mut Item, path: &str, config: &TomlSortConfig) {
    match item {
        Item::Table(table) => apply_sort_table(table, path, config),
        Item::Value(Value::InlineTable(inline)) => apply_sort_inline(inline, path, config),
        Item::ArrayOfTables(array) => {
            for table in array.iter_mut() {
                apply_sort_table(table, path, config);
            }
        }
        _ => {}
    }
}

fn apply_sort_table(table: &mut Table, path: &str, config: &TomlSortConfig) {
    // Apply the sorting strategy that matches this path.
    if matches_any(path, config.semver_sort) {
        table.sort_values_by(|k1, _, k2, _| semver_cmp(k1, k2));
    } else if let Some(order) = find_key_order(path, config) {
        table.sort_values_by(|k1, _, k2, _| key_order_cmp(k1, k2, order));
    } else if matches_any(path, config.sort_children) {
        table.sort_values();
    }

    // Recurse into children.
    for (key, child) in table.iter_mut() {
        let child_path = if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        };
        apply_sort(child, &child_path, config);
    }
}

fn apply_sort_inline(inline: &mut InlineTable, path: &str, config: &TomlSortConfig) {
    // Apply the sorting strategy that matches this path.
    if matches_any(path, config.semver_sort) {
        inline.sort_values_by(|k1, _, k2, _| semver_cmp(k1, k2));
    } else if let Some(order) = find_key_order(path, config) {
        inline.sort_values_by(|k1, _, k2, _| key_order_cmp(k1, k2, order));
    } else if matches_any(path, config.sort_children) {
        inline.sort_values();
    }

    // Collect keys of nested inline tables for recursion.
    let keys: Vec<String> = inline
        .iter()
        .filter(|(_, v)| v.is_inline_table())
        .map(|(k, _)| k.to_string())
        .collect();

    for key in &keys {
        let child_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        if let Some(nested) = inline
            .get_mut(key.as_str())
            .and_then(Value::as_inline_table_mut)
        {
            apply_sort_inline(nested, &child_path, config);
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level section reordering (string-based)
// ---------------------------------------------------------------------------

/// Extract the top-level key from a TOML section header like `[groups.foo]`
/// or array-of-tables header like `[[override]]`.
fn top_level_key(header: &str) -> Option<&str> {
    let trimmed = header.trim();
    let inner = if let Some(s) = trimmed.strip_prefix("[[") {
        s.strip_suffix("]]")?.trim()
    } else {
        trimmed.strip_prefix('[')?.strip_suffix(']')?.trim()
    };
    // Return everything before the first '.'
    Some(inner.split('.').next().unwrap_or(inner))
}

/// Reorder top-level TOML sections according to the given order.
///
/// Splits the serialized document into chunks, each starting with a
/// `[section]` header, groups them by top-level key, and reassembles
/// in the desired order.
fn reorder_sections(content: &str, order: &[&str]) -> String {
    // Split into (top_level_key, chunk) pairs. A "chunk" is a section
    // header plus all subsequent lines until the next header. Trailing
    // blank lines are trimmed from each chunk so whitespace is stable
    // across repeated sorts.
    //
    // Lines before the first section header (comments, blank lines) are
    // kept as a preamble and always emitted first.
    let mut preamble = String::new();
    let mut chunks: Vec<(&str, String)> = Vec::new();
    let mut current_key: &str = "";
    let mut current_chunk = String::new();
    let mut seen_section = false;

    for line in content.lines() {
        if let Some(key) = top_level_key(line) {
            // Flush previous chunk
            if !current_chunk.is_empty() {
                chunks.push((current_key, core::mem::take(&mut current_chunk)));
            }
            current_key = key;
            seen_section = true;
        } else if !seen_section {
            preamble.push_str(line);
            preamble.push('\n');
            continue;
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }
    if !current_chunk.is_empty() {
        chunks.push((current_key, current_chunk));
    }

    // Sort chunks by rank, preserving relative order within the same
    // top-level key (stable sort).
    chunks.sort_by(|(a, _), (b, _)| {
        section_rank(a, order)
            .cmp(&section_rank(b, order))
            .then(a.cmp(b))
    });

    // Reassemble with the preamble first, then sections separated by
    // a single blank line.
    let mut output = String::new();
    let preamble = preamble.trim_end();
    if !preamble.is_empty() {
        output.push_str(preamble);
        output.push_str("\n\n");
    }
    for (_, chunk) in &chunks {
        let trimmed = chunk.trim_end();
        output.push_str(trimmed);
        output.push_str("\n\n");
    }
    output
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // path_matches
    // -----------------------------------------------------------------------

    #[test]
    fn path_matches_exact() {
        assert!(path_matches("sources", "sources"));
        assert!(path_matches("groups.foo.schemas", "groups.foo.schemas"));
    }

    #[test]
    fn path_matches_wildcard() {
        assert!(path_matches("groups.foo.schemas", "groups.*.schemas"));
        assert!(path_matches("groups.bar.schemas.baz", "groups.*.schemas.*"));
    }

    #[test]
    fn path_matches_no_match() {
        assert!(!path_matches("sources", "groups"));
        assert!(!path_matches("groups.foo", "groups.*.schemas"));
        assert!(!path_matches("groups.foo.bar.schemas", "groups.*.schemas"));
    }

    // -----------------------------------------------------------------------
    // key_order_cmp
    // -----------------------------------------------------------------------

    #[test]
    fn key_order_cmp_priority_before_alpha() {
        let order = &["name", "description"];
        assert_eq!(key_order_cmp("name", "description", order), Ordering::Less);
        assert_eq!(
            key_order_cmp("description", "name", order),
            Ordering::Greater
        );
    }

    #[test]
    fn key_order_cmp_priority_before_unknown() {
        let order = &["name", "description"];
        assert_eq!(key_order_cmp("name", "zzz", order), Ordering::Less);
    }

    #[test]
    fn key_order_cmp_unknown_alpha() {
        let order = &["name"];
        assert_eq!(key_order_cmp("alpha", "beta", order), Ordering::Less);
        assert_eq!(key_order_cmp("beta", "alpha", order), Ordering::Greater);
    }

    // -----------------------------------------------------------------------
    // Catalog sorting (via sort_toml + CATALOG_SORT)
    // -----------------------------------------------------------------------

    #[test]
    fn top_level_ordering() {
        let input = r#"
[groups.z]
name = "Z"
description = "Z group"

[sources.alpha]
url = "https://example.com"

[catalog]

[target.local]
type = "dir"
dir = "out"
base-url = "https://example.com/"
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let keys: Vec<&str> = doc.as_table().iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["catalog", "target", "sources", "groups"]);
    }

    #[test]
    fn groups_sorted_lexicographically() {
        let input = r#"
[catalog]

[groups.zebra]
name = "Zebra"
description = "Z group"

[groups.alpha]
name = "Alpha"
description = "A group"
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let groups = doc["groups"].as_table().unwrap();
        let keys: Vec<&str> = groups.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["alpha", "zebra"]);
    }

    #[test]
    fn sources_sorted_lexicographically() {
        let input = r#"
[catalog]

[sources.beta]
url = "https://beta.com"

[sources.alpha]
url = "https://alpha.com"
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let sources = doc["sources"].as_table().unwrap();
        let keys: Vec<&str> = sources.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["alpha", "beta"]);
    }

    #[test]
    fn schema_key_ordering() {
        let input = r#"
[catalog]

[groups.test]
name = "Test"
description = "Test group"

[groups.test.schemas.my-schema]
file-match = ["*.json"]
name = "My Schema"
url = "https://example.com/schema.json"
description = "A test schema"
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let schema = doc["groups"]["test"]["schemas"]["my-schema"]
            .as_table()
            .unwrap();
        let keys: Vec<&str> = schema.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["name", "description", "file-match", "url"]);
    }

    #[test]
    fn schema_inline_key_ordering() {
        let input = r#"
[catalog]

[groups.test]
name = "Test"
description = "Test group"

[groups.test.schemas]
foo = { file-match = ["*.json"], name = "Foo", description = "A foo" }
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let schema = doc["groups"]["test"]["schemas"]["foo"]
            .as_inline_table()
            .unwrap();
        let keys: Vec<&str> = schema.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["name", "description", "file-match"]);
    }

    #[test]
    fn versions_sorted_by_semver() {
        let input = r#"
[catalog]

[groups.test]
name = "Test"
description = "Test group"

[groups.test.schemas.my-schema]
name = "My Schema"
description = "A test schema"

[groups.test.schemas.my-schema.versions]
"2.0.0" = "https://example.com/v2"
"1.0.0" = "https://example.com/v1"
"1.2.0" = "https://example.com/v1.2"
"10.0.0" = "https://example.com/v10"
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let versions = doc["groups"]["test"]["schemas"]["my-schema"]["versions"]
            .as_table()
            .unwrap();
        let keys: Vec<&str> = versions.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["1.0.0", "1.2.0", "2.0.0", "10.0.0"]);
    }

    #[test]
    fn schemas_sorted_lexicographically() {
        let input = r#"
[catalog]

[groups.test]
name = "Test"
description = "Test group"

[groups.test.schemas]
zebra = { name = "Zebra" }
alpha = { name = "Alpha" }
middle = { name = "Middle" }
"#;
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let schemas = doc["groups"]["test"]["schemas"].as_table().unwrap();
        let keys: Vec<&str> = schemas.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn already_sorted_is_idempotent() {
        let input = r#"
[catalog]

[target.local]
type = "dir"
dir = "out"
base-url = "https://example.com/"

[sources.schemastore]
url = "https://www.schemastore.org/api/json/catalog.json"

[groups.github]
name = "GitHub"
description = "GitHub configuration files"
"#;
        let first = sort_toml(input, &CATALOG_SORT).unwrap();
        let second = sort_toml(&first, &CATALOG_SORT).unwrap();
        assert_eq!(first, second);
    }

    // -----------------------------------------------------------------------
    // split_preamble
    // -----------------------------------------------------------------------

    #[test]
    fn split_preamble_extracts_leading_comment() {
        let input = "# comment\n\n[catalog]\ntitle = \"X\"\n";
        let (pre, body) = split_preamble(input);
        assert_eq!(pre, "# comment");
        assert!(body.starts_with("[catalog]"));
    }

    #[test]
    fn split_preamble_handles_multiple_comments() {
        let input = "# line1\n# line2\n\n[catalog]\n";
        let (pre, body) = split_preamble(input);
        assert_eq!(pre, "# line1\n# line2");
        assert!(body.starts_with("[catalog]"));
    }

    #[test]
    fn split_preamble_empty_when_no_comments() {
        let input = "[catalog]\ntitle = \"X\"\n";
        let (pre, body) = split_preamble(input);
        assert!(pre.is_empty());
        assert_eq!(body, input);
    }

    #[test]
    fn split_preamble_all_comments() {
        let input = "# only comments\n# nothing else\n";
        let (pre, body) = split_preamble(input);
        assert_eq!(pre, "# only comments\n# nothing else");
        assert!(body.is_empty());
    }

    // -----------------------------------------------------------------------
    // reorder_sections – preamble preservation
    // -----------------------------------------------------------------------

    #[test]
    fn reorder_preserves_preamble_before_sections() {
        let input = "# preamble\n\n[groups.z]\nname = \"Z\"\n\n[catalog]\ntitle = \"X\"\n";
        let result = reorder_sections(input, CATALOG_SORT.section_order);
        assert!(
            result.starts_with("# preamble"),
            "preamble should stay at top, got:\n{result}"
        );
        // catalog still comes before groups after reordering
        assert!(result.find("[catalog]").unwrap() < result.find("[groups.z]").unwrap());
    }

    #[test]
    fn reorder_no_preamble_still_works() {
        let input = "[groups.z]\nname = \"Z\"\n\n[catalog]\ntitle = \"X\"\n";
        let result = reorder_sections(input, CATALOG_SORT.section_order);
        assert!(result.starts_with("[catalog]"));
    }

    // -----------------------------------------------------------------------
    // sort_toml – leading comment round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn leading_comment_preserved_at_top() {
        let input = "# :schema ./schemas/lintel/lintel-catalog-toml.json\n\n\
                      [catalog]\ntitle = \"Lintel Catalog\"\n\n\
                      [groups.test]\nname = \"Test\"\ndescription = \"Test group\"\n";
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        assert!(
            result.starts_with("# :schema"),
            "leading comment should stay at top, got:\n{result}"
        );
    }

    #[test]
    fn leading_comment_with_sections_out_of_order() {
        let input = "# :schema ./path.json\n\n\
                      [groups.z]\nname = \"Z\"\ndescription = \"Z\"\n\n\
                      [catalog]\ntitle = \"C\"\n";
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        assert!(
            result.starts_with("# :schema"),
            "comment should stay at top even when sections are reordered, got:\n{result}"
        );
        assert!(result.find("[catalog]").unwrap() < result.find("[groups.z]").unwrap());
    }

    #[test]
    fn multiple_leading_comments_preserved() {
        let input = "# :schema ./path.json\n# another comment\n\n\
                      [catalog]\ntitle = \"X\"\n";
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        assert!(result.starts_with("# :schema ./path.json\n# another comment"));
    }

    #[test]
    fn no_leading_comment_is_fine() {
        let input = "[catalog]\ntitle = \"X\"\n\n\
                      [groups.test]\nname = \"T\"\ndescription = \"T\"\n";
        let result = sort_toml(input, &CATALOG_SORT).unwrap();
        assert!(result.starts_with("[catalog]"));
    }

    #[test]
    fn leading_comment_idempotent() {
        let input = "# :schema ./path.json\n\n\
                      [catalog]\ntitle = \"X\"\n\n\
                      [groups.test]\nname = \"T\"\ndescription = \"T\"\n";
        let first = sort_toml(input, &CATALOG_SORT).unwrap();
        let second = sort_toml(&first, &CATALOG_SORT).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn format_text_detects_catalog() {
        let input = r#"
[groups.z]
name = "Z"
description = "Z"

[catalog]
"#;
        let path = Path::new("lintel-catalog.toml");
        let config = dprint_plugin_toml::configuration::ConfigurationBuilder::new().build();
        let result = format_text(path, input, &config).unwrap().unwrap();
        // After sorting, catalog should come before groups
        assert!(result.find("[catalog]").unwrap() < result.find("[groups.z]").unwrap());
    }

    #[test]
    fn format_text_skips_regular_toml() {
        let input = "[package]\nname = \"test\"\n";
        let path = Path::new("Cargo.toml");
        let config = dprint_plugin_toml::configuration::ConfigurationBuilder::new().build();
        // Should not error — just formats normally
        let _ = format_text(path, input, &config);
    }

    #[test]
    fn top_level_key_extraction() {
        assert_eq!(top_level_key("[catalog]"), Some("catalog"));
        assert_eq!(top_level_key("[groups.foo]"), Some("groups"));
        assert_eq!(top_level_key("[groups.foo.schemas]"), Some("groups"));
        assert_eq!(top_level_key("[target.local]"), Some("target"));
        assert_eq!(top_level_key("[[override]]"), Some("override"));
        assert_eq!(top_level_key("[[override.sub]]"), Some("override"));
        assert_eq!(top_level_key("not a header"), None);
    }

    // -----------------------------------------------------------------------
    // sort_toml – custom (non-catalog) config
    // -----------------------------------------------------------------------

    #[test]
    fn sort_toml_custom_config() {
        const CUSTOM: TomlSortConfig = TomlSortConfig {
            section_order: &["metadata", "dependencies"],
            sort_children: &["dependencies"],
            key_order: &[("metadata", &["name", "version"])],
            semver_sort: &[],
        };

        let input = "\
[dependencies]\nz-lib = \"1.0\"\na-lib = \"2.0\"\n\n\
[metadata]\nversion = \"1.0\"\nauthor = \"test\"\nname = \"my-pkg\"\n";

        let result = sort_toml(input, &CUSTOM).unwrap();

        // metadata before dependencies (section_order)
        assert!(
            result.find("[metadata]").unwrap() < result.find("[dependencies]").unwrap(),
            "metadata should come before dependencies, got:\n{result}"
        );

        // dependencies sorted lexicographically
        let doc: DocumentMut = result.parse().unwrap();
        let deps = doc["dependencies"].as_table().unwrap();
        let keys: Vec<&str> = deps.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["a-lib", "z-lib"]);

        // metadata keys: name, version, then rest alphabetically
        let meta = doc["metadata"].as_table().unwrap();
        let keys: Vec<&str> = meta.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["name", "version", "author"]);
    }

    // -----------------------------------------------------------------------
    // lintel.toml sorting (via sort_toml + LINTEL_SORT)
    // -----------------------------------------------------------------------

    #[test]
    fn lintel_root_key_ordering() {
        let input = "\
registries = [\"https://example.com/catalog.json\"]\n\
exclude = [\"**/testdata/**\"]\n\
root = true\n\
no-default-catalog = true\n\n\
[rewrite]\n\
\"http://localhost/\" = \"//schemas/\"\n\n\
[schemas]\n\
\"*.json\" = \"https://example.com/schema.json\"\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let keys: Vec<&str> = doc.as_table().iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            vec![
                "root",
                "no-default-catalog",
                "exclude",
                "registries",
                "schemas",
                "rewrite"
            ]
        );
    }

    #[test]
    fn lintel_section_ordering() {
        let input = "\
[format.dprint]\n\
line-width = 100\n\n\
[rewrite]\n\
\"http://localhost/\" = \"//schemas/\"\n\n\
[schemas]\n\
\"*.json\" = \"https://example.com/schema.json\"\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let schemas_pos = result.find("[schemas]").unwrap();
        let rewrite_pos = result.find("[rewrite]").unwrap();
        let format_pos = result.find("[format.dprint]").unwrap();
        assert!(
            schemas_pos < rewrite_pos && rewrite_pos < format_pos,
            "expected schemas < rewrite < format, got:\n{result}"
        );
    }

    #[test]
    fn lintel_schemas_sorted_lex() {
        let input = "\
[schemas]\n\
\"z-config/*.yaml\" = \"https://example.com/z.json\"\n\
\"a-config/*.json\" = \"https://example.com/a.json\"\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let schemas = doc["schemas"].as_table().unwrap();
        let keys: Vec<&str> = schemas.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["a-config/*.json", "z-config/*.yaml"]);
    }

    #[test]
    fn lintel_rewrite_sorted_lex() {
        let input = "\
[rewrite]\n\
\"https://json.schemastore.org/\" = \"//schemastore/\"\n\
\"http://localhost:8000/\" = \"//schemas/\"\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let rewrite = doc["rewrite"].as_table().unwrap();
        let keys: Vec<&str> = rewrite.iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            vec!["http://localhost:8000/", "https://json.schemastore.org/"]
        );
    }

    #[test]
    fn lintel_override_key_ordering() {
        let input = "\
[[override]]\n\
validate_formats = false\n\
schemas = [\"**/vector.json\"]\n\
files = [\"**/vector.json\"]\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let doc: DocumentMut = result.parse().unwrap();
        let overrides = doc["override"].as_array_of_tables().unwrap();
        let first = overrides.iter().next().unwrap();
        let keys: Vec<&str> = first.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["files", "schemas", "validate_formats"]);
    }

    #[test]
    fn lintel_override_section_position() {
        let input = "\
[[override]]\n\
files = [\"*.json\"]\n\
validate_formats = false\n\n\
[schemas]\n\
\"*.json\" = \"https://example.com/schema.json\"\n\n\
[rewrite]\n\
\"http://localhost/\" = \"//schemas/\"\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let schemas_pos = result.find("[schemas]").unwrap();
        let rewrite_pos = result.find("[rewrite]").unwrap();
        let override_pos = result.find("[[override]]").unwrap();
        assert!(
            schemas_pos < rewrite_pos && rewrite_pos < override_pos,
            "expected schemas < rewrite < override, got:\n{result}"
        );
    }

    #[test]
    fn lintel_format_dprint_plugins_sorted() {
        let input = "\
[format.dprint.toml]\n\
\"cargo.applyConventions\" = false\n\n\
[format.dprint.json]\n\
indent-width = 4\n\n\
[format.dprint]\n\
line-width = 100\n";

        let result = sort_toml(input, &LINTEL_SORT).unwrap();
        let dprint_pos = result.find("[format.dprint]").unwrap();
        let json_pos = result.find("[format.dprint.json]").unwrap();
        let toml_pos = result.find("[format.dprint.toml]").unwrap();
        assert!(
            dprint_pos < json_pos && json_pos < toml_pos,
            "expected dprint < json < toml, got:\n{result}"
        );
    }

    #[test]
    fn lintel_full_sort_idempotent() {
        let input = "\
# :schema ./schemas/lintel/lintel-toml.json\n\n\
root = true\n\
exclude = [\"**/testdata/**\"]\n\n\
[schemas]\n\
\"*.json\" = \"https://example.com/schema.json\"\n\n\
[rewrite]\n\
\"http://localhost/\" = \"//schemas/\"\n\n\
[[override]]\n\
files = [\"**/vector.json\"]\n\
schemas = [\"**/vector.json\"]\n\
validate_formats = false\n\n\
[format.dprint]\n\
line-width = 100\n";

        let first = sort_toml(input, &LINTEL_SORT).unwrap();
        let second = sort_toml(&first, &LINTEL_SORT).unwrap();
        assert_eq!(first, second, "lintel.toml sort should be idempotent");
    }

    #[test]
    fn format_text_detects_lintel_toml() {
        let input = "\
registries = [\"https://example.com\"]\n\
root = true\n";
        let path = Path::new("lintel.toml");
        let config = dprint_plugin_toml::configuration::ConfigurationBuilder::new().build();
        let result = format_text(path, input, &config).unwrap().unwrap();
        // root should come before registries after sorting
        assert!(
            result.find("root").unwrap() < result.find("registries").unwrap(),
            "root should be before registries, got:\n{result}"
        );
    }
}
