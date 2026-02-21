use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

use crate::workspace::WorkspaceInfo;

/// Metadata extracted from the crate's `Cargo.toml` after processing.
pub struct CrateMetadata {
    pub name: String,
    pub description: Option<String>,
}

// --- Diagnostic types ---

#[derive(Debug, Error, Diagnostic)]
#[error("[package] description is missing")]
#[diagnostic(
    code(furnish::missing_description),
    help("cargo furnish update --description \"...\" {crate_name}")
)]
pub struct MissingDescription {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("add description after this field")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("[package] keywords is missing")]
#[diagnostic(
    code(furnish::missing_keywords),
    help("cargo furnish update --keywords \"...\" {crate_name}")
)]
pub struct MissingKeywords {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("add keywords after this field")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("[package] categories is missing")]
#[diagnostic(
    code(furnish::missing_categories),
    help("cargo furnish update --categories \"...\" {crate_name}")
)]
pub struct MissingCategories {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("add categories after this field")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("[package] field ordering is wrong (expected: {expected})")]
#[diagnostic(
    code(furnish::field_order),
    severity(Warning),
    help("autofixable with cargo furnish check --fix")
)]
pub struct WrongFieldOrder {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("fields are in wrong order here")]
    pub span: SourceSpan,
    pub expected: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("{field} should use .workspace = true")]
#[diagnostic(
    code(furnish::not_workspace_inherited),
    severity(Warning),
    help("autofixable with cargo furnish check --fix")
)]
pub struct NotWorkspaceInherited {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("should be {field}.workspace = true")]
    pub span: SourceSpan,
    pub field: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("top-level section ordering is wrong (expected: {expected})")]
#[diagnostic(
    code(furnish::section_order),
    severity(Warning),
    help("autofixable with cargo furnish check --fix")
)]
pub struct WrongSectionOrder {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("sections are in wrong order")]
    pub span: SourceSpan,
    pub expected: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("[lints] section is missing")]
#[diagnostic(
    code(furnish::missing_lints),
    help("autofixable with cargo furnish check --fix")
)]
pub struct MissingLints {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("no [lints] section found")]
    pub span: SourceSpan,
}

#[derive(Debug, Error, Diagnostic)]
#[error("unnecessary `readme` field (Cargo auto-discovers README.md)")]
#[diagnostic(
    code(furnish::unnecessary_readme),
    severity(Warning),
    help("autofixable with cargo furnish check --fix")
)]
pub struct UnnecessaryReadme {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("remove this field")]
    pub span: SourceSpan,
}

/// Fields that should use `.workspace = true` when the workspace defines them.
const WORKSPACE_INHERITABLE: &[&str] = &["edition", "authors", "license", "repository", "homepage"];

/// Desired order of fields within `[package]`.
const PACKAGE_FIELD_ORDER: &[&str] = &[
    "name",
    "version",
    "edition",
    "authors",
    "description",
    "license",
    "repository",
    "homepage",
    "keywords",
    "categories",
];

/// Desired order of top-level sections.
const SECTION_ORDER: &[&str] = &[
    "package",
    "lints",
    "dependencies",
    "build-dependencies",
    "dev-dependencies",
];

/// Find the byte offset and length of a line containing `needle` in `content`.
fn find_line_span(content: &str, needle: &str) -> (usize, usize) {
    for (offset, line) in line_offsets(content) {
        if line.contains(needle) {
            return (offset, line.len());
        }
    }
    (0, content.lines().next().map_or(1, |l| l.len().max(1)))
}

/// Find the span of the last present field that precedes `missing_field` in `PACKAGE_FIELD_ORDER`.
/// Falls back to the `[package]` line if no predecessor is found.
fn find_insertion_span(content: &str, package: &Table, missing_field: &str) -> (usize, usize) {
    let idx = PACKAGE_FIELD_ORDER
        .iter()
        .position(|&f| f == missing_field)
        .unwrap_or(0);

    // Walk backwards through the canonical order to find the last present predecessor
    for &predecessor in PACKAGE_FIELD_ORDER[..idx].iter().rev() {
        if package.get(predecessor).is_some() {
            // For dotted keys like `edition.workspace = true`, search for `edition`
            return find_line_span(content, predecessor);
        }
    }

    find_line_span(content, "[package]")
}

/// Iterate over (`byte_offset`, `line_text`) pairs.
fn line_offsets(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    content.lines().map(move |line| {
        let o = offset;
        offset += line.len() + 1; // +1 for newline
        (o, line)
    })
}

/// Check a crate's Cargo.toml and return diagnostics. Does not modify files.
pub fn check_cargo_toml(
    crate_dir: &Path,
    ws: &WorkspaceInfo,
) -> Result<(CrateMetadata, Vec<Box<dyn Diagnostic + Send + Sync>>)> {
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .with_context(|| format!("failed to read {}", cargo_toml_path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", cargo_toml_path.display()))?;
    let file_name = cargo_toml_path.display().to_string();

    let package = doc
        .get("package")
        .and_then(|p| p.as_table())
        .context("Cargo.toml has no [package] table")?;

    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .context("package.name is missing")?
        .to_string();

    let description = package
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut diagnostics: Vec<Box<dyn Diagnostic + Send + Sync>> = Vec::new();
    let src = |content: &str| NamedSource::new(file_name.clone(), content.to_string());

    let (pkg_offset, pkg_len) = find_line_span(&content, "[package]");

    check_package_fields(
        &content,
        package,
        &name,
        description.as_ref(),
        ws,
        &src,
        &mut diagnostics,
    );

    check_field_and_section_order(
        &content,
        &doc,
        package,
        ws,
        &src,
        (pkg_offset, pkg_len),
        &mut diagnostics,
    );

    Ok((CrateMetadata { name, description }, diagnostics))
}

fn check_package_fields(
    content: &str,
    package: &Table,
    name: &str,
    description: Option<&String>,
    ws: &WorkspaceInfo,
    src: &dyn Fn(&str) -> NamedSource<String>,
    diagnostics: &mut Vec<Box<dyn Diagnostic + Send + Sync>>,
) {
    if package.get("readme").is_some() {
        let (offset, len) = find_line_span(content, "readme");
        diagnostics.push(Box::new(UnnecessaryReadme {
            src: src(content),
            span: (offset, len).into(),
        }));
    }

    if description.is_none() {
        let (offset, len) = find_insertion_span(content, package, "description");
        diagnostics.push(Box::new(MissingDescription {
            src: src(content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }
    if get_string_array(package, "keywords").is_none() {
        let (offset, len) = find_insertion_span(content, package, "keywords");
        diagnostics.push(Box::new(MissingKeywords {
            src: src(content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }
    if get_string_array(package, "categories").is_none() {
        let (offset, len) = find_insertion_span(content, package, "categories");
        diagnostics.push(Box::new(MissingCategories {
            src: src(content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }

    for &field in WORKSPACE_INHERITABLE {
        if ws.package_fields.contains(field)
            && let Some(item) = package.get(field)
            && !is_workspace_true(item)
        {
            let (offset, len) = find_line_span(content, &format!("{field} ="));
            diagnostics.push(Box::new(NotWorkspaceInherited {
                src: src(content),
                span: (offset, len).into(),
                field: field.to_string(),
            }));
        }
    }
}

fn check_field_and_section_order(
    content: &str,
    doc: &DocumentMut,
    package: &Table,
    ws: &WorkspaceInfo,
    src: &dyn Fn(&str) -> NamedSource<String>,
    (pkg_offset, pkg_len): (usize, usize),
    diagnostics: &mut Vec<Box<dyn Diagnostic + Send + Sync>>,
) {
    let known_fields: HashSet<&str> = PACKAGE_FIELD_ORDER.iter().copied().collect();
    let current_fields: Vec<&str> = package
        .iter()
        .map(|(k, _)| k)
        .filter(|k| known_fields.contains(k))
        .collect();
    let expected_fields: Vec<&str> = PACKAGE_FIELD_ORDER
        .iter()
        .copied()
        .filter(|f| package.get(f).is_some())
        .collect();
    if current_fields != expected_fields {
        diagnostics.push(Box::new(WrongFieldOrder {
            src: src(content),
            span: (pkg_offset, pkg_len).into(),
            expected: expected_fields.join(", "),
        }));
    }

    if ws.has_workspace_lints && doc.get("lints").is_none() {
        diagnostics.push(Box::new(MissingLints {
            src: src(content),
            span: (pkg_offset, pkg_len).into(),
        }));
    }

    let current_sections: Vec<&str> = doc
        .as_table()
        .iter()
        .map(|(k, _)| k)
        .filter(|k| SECTION_ORDER.contains(k))
        .collect();
    let expected_sections: Vec<&str> = SECTION_ORDER
        .iter()
        .copied()
        .filter(|s| doc.get(s).is_some() || (*s == "lints" && ws.has_workspace_lints))
        .collect();
    if current_sections != expected_sections {
        diagnostics.push(Box::new(WrongSectionOrder {
            src: src(content),
            span: (pkg_offset, pkg_len).into(),
            expected: expected_sections.join(", "),
        }));
    }
}

/// Resolve final metadata values, preferring existing or forced values.
struct ResolvedMetadata {
    description: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

fn resolve_metadata(
    existing_description: Option<String>,
    existing_keywords: Option<Vec<String>>,
    existing_categories: Option<Vec<String>>,
    description: Option<&str>,
    keywords: Option<&[String]>,
    categories: Option<&[String]>,
    force: bool,
) -> ResolvedMetadata {
    let description = if force {
        description.map(String::from).or(existing_description)
    } else {
        existing_description.or_else(|| description.map(String::from))
    };

    let keywords = if force && keywords.is_some() {
        keywords.map(<[String]>::to_vec)
    } else if existing_keywords.is_some() {
        existing_keywords
    } else {
        keywords.map(<[String]>::to_vec)
    };

    let categories = if force && categories.is_some() {
        categories.map(<[String]>::to_vec)
    } else if existing_categories.is_some() {
        existing_categories
    } else {
        categories.map(<[String]>::to_vec)
    };

    ResolvedMetadata {
        description,
        keywords,
        categories,
    }
}

fn rebuild_package_fields(
    package: &mut Table,
    original: &DocumentMut,
    ws: &WorkspaceInfo,
    name: &str,
    version: &str,
    meta: &ResolvedMetadata,
    force: bool,
) {
    let known_fields: HashSet<&str> = PACKAGE_FIELD_ORDER.iter().copied().collect();
    let unknown_fields: Vec<(String, Item)> = package
        .iter()
        .filter(|(k, _)| !known_fields.contains(k))
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();

    let keys_to_remove: Vec<String> = package.iter().map(|(k, _)| k.to_string()).collect();
    for key in &keys_to_remove {
        package.remove(key);
    }

    package.insert("name", toml_edit::value(name));
    package.insert("version", toml_edit::value(version));

    for field in &PACKAGE_FIELD_ORDER[2..] {
        match *field {
            "description" => {
                if let Some(ref desc) = meta.description {
                    package.insert("description", toml_edit::value(desc.as_str()));
                }
            }
            "keywords" => {
                if let Some(ref kws) = meta.keywords {
                    let arr: toml_edit::Array =
                        kws.iter().map(|s| Value::from(s.as_str())).collect();
                    package.insert("keywords", Item::Value(Value::Array(arr)));
                }
            }
            "categories" => {
                if let Some(ref cats) = meta.categories {
                    let arr: toml_edit::Array =
                        cats.iter().map(|s| Value::from(s.as_str())).collect();
                    package.insert("categories", Item::Value(Value::Array(arr)));
                }
            }
            f if WORKSPACE_INHERITABLE.contains(&f) => {
                insert_workspace_field(package, original, ws, f, force);
            }
            _ => {}
        }
    }

    for (key, val) in &unknown_fields {
        package.insert(key, val.clone());
    }
}

fn insert_workspace_field(
    package: &mut Table,
    original: &DocumentMut,
    ws: &WorkspaceInfo,
    field: &str,
    force: bool,
) {
    if !ws.package_fields.contains(field) {
        return;
    }
    if force {
        package.insert(field, workspace_true());
        return;
    }
    let orig_item = original
        .get("package")
        .and_then(|p| p.as_table())
        .and_then(|t| t.get(field));
    match orig_item {
        Some(orig) if is_workspace_true(orig) => {
            package.insert(field, workspace_true());
        }
        Some(orig) => {
            package.insert(field, orig.clone());
        }
        None => {
            package.insert(field, workspace_true());
        }
    }
}

/// Fix a crate's Cargo.toml — reorder fields/sections and fill in missing values.
pub fn fix_cargo_toml(
    crate_dir: &Path,
    ws: &WorkspaceInfo,
    description: Option<&str>,
    keywords: Option<&[String]>,
    categories: Option<&[String]>,
    force: bool,
) -> Result<CrateMetadata> {
    let cargo_toml_path = crate_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .with_context(|| format!("failed to read {}", cargo_toml_path.display()))?;
    let original = content
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", cargo_toml_path.display()))?;
    let mut doc = original.clone();

    let package = doc
        .get_mut("package")
        .and_then(|p| p.as_table_mut())
        .context("Cargo.toml has no [package] table")?;

    package.remove("readme");

    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .context("package.name is missing")?
        .to_string();
    let version = package
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.1")
        .to_string();

    let meta = resolve_metadata(
        package
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
        get_string_array(package, "keywords"),
        get_string_array(package, "categories"),
        description,
        keywords,
        categories,
        force,
    );

    rebuild_package_fields(package, &original, ws, &name, &version, &meta, force);

    if ws.has_workspace_lints && doc.get("lints").is_none() {
        let mut lints = Table::new();
        lints.insert("workspace", toml_edit::value(true));
        doc.insert("lints", Item::Table(lints));
    }

    reorder_sections(&mut doc);

    let result = doc.to_string();
    std::fs::write(&cargo_toml_path, &result)
        .with_context(|| format!("failed to write {}", cargo_toml_path.display()))?;
    eprintln!("  fixed {}", cargo_toml_path.display());

    Ok(CrateMetadata {
        name,
        description: meta.description,
    })
}

fn workspace_true() -> Item {
    let mut t = InlineTable::new();
    t.set_dotted(true);
    t.insert("workspace", Value::from(true));
    Item::Value(Value::InlineTable(t))
}

fn is_workspace_true(item: &Item) -> bool {
    match item {
        // `edition = { workspace = true }` (inline table)
        Item::Value(Value::InlineTable(t)) => {
            t.get("workspace").and_then(Value::as_bool).unwrap_or(false)
        }
        // `edition.workspace = true` (dotted key — parsed as implicit Table)
        Item::Table(t) => t.get("workspace").and_then(Item::as_bool).unwrap_or(false),
        _ => false,
    }
}

fn get_string_array(table: &Table, key: &str) -> Option<Vec<String>> {
    table.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

fn reorder_sections(doc: &mut DocumentMut) {
    let all_keys: Vec<String> = doc.as_table().iter().map(|(k, _)| k.to_string()).collect();

    let mut ordered_items: Vec<(String, Item)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for &section in SECTION_ORDER {
        if let Some(item) = doc.remove(section) {
            ordered_items.push((section.to_string(), item));
            seen.insert(section.to_string());
        }
    }

    for key in &all_keys {
        if !seen.contains(key)
            && let Some(item) = doc.remove(key)
        {
            ordered_items.push((key.clone(), item));
        }
    }

    for (key, item) in ordered_items {
        doc.insert(&key, item);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn dotted_key_is_workspace_true() {
        let input = "[package]\nname = \"test\"\nedition.workspace = true\n";
        let doc: DocumentMut = input.parse().unwrap();
        let package = doc.get("package").unwrap().as_table().unwrap();
        let edition = package.get("edition").unwrap();
        assert!(is_workspace_true(edition));
    }

    #[test]
    fn inline_table_is_workspace_true() {
        let input = "[package]\nname = \"test\"\nedition = { workspace = true }\n";
        let doc: DocumentMut = input.parse().unwrap();
        let package = doc.get("package").unwrap().as_table().unwrap();
        let edition = package.get("edition").unwrap();
        assert!(is_workspace_true(edition));
    }

    #[test]
    fn string_value_is_not_workspace_true() {
        let input = "[package]\nname = \"test\"\nedition = \"2021\"\n";
        let doc: DocumentMut = input.parse().unwrap();
        let package = doc.get("package").unwrap().as_table().unwrap();
        let edition = package.get("edition").unwrap();
        assert!(!is_workspace_true(edition));
    }
}
