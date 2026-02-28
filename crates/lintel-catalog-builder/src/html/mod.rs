mod assets;
mod context;
mod engine;
mod schema_doc;
mod search;

use std::path::Path;

use anyhow::{Context, Result};
use tracing::debug;

use crate::download::ProcessedSchemas;
use crate::targets::OutputContext;
use context::{
    SharedSchemaPage, SiteInfo, build_group_page, build_home_context, build_schema_page,
    build_schemas_index, build_site_info, build_version_page, schema_group_map, schema_page_url,
    version_page_url,
};

/// Generate all HTML pages, assets, search index, and sitemap.
pub async fn generate_site(ctx: &OutputContext<'_>) -> Result<()> {
    let env = engine::create_engine()?;
    let site = build_site_info(ctx);
    let mut sitemap_urls: Vec<String> = Vec::new();

    render_home_page(&env, &site, ctx)?;
    render_schemas_index(&env, &site, ctx, &mut sitemap_urls)?;
    render_group_pages(&env, &site, ctx, &mut sitemap_urls)?;
    render_schema_pages(&env, &site, ctx, &mut sitemap_urls)?;
    render_shared_pages(&env, &site, ctx, &mut sitemap_urls)?;
    render_sitemap(&env, &site, ctx.output_dir, &sitemap_urls)?;
    write_robots_txt(ctx.output_dir, &site.base_url)?;

    assets::write_assets(ctx.output_dir).await?;
    search::write_search_index(ctx.output_dir, ctx.catalog, ctx.base_url, ctx.groups_meta).await?;

    debug!("site generation complete");
    Ok(())
}

/// Render and write the home page.
fn render_home_page(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
) -> Result<()> {
    let home_ctx = build_home_context(ctx, site);
    let html = engine::render(env, "index.html", &home_ctx)?;
    let path = ctx.output_dir.join("index.html");
    std::fs::write(&path, html)
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    debug!(path = %path.display(), "wrote index.html");
    Ok(())
}

/// Render and write the `/schemas/` index page.
fn render_schemas_index(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
    sitemap_urls: &mut Vec<String>,
) -> Result<()> {
    let page = build_schemas_index(ctx, site);
    let html = engine::render(env, "schemas_index.html", &page)?;
    let dir = ctx.output_dir.join("schemas");
    std::fs::create_dir_all(&dir)
        .with_context(|| alloc::format!("failed to create {}", dir.display()))?;
    let path = dir.join("index.html");
    std::fs::write(&path, html)
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    debug!(path = %path.display(), "wrote schemas index");
    sitemap_urls.push(String::from("schemas/"));
    Ok(())
}

/// Render and write all group pages.
fn render_group_pages(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
    sitemap_urls: &mut Vec<String>,
) -> Result<()> {
    for meta in ctx.groups_meta {
        let group = ctx.catalog.groups.iter().find(|g| g.name == meta.1);
        let Some(group) = group else { continue };
        if group.schemas.is_empty() {
            continue;
        }

        let page = build_group_page(site, meta, group, ctx.catalog);
        let html = engine::render(env, "group.html", &page)?;

        let dir = ctx.output_dir.join("schemas").join(&meta.0);
        std::fs::create_dir_all(&dir)
            .with_context(|| alloc::format!("failed to create {}", dir.display()))?;
        let path = dir.join("index.html");
        std::fs::write(&path, html)
            .with_context(|| alloc::format!("failed to write {}", path.display()))?;
        debug!(path = %path.display(), "wrote group page");

        sitemap_urls.push(alloc::format!("schemas/{}/", meta.0));
    }
    Ok(())
}

/// Render and write all schema and version pages.
fn render_schema_pages(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
    sitemap_urls: &mut Vec<String>,
) -> Result<()> {
    let group_map = schema_group_map(ctx.catalog, ctx.groups_meta);

    for entry in &ctx.catalog.schemas {
        let Some(page_url) = schema_page_url(&entry.url, &site.base_url) else {
            continue;
        };

        let group_info = group_map.get(entry.name.as_str()).copied();

        let doc = load_schema_doc(&page_url, ctx.processed);
        let page = build_schema_page(site, entry, &page_url, group_info, doc);
        let html = engine::render(env, "schema.html", &page)?;
        write_page(ctx.output_dir, &page_url, &html)?;
        sitemap_urls.push(page_url.clone());

        let (group_key, group_name) = group_info.map_or((None, None), |(k, n)| (Some(k), Some(n)));
        render_version_pages(
            env,
            site,
            ctx,
            entry,
            &page_url,
            group_name,
            group_key,
            sitemap_urls,
        )?;
    }
    Ok(())
}

/// Try to load and extract schema documentation from in-memory processed schemas.
fn load_schema_doc(page_url: &str, processed: &ProcessedSchemas) -> Option<schema_doc::SchemaDoc> {
    let relative_key = format!("{page_url}latest.json");
    let value = processed.get(&relative_key)?;
    let doc = schema_doc::extract_schema_doc(&value);
    if doc.has_content { Some(doc) } else { None }
}

/// Load schema doc for a specific relative path (e.g. version or shared schema).
fn load_schema_doc_by_path(
    relative_path: &str,
    processed: &ProcessedSchemas,
) -> Option<schema_doc::SchemaDoc> {
    let value = processed.get(relative_path)?;
    let doc = schema_doc::extract_schema_doc(&value);
    if doc.has_content { Some(doc) } else { None }
}

/// Render version detail pages for a single schema.
#[allow(clippy::too_many_arguments)]
fn render_version_pages(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
    entry: &schema_catalog::SchemaEntry,
    schema_page_url: &str,
    group_name: Option<&str>,
    group_key: Option<&str>,
    sitemap_urls: &mut Vec<String>,
) -> Result<()> {
    for (vname, vurl) in &entry.versions {
        let Some(vpage_url) = version_page_url(vurl, &site.base_url) else {
            continue;
        };
        // Load schema doc for this version from ProcessedSchemas
        // The version JSON is at e.g. schemas/github/workflow/versions/v2.json
        let relative = vurl.strip_prefix(&site.base_url).unwrap_or(vurl);
        let version_doc = load_schema_doc_by_path(relative, ctx.processed);

        let page = build_version_page(
            site,
            &entry.name,
            vname,
            &vpage_url,
            vurl,
            schema_page_url,
            group_name,
            group_key,
            version_doc,
        );
        let html = engine::render(env, "version.html", &page)?;
        write_page(ctx.output_dir, &vpage_url, &html)?;
        sitemap_urls.push(vpage_url);
    }
    Ok(())
}

/// Render HTML pages for `_shared` dependency schemas.
fn render_shared_pages(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    ctx: &OutputContext<'_>,
    sitemap_urls: &mut Vec<String>,
) -> Result<()> {
    let keys = ctx.processed.keys();
    for key in &keys {
        if !key.contains("/_shared/") {
            continue;
        }
        // key is e.g. "schemas/github/workflow/_shared/workflow--rule.json"
        let Some(value) = ctx.processed.get(key) else {
            continue;
        };

        // Extract the schema name from the filename
        let filename = key
            .rsplit('/')
            .next()
            .unwrap_or(key)
            .trim_end_matches(".json");
        let json_url = format!("{}{}", site.base_url, key);

        // Derive the page URL (strip .json, add trailing slash)
        let page_path = key.trim_end_matches(".json");
        let page_url = format!("{page_path}/");

        // Find the parent schema (the directory above _shared)
        let parent_parts: Vec<&str> = key.split("/_shared/").collect();
        let (parent_schema_name, parent_schema_url) = if parent_parts.len() == 2 {
            let parent_path = parent_parts[0];
            // parent_path is e.g. "schemas/github/workflow"
            let parent_name = parent_path.rsplit('/').next().unwrap_or(parent_path);
            (
                Some(String::from(parent_name)),
                Some(format!("{}{}/", site.base_path, parent_path)),
            )
        } else {
            (None, None)
        };

        // Build breadcrumbs
        let mut breadcrumbs = vec![context::Breadcrumb {
            label: String::from("Home"),
            url: Some(site.base_path.clone()),
        }];
        if let Some(ref parent_name) = parent_schema_name
            && let Some(ref parent_url) = parent_schema_url
        {
            breadcrumbs.push(context::Breadcrumb {
                label: parent_name.clone(),
                url: Some(parent_url.clone()),
            });
        }
        breadcrumbs.push(context::Breadcrumb {
            label: String::from(filename),
            url: None,
        });

        let schema_doc = {
            let doc = schema_doc::extract_schema_doc(&value);
            if doc.has_content { Some(doc) } else { None }
        };

        let page = SharedSchemaPage {
            site: site.clone(),
            name: String::from(filename),
            json_url,
            parent_schema_name,
            parent_schema_url,
            breadcrumbs,
            schema_doc,
        };

        let html = engine::render(env, "shared.html", &page)?;
        write_page(ctx.output_dir, &page_url, &html)?;
        sitemap_urls.push(page_url);
    }
    Ok(())
}

/// Render and write the sitemap.
fn render_sitemap(
    env: &minijinja::Environment<'_>,
    site: &SiteInfo,
    output_dir: &Path,
    urls: &[String],
) -> Result<()> {
    let lastmod = today_iso8601();
    let sitemap_ctx = context::SitemapContext {
        base_url: site.base_url.clone(),
        urls: urls.to_vec(),
        lastmod,
    };
    let xml = engine::render(env, "sitemap.xml", &sitemap_ctx)?;
    let path = output_dir.join("sitemap.xml");
    std::fs::write(&path, xml)
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    debug!(path = %path.display(), "wrote sitemap.xml");
    Ok(())
}

/// Return today's date as an ISO 8601 string (e.g. `2025-01-15`).
fn today_iso8601() -> String {
    // Compute date from UNIX epoch without pulling in a datetime crate.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Simple Gregorian calendar calculation
    let (year, month, day) = days_to_ymd(days);
    alloc::format!("{year:04}-{month:02}-{day:02}")
}

/// Convert a count of days since 1970-01-01 to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let year_days = if is_leap(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Write `robots.txt` to the output directory.
fn write_robots_txt(output_dir: &Path, base_url: &str) -> Result<()> {
    let content = alloc::format!("User-agent: *\nAllow: /\n\nSitemap: {base_url}sitemap.xml\n");
    let path = output_dir.join("robots.txt");
    std::fs::write(&path, content)
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    debug!(path = %path.display(), "wrote robots.txt");
    Ok(())
}

/// Write an HTML page to `{output_dir}/{page_url}/index.html`.
fn write_page(output_dir: &Path, page_url: &str, html: &str) -> Result<()> {
    let dir = output_dir.join(page_url.trim_end_matches('/'));
    std::fs::create_dir_all(&dir)
        .with_context(|| alloc::format!("failed to create {}", dir.display()))?;
    let path = dir.join("index.html");
    std::fs::write(&path, html)
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    debug!(path = %path.display(), "wrote page");
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use std::path::Path;

    use schema_catalog::SchemaEntry;

    use crate::catalog::build_output_catalog;
    use crate::download::ProcessedSchemas;

    use super::*;

    fn test_ctx(dir: &Path) -> (schema_catalog::Catalog, Vec<(String, String, String)>) {
        let catalog = build_output_catalog(
            Some(String::from("Test Catalog")),
            vec![
                SchemaEntry {
                    name: String::from("Workflow"),
                    description: String::from("GitHub Actions workflow"),
                    url: String::from("https://example.com/schemas/github/workflow/latest.json"),
                    source_url: Some(String::from("https://upstream.example.com/workflow.json")),
                    file_match: vec![String::from(".github/workflows/*.yml")],
                    versions: {
                        let mut m = BTreeMap::new();
                        m.insert(
                            String::from("v2"),
                            String::from(
                                "https://example.com/schemas/github/workflow/versions/v2.json",
                            ),
                        );
                        m
                    },
                },
                SchemaEntry {
                    name: String::from("Unassigned Schema"),
                    description: String::from("Not in any group"),
                    url: String::from("https://example.com/schemas/misc/unassigned/latest.json"),
                    source_url: None,
                    file_match: vec![],
                    versions: BTreeMap::new(),
                },
            ],
            vec![schema_catalog::CatalogGroup {
                name: String::from("GitHub"),
                description: String::from("GitHub configuration schemas"),
                schemas: vec![String::from("Workflow")],
            }],
        );
        let groups_meta = vec![(
            String::from("github"),
            String::from("GitHub"),
            String::from("GitHub configuration schemas"),
        )];
        let _ = dir;
        (catalog, groups_meta)
    }

    #[tokio::test]
    async fn generate_site_creates_pages() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let (catalog, groups_meta) = test_ctx(dir.path());
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            config_dir: dir.path(),
            catalog: &catalog,
            groups_meta: &groups_meta,
            base_url: "https://example.com/",
            source_count: 0,
            processed: &processed,
            site_description: None,
            ga_tracking_id: None,
            og_image: None,
        };

        generate_site(&ctx).await?;

        // index.html
        assert!(dir.path().join("index.html").exists());
        let index = std::fs::read_to_string(dir.path().join("index.html"))?;
        assert!(index.contains("Test Catalog"));
        assert!(index.contains("GitHub"));

        // Group page
        assert!(dir.path().join("schemas/github/index.html").exists());
        let group = std::fs::read_to_string(dir.path().join("schemas/github/index.html"))?;
        assert!(group.contains("Workflow"));

        // Schema page
        let schema_path = dir.path().join("schemas/github/workflow/index.html");
        assert!(schema_path.exists());
        let schema = std::fs::read_to_string(&schema_path)?;
        assert!(schema.contains("Workflow"));
        assert!(schema.contains(".github/workflows/*.yml"));

        // Version page
        let version_path = dir
            .path()
            .join("schemas/github/workflow/versions/v2/index.html");
        assert!(version_path.exists());

        // Schemas index page
        let schemas_index_path = dir.path().join("schemas/index.html");
        assert!(schemas_index_path.exists());
        let schemas_index = std::fs::read_to_string(&schemas_index_path)?;
        assert!(schemas_index.contains("All Schemas"));
        assert!(schemas_index.contains("Workflow"));

        // Assets
        assert!(dir.path().join("style.css").exists());
        assert!(dir.path().join("app.js").exists());
        assert!(dir.path().join("search-index.json").exists());
        assert!(dir.path().join("sitemap.xml").exists());
        assert!(dir.path().join("robots.txt").exists());

        // Sitemap contains lastmod
        let sitemap = std::fs::read_to_string(dir.path().join("sitemap.xml"))?;
        assert!(sitemap.contains("<lastmod>"));
        assert!(sitemap.contains("schemas/"));

        // robots.txt points to sitemap
        let robots = std::fs::read_to_string(dir.path().join("robots.txt"))?;
        assert!(robots.contains("Sitemap: https://example.com/sitemap.xml"));

        Ok(())
    }

    #[test]
    fn schema_page_url_strips_base_and_latest() {
        let url = "https://example.com/schemas/github/workflow/latest.json";
        let base = "https://example.com/";
        assert_eq!(
            schema_page_url(url, base),
            Some(String::from("schemas/github/workflow/"))
        );
    }

    #[test]
    fn schema_page_url_returns_none_for_remote() {
        let url = "https://other.example.com/schema.json";
        let base = "https://example.com/";
        assert_eq!(schema_page_url(url, base), None);
    }

    #[test]
    fn version_page_url_strips_json() {
        let url = "https://example.com/schemas/github/workflow/versions/v2.json";
        let base = "https://example.com/";
        assert_eq!(
            version_page_url(url, base),
            Some(String::from("schemas/github/workflow/versions/v2/"))
        );
    }
}
