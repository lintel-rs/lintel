//! TOML formatting with special handling for `lintel-catalog.toml`.
//!
//! When the file is named `lintel-catalog.toml`, the document is sorted before
//! being formatted by dprint:
//!
//! - Top-level tables: `catalog`, `target`, `sources`, `groups`.
//! - Sub-keys of `[sources]` and `[groups]`: lexicographic.
//! - `groups.<g>.schemas` entries: lexicographic.
//! - `groups.<g>.schemas.<s>` keys: `name`, `description`, `file-match`, then
//!   remaining keys alphabetically.
//! - `groups.<g>.schemas.<s>.versions`: sorted by semver.

use alloc::borrow::Cow;
use core::cmp::Ordering;
use std::path::Path;

use anyhow::Result;
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Format TOML content. Applies catalog-specific sorting when the file is
/// named `lintel-catalog.toml`, then delegates to dprint for formatting.
pub fn format_text(
    path: &Path,
    content: &str,
    config: &dprint_plugin_toml::configuration::Configuration,
) -> Result<Option<String>> {
    let content = if is_catalog_toml(path) {
        Cow::Owned(sort_catalog(content).map_err(|e| anyhow::anyhow!("{e}"))?)
    } else {
        Cow::Borrowed(content)
    };

    dprint_plugin_toml::format_text(path, &content, config).map_err(|e| anyhow::anyhow!("{e}"))
}

// ---------------------------------------------------------------------------
// Catalog detection
// ---------------------------------------------------------------------------

fn is_catalog_toml(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "lintel-catalog.toml")
}

// ---------------------------------------------------------------------------
// Catalog sorting
// ---------------------------------------------------------------------------

fn sort_catalog(content: &str) -> Result<String, toml_edit::TomlError> {
    let mut doc: DocumentMut = content.parse()?;

    if let Some(sources) = doc.get_mut("sources").and_then(Item::as_table_mut) {
        sort_and_reposition(sources);
    }

    if let Some(groups) = doc.get_mut("groups").and_then(Item::as_table_mut) {
        sort_and_reposition(groups);
        sort_groups(groups);
    }

    // toml_edit doesn't reliably reorder top-level implicit tables via
    // position/map order, so we reorder the serialized sections instead.
    Ok(reorder_top_level_sections(&doc.to_string()))
}

// ---------------------------------------------------------------------------
// Top-level section reordering (string-based)
// ---------------------------------------------------------------------------

const TOP_LEVEL_ORDER: &[&str] = &["catalog", "target", "sources", "groups"];

fn top_level_rank(key: &str) -> usize {
    TOP_LEVEL_ORDER
        .iter()
        .position(|&k| k == key)
        .unwrap_or(TOP_LEVEL_ORDER.len())
}

/// Extract the top-level key from a TOML section header like `[groups.foo]`.
fn top_level_key(header: &str) -> Option<&str> {
    let trimmed = header.trim();
    // Skip array-of-tables [[...]]
    if trimmed.starts_with("[[") {
        return None;
    }
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?.trim();
    // Return everything before the first '.'
    Some(inner.split('.').next().unwrap_or(inner))
}

/// Reorder top-level TOML sections according to [`TOP_LEVEL_ORDER`].
///
/// Splits the serialized document into chunks, each starting with a
/// `[section]` header, groups them by top-level key, and reassembles
/// in the desired order.
fn reorder_top_level_sections(content: &str) -> String {
    // Split into (top_level_key, chunk) pairs. A "chunk" is a section
    // header plus all subsequent lines until the next header. Trailing
    // blank lines are trimmed from each chunk so whitespace is stable
    // across repeated sorts.
    let mut chunks: Vec<(&str, String)> = Vec::new();
    let mut current_key: &str = "";
    let mut current_chunk = String::new();

    for line in content.lines() {
        if let Some(key) = top_level_key(line) {
            // Flush previous chunk
            if !current_chunk.is_empty() {
                chunks.push((current_key, core::mem::take(&mut current_chunk)));
            }
            current_key = key;
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }
    if !current_chunk.is_empty() {
        chunks.push((current_key, current_chunk));
    }

    // Sort chunks by top-level rank, preserving relative order within
    // the same top-level key (stable sort).
    chunks.sort_by(|(a, _), (b, _)| top_level_rank(a).cmp(&top_level_rank(b)).then(a.cmp(b)));

    // Reassemble with a single blank line between sections.
    let mut output = String::new();
    for (_, chunk) in &chunks {
        let trimmed = chunk.trim_end();
        output.push_str(trimmed);
        output.push_str("\n\n");
    }
    output
}

// ---------------------------------------------------------------------------
// Position management
// ---------------------------------------------------------------------------

/// Recursively assign monotonically increasing positions to all table entries
/// so `toml_edit` serializes them in the order they appear in the map.
fn assign_positions(item: &mut Item, pos: &mut isize) {
    let Some(t) = item.as_table_mut() else {
        return;
    };
    t.set_position(Some(*pos));
    *pos += 1;
    for (_, child) in t.iter_mut() {
        assign_positions(child, pos);
    }
}

/// Sort a table's keys lexicographically and reassign positions.
fn sort_and_reposition(table: &mut Table) {
    table.sort_values();
    let mut pos = 0isize;
    for (_, item) in table.iter_mut() {
        assign_positions(item, &mut pos);
    }
}

// ---------------------------------------------------------------------------
// Groups-specific sorting
// ---------------------------------------------------------------------------

const SCHEMA_KEY_ORDER: &[&str] = &["name", "description", "file-match"];

fn schema_key_rank(key: &str) -> usize {
    SCHEMA_KEY_ORDER
        .iter()
        .position(|&k| k == key)
        .unwrap_or(SCHEMA_KEY_ORDER.len())
}

fn sort_groups(groups: &mut Table) {
    for (_group_key, group_item) in groups.iter_mut() {
        let Some(group_table) = group_item.as_table_mut() else {
            continue;
        };

        let Some(schemas) = group_table.get_mut("schemas").and_then(Item::as_table_mut) else {
            continue;
        };

        // Sort schema entries lexicographically
        sort_and_reposition(schemas);

        // Sort keys within each schema definition
        for (_schema_key, schema_item) in schemas.iter_mut() {
            sort_schema_definition(schema_item);
        }
    }
}

fn sort_schema_definition(item: &mut Item) {
    match item {
        Item::Value(Value::InlineTable(inline)) => {
            inline.sort_values_by(|k1, _, k2, _| {
                schema_key_rank(k1)
                    .cmp(&schema_key_rank(k2))
                    .then(k1.cmp(k2))
            });
            sort_versions_in_inline(inline);
        }
        Item::Table(table) => {
            table.sort_values_by(|k1, _, k2, _| {
                schema_key_rank(k1)
                    .cmp(&schema_key_rank(k2))
                    .then(k1.cmp(k2))
            });
            let mut pos = 0;
            for (_, child) in table.iter_mut() {
                assign_positions(child, &mut pos);
            }
            sort_versions_in_table(table);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Versions sorting (semver)
// ---------------------------------------------------------------------------

fn semver_cmp(a: &str, b: &str) -> Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b),
    }
}

fn sort_versions_in_inline(inline: &mut InlineTable) {
    if let Some(versions) = inline
        .get_mut("versions")
        .and_then(Value::as_inline_table_mut)
    {
        versions.sort_values_by(|k1, _, k2, _| semver_cmp(k1, k2));
    }
}

fn sort_versions_in_table(table: &mut Table) {
    let Some(versions_item) = table.get_mut("versions") else {
        return;
    };
    match versions_item {
        Item::Value(Value::InlineTable(inline)) => {
            inline.sort_values_by(|k1, _, k2, _| semver_cmp(k1, k2));
        }
        Item::Table(t) => {
            t.sort_values_by(|k1, _, k2, _| semver_cmp(k1, k2));
            let mut pos = 0;
            for (_, child) in t.iter_mut() {
                assign_positions(child, &mut pos);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let result = sort_catalog(input).unwrap();
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
        let first = sort_catalog(input).unwrap();
        let second = sort_catalog(&first).unwrap();
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
        assert_eq!(top_level_key("[[array]]"), None);
        assert_eq!(top_level_key("not a header"), None);
    }
}
