use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use schema_catalog::{Catalog, CatalogGroup, SchemaEntry};
use serde::Serialize;

use super::schema_doc::SchemaDoc;
use crate::targets::OutputContext;

/// Shared site metadata passed to every template.
#[derive(Serialize, Clone)]
pub struct SiteInfo {
    pub title: String,
    pub description: String,
    /// Full absolute URL for SEO meta tags (canonical, og:url, sitemap).
    pub base_url: String,
    /// Root-relative path for internal links and assets (e.g. `/` or `/catalog/`).
    pub base_path: String,
    pub schema_count: usize,
    pub group_count: usize,
    /// Package version for the footer.
    pub version: String,
    /// Google Analytics measurement ID, if configured.
    pub ga_tracking_id: Option<String>,
}

/// Context for the home page template.
#[derive(Serialize)]
pub struct HomeContext {
    pub site: SiteInfo,
    pub groups: Vec<HomeGroup>,
    pub unassigned: Vec<SchemaCard>,
}

/// A group summary for the home page grid.
#[derive(Serialize)]
pub struct HomeGroup {
    pub key: String,
    pub name: String,
    pub description: String,
    pub schema_count: usize,
}

/// Context for a group detail page.
#[derive(Serialize)]
pub struct GroupPage {
    pub site: SiteInfo,
    pub key: String,
    pub name: String,
    pub description: String,
    pub seo_description: String,
    pub schemas: Vec<SchemaCard>,
    pub breadcrumbs: Vec<Breadcrumb>,
}

/// A schema card used in listing pages.
#[derive(Serialize)]
pub struct SchemaCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub file_match: Vec<String>,
}

/// Context for a schema detail page.
#[derive(Serialize)]
pub struct SchemaPage {
    pub site: SiteInfo,
    pub name: String,
    pub description: String,
    pub description_html: String,
    pub seo_description: String,
    pub page_url: String,
    pub json_url: String,
    pub source_url: Option<String>,
    pub file_match: Vec<String>,
    pub versions: Vec<VersionLink>,
    pub group_name: Option<String>,
    pub group_key: Option<String>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub schema_doc: Option<SchemaDoc>,
}

/// A version link on schema detail pages.
#[derive(Serialize)]
pub struct VersionLink {
    pub name: String,
    pub url: String,
}

/// Context for a version detail page.
#[derive(Serialize)]
pub struct VersionPage {
    pub site: SiteInfo,
    pub version_name: String,
    pub schema_name: String,
    pub page_url: String,
    pub json_url: String,
    pub schema_page_url: String,
    pub group_name: Option<String>,
    pub group_key: Option<String>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub schema_doc: Option<SchemaDoc>,
}

/// Context for a `_shared` dependency schema page.
#[derive(Serialize)]
pub struct SharedSchemaPage {
    pub site: SiteInfo,
    pub name: String,
    pub json_url: String,
    pub parent_schema_name: Option<String>,
    pub parent_schema_url: Option<String>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub schema_doc: Option<SchemaDoc>,
}

/// Breadcrumb navigation entry.
#[derive(Serialize)]
pub struct Breadcrumb {
    pub label: String,
    pub url: Option<String>,
}

/// Context for the sitemap template.
#[derive(Serialize)]
pub struct SitemapContext {
    pub base_url: String,
    pub urls: Vec<String>,
}

/// Search index entry with compact keys.
#[derive(Serialize)]
pub struct SearchEntry {
    pub n: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub d: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub f: String,
    pub u: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub g: String,
}

/// Build [`SiteInfo`] from the output context.
pub fn build_site_info(ctx: &OutputContext<'_>) -> SiteInfo {
    let title = ctx
        .catalog
        .title
        .clone()
        .unwrap_or_else(|| String::from("Schema Catalog"));
    let base_url = ensure_trailing_slash(ctx.base_url);
    let base_path = extract_path(&base_url);
    let schema_count = ctx.processed.len();
    let description = if let Some(desc) = ctx.site_description {
        String::from(desc)
    } else {
        alloc::format!(
            "A catalog of {} JSON Schemas for editor auto-completion, validation, and documentation",
            format_number(schema_count),
        )
    };
    SiteInfo {
        title,
        description,
        base_url,
        base_path,
        schema_count,
        group_count: ctx.catalog.groups.len(),
        version: String::from(env!("CARGO_PKG_VERSION")),
        ga_tracking_id: ctx.ga_tracking_id.map(String::from),
    }
}

/// Extract the path portion from a URL (e.g. `https://example.com/catalog/` → `/catalog/`).
/// Falls back to `/` if parsing fails.
fn extract_path(url: &str) -> String {
    url::Url::parse(url).map_or_else(|_| String::from("/"), |u| ensure_trailing_slash(u.path()))
}

/// Build the home page context.
pub fn build_home_context(ctx: &OutputContext<'_>, site: &SiteInfo) -> HomeContext {
    let assigned = assigned_schema_names(&ctx.catalog.groups);
    let base_url = &site.base_url;

    let groups: Vec<HomeGroup> = ctx
        .groups_meta
        .iter()
        .map(|(key, name, desc)| HomeGroup {
            key: key.clone(),
            name: name.clone(),
            description: desc.clone(),
            schema_count: group_schema_count(&ctx.catalog.groups, name),
        })
        .collect();

    let unassigned: Vec<SchemaCard> = ctx
        .catalog
        .schemas
        .iter()
        .filter(|s| !assigned.contains(s.name.as_str()))
        .filter_map(|s| schema_card(s, base_url))
        .collect();

    HomeContext {
        site: site.clone(),
        groups,
        unassigned,
    }
}

/// Build a group page context.
///
/// `meta` is `(key, name, description)` from the groups metadata.
pub fn build_group_page(
    site: &SiteInfo,
    meta: &(String, String, String),
    group: &CatalogGroup,
    catalog: &Catalog,
) -> GroupPage {
    let (key, name, description) = (meta.0.as_str(), meta.1.as_str(), meta.2.as_str());
    let schemas: Vec<SchemaCard> = catalog
        .schemas
        .iter()
        .filter(|s| group.schemas.contains(&s.name))
        .filter_map(|s| schema_card(s, &site.base_url))
        .collect();

    let count = schemas.len();
    let schema_word = if count == 1 { "schema" } else { "schemas" };
    let seo_description = alloc::format!(
        "{} {schema_word} for {}. {} Lintel is a catalog of {} JSON Schemas for project configuration.",
        format_number(count),
        name,
        ensure_sentence_end(description),
        format_number(site.schema_count),
    );

    GroupPage {
        site: site.clone(),
        key: String::from(key),
        name: String::from(name),
        description: String::from(description),
        seo_description,
        breadcrumbs: vec![
            Breadcrumb {
                label: String::from("Home"),
                url: Some(site.base_path.clone()),
            },
            Breadcrumb {
                label: String::from(name),
                url: None,
            },
        ],
        schemas,
    }
}

/// Build a schema detail page context.
///
/// `group_info` is `Some((key, name))` if the schema belongs to a group.
#[allow(clippy::too_many_arguments)]
pub fn build_schema_page(
    site: &SiteInfo,
    entry: &SchemaEntry,
    page_url: &str,
    group_info: Option<(&str, &str)>,
    schema_doc: Option<SchemaDoc>,
) -> SchemaPage {
    let group_key = group_info.map(|(k, _)| k);
    let group_name = group_info.map(|(_, n)| n);
    let versions: Vec<VersionLink> = entry
        .versions
        .iter()
        .filter_map(|(vname, vurl)| {
            version_page_url(vurl, &site.base_url).map(|url| VersionLink {
                name: vname.clone(),
                url,
            })
        })
        .collect();

    let mut breadcrumbs = vec![Breadcrumb {
        label: String::from("Home"),
        url: Some(site.base_path.clone()),
    }];
    if let (Some(gn), Some(gk)) = (group_name, group_key) {
        breadcrumbs.push(Breadcrumb {
            label: String::from(gn),
            url: Some(alloc::format!("{}schemas/{}/", site.base_path, gk)),
        });
    }
    breadcrumbs.push(Breadcrumb {
        label: entry.name.clone(),
        url: None,
    });

    let description_html = md_to_html(&entry.description);
    let seo_description = alloc::format!(
        "Complete reference for {}. {} Lintel is a catalog of {} JSON Schemas for project configuration.",
        entry.name,
        ensure_sentence_end(&entry.description),
        format_number(site.schema_count),
    );

    SchemaPage {
        site: site.clone(),
        name: entry.name.clone(),
        description: entry.description.clone(),
        description_html,
        seo_description,
        page_url: String::from(page_url),
        json_url: entry.url.clone(),
        source_url: entry.source_url.clone(),
        file_match: entry.file_match.clone(),
        versions,
        group_name: group_name.map(String::from),
        group_key: group_key.map(String::from),
        breadcrumbs,
        schema_doc,
    }
}

/// Convert markdown text to HTML using pulldown-cmark.
///
/// External links are annotated with `target="_blank" rel="noopener noreferrer"`.
fn md_to_html(text: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    // Add target="_blank" to external links
    html_output = html_output
        .replace(
            "<a href=\"https://",
            "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://",
        )
        .replace(
            "<a href=\"http://",
            "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"http://",
        );
    html_output
}

/// Build a version detail page context.
#[allow(clippy::too_many_arguments)]
pub fn build_version_page(
    site: &SiteInfo,
    schema_name: &str,
    version_name: &str,
    page_url: &str,
    json_url: &str,
    schema_page_url: &str,
    group_name: Option<&str>,
    group_key: Option<&str>,
    schema_doc: Option<SchemaDoc>,
) -> VersionPage {
    let mut breadcrumbs = vec![Breadcrumb {
        label: String::from("Home"),
        url: Some(site.base_path.clone()),
    }];
    if let (Some(gn), Some(gk)) = (group_name, group_key) {
        breadcrumbs.push(Breadcrumb {
            label: String::from(gn),
            url: Some(alloc::format!("{}schemas/{}/", site.base_path, gk)),
        });
    }
    breadcrumbs.push(Breadcrumb {
        label: String::from(schema_name),
        url: Some(alloc::format!("{}{}", site.base_path, schema_page_url)),
    });
    breadcrumbs.push(Breadcrumb {
        label: String::from(version_name),
        url: None,
    });

    VersionPage {
        site: site.clone(),
        version_name: String::from(version_name),
        schema_name: String::from(schema_name),
        page_url: String::from(page_url),
        json_url: String::from(json_url),
        schema_page_url: alloc::format!("{}{}", site.base_path, schema_page_url),
        group_name: group_name.map(String::from),
        group_key: group_key.map(String::from),
        breadcrumbs,
        schema_doc,
    }
}

/// Strip `base_url` prefix and `/latest.json` suffix to get the schema page path.
///
/// Returns `None` if the URL doesn't start with `base_url` (i.e. remote schema).
pub fn schema_page_url(schema_url: &str, base_url: &str) -> Option<String> {
    let relative = schema_url.strip_prefix(base_url)?;
    let path = relative.strip_suffix("latest.json").unwrap_or(relative);
    Some(ensure_trailing_slash(path))
}

/// Strip `base_url` prefix and `.json` suffix, add trailing slash for version page URL.
pub fn version_page_url(version_url: &str, base_url: &str) -> Option<String> {
    let relative = version_url.strip_prefix(base_url)?;
    let path = relative.strip_suffix(".json").unwrap_or(relative);
    Some(ensure_trailing_slash(path))
}

fn schema_card(entry: &SchemaEntry, base_url: &str) -> Option<SchemaCard> {
    let url = schema_page_url(&entry.url, base_url)?;
    Some(SchemaCard {
        name: entry.name.clone(),
        description: entry.description.clone(),
        url,
        file_match: entry.file_match.clone(),
    })
}

fn ensure_trailing_slash(s: &str) -> String {
    if s.ends_with('/') {
        String::from(s)
    } else {
        alloc::format!("{s}/")
    }
}

fn assigned_schema_names(groups: &[CatalogGroup]) -> alloc::collections::BTreeSet<&str> {
    let mut set = alloc::collections::BTreeSet::new();
    for g in groups {
        for s in &g.schemas {
            set.insert(s.as_str());
        }
    }
    set
}

fn group_schema_count(groups: &[CatalogGroup], group_name: &str) -> usize {
    groups
        .iter()
        .find(|g| g.name == group_name)
        .map_or(0, |g| g.schemas.len())
}

/// Ensure a string ends with a sentence-ending punctuation mark.
fn ensure_sentence_end(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?') {
        String::from(trimmed)
    } else {
        alloc::format!("{trimmed}.")
    }
}

/// Format a number with comma separators (e.g. `1234` → `"1,234"`).
fn format_number(n: usize) -> String {
    let s = alloc::format!("{n}");
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Build a mapping from schema name to `(group_key, group_name)`.
pub fn schema_group_map<'a>(
    catalog: &'a Catalog,
    groups_meta: &'a [(String, String, String)],
) -> BTreeMap<&'a str, (&'a str, &'a str)> {
    let mut map = BTreeMap::new();
    for group in &catalog.groups {
        let meta = groups_meta.iter().find(|(_, name, _)| *name == group.name);
        if let Some((key, name, _)) = meta {
            for schema_name in &group.schemas {
                map.insert(schema_name.as_str(), (key.as_str(), name.as_str()));
            }
        }
    }
    map
}
