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

#[derive(Debug, Error, Diagnostic)]
#[error("`publish = false` should not be set")]
#[diagnostic(
    code(furnish::publish_false),
    severity(Warning),
    help("autofixable with cargo furnish check --fix")
)]
pub struct PublishFalse {
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

    let pkg_span = find_line_span(&content, "[package]");

    let ctx = CargoTomlCheck {
        content: &content,
        doc: &doc,
        package,
        ws,
        crate_dir,
        src: &src,
        pkg_span,
    };

    check_package_fields(&ctx, &name, description.as_ref(), &mut diagnostics);

    check_lints_section(&ctx, &mut diagnostics);

    Ok((CrateMetadata { name, description }, diagnostics))
}

/// Context for Cargo.toml checking operations.
struct CargoTomlCheck<'a> {
    content: &'a str,
    doc: &'a DocumentMut,
    package: &'a Table,
    ws: &'a WorkspaceInfo,
    crate_dir: &'a Path,
    src: &'a dyn Fn(&str) -> NamedSource<String>,
    pkg_span: (usize, usize),
}

fn check_package_fields(
    ctx: &CargoTomlCheck<'_>,
    name: &str,
    description: Option<&String>,
    diagnostics: &mut Vec<Box<dyn Diagnostic + Send + Sync>>,
) {
    if ctx.package.get("readme").is_some() {
        let (offset, len) = find_line_span(ctx.content, "readme");
        diagnostics.push(Box::new(UnnecessaryReadme {
            src: (ctx.src)(ctx.content),
            span: (offset, len).into(),
        }));
    }

    if ctx.package.get("publish").and_then(Item::as_bool) == Some(false) {
        let (offset, len) = find_line_span(ctx.content, "publish =");
        diagnostics.push(Box::new(PublishFalse {
            src: (ctx.src)(ctx.content),
            span: (offset, len).into(),
        }));
    }

    if description.is_none() {
        let (offset, len) = find_insertion_span(ctx.content, ctx.package, "description");
        diagnostics.push(Box::new(MissingDescription {
            src: (ctx.src)(ctx.content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }
    if get_string_array(ctx.package, "keywords").is_none() {
        let (offset, len) = find_insertion_span(ctx.content, ctx.package, "keywords");
        diagnostics.push(Box::new(MissingKeywords {
            src: (ctx.src)(ctx.content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }
    if get_string_array(ctx.package, "categories").is_none() {
        let (offset, len) = find_insertion_span(ctx.content, ctx.package, "categories");
        diagnostics.push(Box::new(MissingCategories {
            src: (ctx.src)(ctx.content),
            span: (offset, len).into(),
            crate_name: name.to_string(),
        }));
    }

    // A crate-local LICENSE file allows overriding the workspace license.
    let has_local_license = ctx.crate_dir.join("LICENSE").exists();

    for &field in WORKSPACE_INHERITABLE {
        // Skip the license check if the crate ships its own LICENSE file.
        if field == "license" && has_local_license {
            continue;
        }
        if ctx.ws.package_fields.contains(field)
            && let Some(item) = ctx.package.get(field)
            && !is_workspace_true(item)
        {
            let (offset, len) = find_line_span(ctx.content, &format!("{field} ="));
            diagnostics.push(Box::new(NotWorkspaceInherited {
                src: (ctx.src)(ctx.content),
                span: (offset, len).into(),
                field: field.to_string(),
            }));
        }
    }
}

fn check_lints_section(
    ctx: &CargoTomlCheck<'_>,
    diagnostics: &mut Vec<Box<dyn Diagnostic + Send + Sync>>,
) {
    if ctx.ws.has_workspace_lints && ctx.doc.get("lints").is_none() {
        diagnostics.push(Box::new(MissingLints {
            src: (ctx.src)(ctx.content),
            span: ctx.pkg_span.into(),
        }));
    }
}

/// User-supplied metadata updates (from CLI arguments).
pub struct MetadataUpdate<'a> {
    pub description: Option<&'a str>,
    pub keywords: Option<&'a [String]>,
    pub categories: Option<&'a [String]>,
    pub force: bool,
}

/// Metadata already present in Cargo.toml.
struct ExistingMetadata {
    description: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

/// Resolve final metadata values, preferring existing or forced values.
struct ResolvedMetadata {
    description: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

fn resolve_metadata(existing: ExistingMetadata, update: &MetadataUpdate<'_>) -> ResolvedMetadata {
    let description = if update.force {
        update
            .description
            .map(String::from)
            .or(existing.description)
    } else {
        existing
            .description
            .or_else(|| update.description.map(String::from))
    };

    let keywords = if update.force && update.keywords.is_some() {
        update.keywords.map(<[String]>::to_vec)
    } else if existing.keywords.is_some() {
        existing.keywords
    } else {
        update.keywords.map(<[String]>::to_vec)
    };

    let categories = if update.force && update.categories.is_some() {
        update.categories.map(<[String]>::to_vec)
    } else if existing.categories.is_some() {
        existing.categories
    } else {
        update.categories.map(<[String]>::to_vec)
    };

    ResolvedMetadata {
        description,
        keywords,
        categories,
    }
}

/// Context for rebuilding package fields during fix.
struct RebuildContext<'a> {
    original: &'a DocumentMut,
    ws: &'a WorkspaceInfo,
    crate_dir: &'a Path,
    force: bool,
}

fn update_package_fields(package: &mut Table, ctx: &RebuildContext<'_>, meta: &ResolvedMetadata) {
    if let Some(ref desc) = meta.description {
        package.insert("description", toml_edit::value(desc.as_str()));
    }
    if let Some(ref kws) = meta.keywords {
        let arr: toml_edit::Array = kws.iter().map(|s| Value::from(s.as_str())).collect();
        package.insert("keywords", Item::Value(Value::Array(arr)));
    }
    if let Some(ref cats) = meta.categories {
        let arr: toml_edit::Array = cats.iter().map(|s| Value::from(s.as_str())).collect();
        package.insert("categories", Item::Value(Value::Array(arr)));
    }
    for &field in WORKSPACE_INHERITABLE {
        insert_workspace_field(package, ctx, field);
    }
}

fn insert_workspace_field(package: &mut Table, ctx: &RebuildContext<'_>, field: &str) {
    if !ctx.ws.package_fields.contains(field) {
        return;
    }
    // A crate-local LICENSE file allows overriding the workspace license;
    // preserve whatever value the crate already has.
    if field == "license" && ctx.crate_dir.join("LICENSE").exists() {
        if let Some(orig) = ctx
            .original
            .get("package")
            .and_then(|p| p.as_table())
            .and_then(|t| t.get(field))
        {
            package.insert(field, orig.clone());
        }
        return;
    }
    if ctx.force {
        package.insert(field, workspace_true());
        return;
    }
    let orig_item = ctx
        .original
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
    update: &MetadataUpdate<'_>,
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
    package.remove("publish");

    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .context("package.name is missing")?
        .to_string();

    let existing = ExistingMetadata {
        description: package
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
        keywords: get_string_array(package, "keywords"),
        categories: get_string_array(package, "categories"),
    };
    let meta = resolve_metadata(existing, update);

    let rebuild_ctx = RebuildContext {
        original: &original,
        ws,
        crate_dir,
        force: update.force,
    };
    update_package_fields(package, &rebuild_ctx, &meta);

    if ws.has_workspace_lints && doc.get("lints").is_none() {
        let mut lints = Table::new();
        lints.insert("workspace", toml_edit::value(true));
        doc.insert("lints", Item::Table(lints));
    }

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
