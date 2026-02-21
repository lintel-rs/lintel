use core::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use schema_catalog::{Catalog, SchemaEntry};
use tracing::debug;

use super::{OutputContext, Target, write_common_files};

/// A target optimized for GitHub Pages deployment.
pub struct GitHubPagesTarget {
    pub base_url: String,
    pub cname: Option<String>,
    pub dir: Option<String>,
}

impl Target for GitHubPagesTarget {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn output_dir(&self, target_name: &str, config_dir: &Path) -> PathBuf {
        if let Some(dir) = &self.dir {
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                path
            } else {
                config_dir.join(path)
            }
        } else {
            config_dir.join(".lintel-pages-output").join(target_name)
        }
    }

    async fn finalize(&self, ctx: &OutputContext<'_>) -> Result<()> {
        write_common_files(ctx).await?;
        write_github_pages_files(ctx, self.cname.as_deref()).await
    }
}

/// Write files specific to a GitHub Pages target.
///
/// - `.nojekyll` — tells GitHub Pages to skip Jekyll processing
/// - `CNAME` — custom domain file (if configured)
/// - `index.html` — landing page for the catalog
async fn write_github_pages_files(ctx: &OutputContext<'_>, cname: Option<&str>) -> Result<()> {
    // .nojekyll
    tokio::fs::write(ctx.output_dir.join(".nojekyll"), "")
        .await
        .context("failed to write .nojekyll")?;

    // CNAME
    if let Some(domain) = cname {
        tokio::fs::write(ctx.output_dir.join("CNAME"), format!("{domain}\n"))
            .await
            .context("failed to write CNAME")?;
        debug!(domain, "wrote CNAME");
    }

    // index.html
    let html = generate_index_html(ctx.catalog, ctx.groups_meta);
    tokio::fs::write(ctx.output_dir.join("index.html"), html)
        .await
        .context("failed to write index.html")?;
    debug!("wrote index.html");

    Ok(())
}

/// Generate a self-contained `index.html` landing page for the catalog.
#[allow(clippy::too_many_lines)]
fn generate_index_html(catalog: &Catalog, groups_meta: &[(String, String)]) -> String {
    let mut html = String::new();
    html.push_str(INDEX_HTML_HEAD);

    // Stats
    let group_count = catalog.groups.len();
    let schema_count = catalog.schemas.len();
    let _ = write!(
        html,
        r#"<div class="stats">
<div class="stat"><strong>{schema_count}</strong><span>schemas</span></div>
<div class="stat"><strong>{group_count}</strong><span>groups</span></div>
</div>
"#,
    );

    // Build group name -> schema names lookup
    let mut group_schema_names: alloc::collections::BTreeMap<&str, &[String]> =
        alloc::collections::BTreeMap::new();
    for g in &catalog.groups {
        group_schema_names.insert(&g.name, &g.schemas);
    }

    // Collect schemas assigned to groups
    let mut assigned_schemas: alloc::collections::BTreeSet<&str> =
        alloc::collections::BTreeSet::new();
    for g in &catalog.groups {
        for s in &g.schemas {
            assigned_schemas.insert(s);
        }
    }

    // Render groups
    for (group_name, group_desc) in groups_meta {
        let schema_names: Vec<&str> = group_schema_names
            .get(group_name.as_str())
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default();

        let group_schemas: Vec<&SchemaEntry> = catalog
            .schemas
            .iter()
            .filter(|s| schema_names.contains(&s.name.as_str()))
            .collect();

        if group_schemas.is_empty() {
            continue;
        }

        let _ = write!(
            html,
            r#"<details open data-group>
<summary>{}<span class="desc">— {}</span><span class="count">{} schemas</span></summary>
<div class="schema-list">
"#,
            html_escape(group_name),
            html_escape(group_desc),
            group_schemas.len(),
        );
        for schema in &group_schemas {
            write_schema_card(&mut html, schema);
        }
        html.push_str("</div>\n</details>\n");
    }

    // Remaining (unassigned) schemas
    let unassigned: Vec<&SchemaEntry> = catalog
        .schemas
        .iter()
        .filter(|s| !assigned_schemas.contains(s.name.as_str()))
        .collect();
    if !unassigned.is_empty() {
        let _ = write!(
            html,
            r#"<details data-group>
<summary>Other Schemas<span class="count">{} schemas</span></summary>
<div class="schema-list">
"#,
            unassigned.len(),
        );
        for schema in &unassigned {
            write_schema_card(&mut html, schema);
        }
        html.push_str("</div>\n</details>\n");
    }

    // Footer + script
    html.push_str(INDEX_HTML_FOOTER);
    html
}

fn write_schema_card(html: &mut String, schema: &SchemaEntry) {
    html.push_str(r#"<div class="schema-card">"#);
    let _ = write!(
        html,
        r#"<div class="schema-name"><a href="{}">{}</a></div>"#,
        html_escape(&schema.url),
        html_escape(&schema.name),
    );
    if !schema.description.is_empty() {
        let _ = write!(
            html,
            r#"<div class="schema-desc">{}</div>"#,
            html_escape(&schema.description),
        );
    }
    if !schema.file_match.is_empty() {
        html.push_str(r#"<div class="schema-patterns">"#);
        for (i, pat) in schema.file_match.iter().enumerate() {
            if i > 0 {
                html.push(' ');
            }
            let _ = write!(html, "<code>{}</code>", html_escape(pat));
        }
        html.push_str("</div>");
    }
    html.push_str("</div>\n");
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const INDEX_HTML_HEAD: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Schema Catalog</title>
<style>
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #1a1a2e; background: #f8f9fa; }
.container { max-width: 960px; margin: 0 auto; padding: 2rem 1.5rem; }
header { text-align: center; margin-bottom: 2rem; padding-bottom: 1.5rem; border-bottom: 2px solid #e9ecef; }
header h1 { font-size: 2rem; font-weight: 700; color: #16213e; }
header p { color: #495057; margin-top: 0.5rem; }
.search-box { margin-bottom: 2rem; }
.search-box input { width: 100%; padding: 0.75rem 1rem; font-size: 1rem; border: 1px solid #dee2e6; border-radius: 8px; outline: none; transition: border-color 0.2s; }
.search-box input:focus { border-color: #4361ee; box-shadow: 0 0 0 3px rgba(67, 97, 238, 0.15); }
.stats { display: flex; gap: 1.5rem; justify-content: center; margin-bottom: 2rem; flex-wrap: wrap; }
.stat { background: #fff; padding: 0.75rem 1.25rem; border-radius: 8px; border: 1px solid #e9ecef; text-align: center; }
.stat strong { display: block; font-size: 1.5rem; color: #4361ee; }
.stat span { font-size: 0.85rem; color: #6c757d; }
details { margin-bottom: 1rem; background: #fff; border-radius: 8px; border: 1px solid #e9ecef; }
summary { padding: 1rem 1.25rem; cursor: pointer; font-weight: 600; font-size: 1.1rem; color: #16213e; user-select: none; list-style: none; display: flex; align-items: center; gap: 0.5rem; }
summary::before { content: "\25B6"; font-size: 0.7rem; transition: transform 0.2s; }
details[open] > summary::before { transform: rotate(90deg); }
summary .count { font-weight: 400; font-size: 0.85rem; color: #6c757d; margin-left: auto; }
summary .desc { font-weight: 400; font-size: 0.9rem; color: #6c757d; margin-left: 0.5rem; }
.schema-list { padding: 0 1.25rem 1rem; }
.schema-card { padding: 0.75rem 0; border-top: 1px solid #f1f3f5; }
.schema-card:first-child { border-top: none; }
.schema-name { font-weight: 600; color: #16213e; }
.schema-name a { color: #4361ee; text-decoration: none; }
.schema-name a:hover { text-decoration: underline; }
.schema-desc { font-size: 0.9rem; color: #495057; margin-top: 0.25rem; }
.schema-patterns { font-size: 0.8rem; color: #868e96; margin-top: 0.25rem; }
.schema-patterns code { background: #f1f3f5; padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.8rem; }
footer { margin-top: 3rem; padding-top: 1.5rem; border-top: 1px solid #e9ecef; text-align: center; font-size: 0.85rem; color: #868e96; }
footer a { color: #4361ee; text-decoration: none; }
.hidden { display: none; }
</style>
</head>
<body>
<div class="container">
<header>
<h1>Schema Catalog</h1>
<p>JSON Schemas for editor auto-completion, validation, and documentation</p>
</header>
<div class="search-box">
<input type="text" id="search" placeholder="Search schemas by name or description..." autocomplete="off">
</div>
"#;

const INDEX_HTML_FOOTER: &str = r#"<footer>
<p>Generated by <a href="https://github.com/lintel-rs/lintel">lintel-catalog-builder</a></p>
</footer>
</div>
<script>
document.getElementById('search').addEventListener('input', function(e) {
  var q = e.target.value.toLowerCase();
  document.querySelectorAll('.schema-card').forEach(function(card) {
    var text = card.textContent.toLowerCase();
    card.classList.toggle('hidden', q.length > 0 && !text.includes(q));
  });
  document.querySelectorAll('[data-group]').forEach(function(group) {
    var visible = group.querySelectorAll('.schema-card:not(.hidden)').length;
    group.classList.toggle('hidden', q.length > 0 && visible === 0);
    if (q.length > 0 && visible > 0) group.open = true;
  });
});
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use schema_catalog::SchemaEntry;

    use crate::catalog::build_output_catalog;

    use super::*;

    #[tokio::test]
    async fn write_gh_pages_files_creates_nojekyll() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(None, vec![], vec![]);
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            catalog: &catalog,
            groups_meta: &[],
            source_count: 0,
        };
        write_github_pages_files(&ctx, Some("example.com")).await?;
        assert!(dir.path().join(".nojekyll").exists());
        let cname = tokio::fs::read_to_string(dir.path().join("CNAME")).await?;
        assert_eq!(cname.trim(), "example.com");
        assert!(dir.path().join("index.html").exists());
        Ok(())
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(
            html_escape("<b>\"hi\"&</b>"),
            "&lt;b&gt;&quot;hi&quot;&amp;&lt;/b&gt;"
        );
    }

    #[test]
    fn generate_index_html_contains_schema() {
        let catalog = build_output_catalog(
            None,
            vec![SchemaEntry {
                name: "Test Schema".into(),
                description: "A test".into(),
                url: "schemas/test.json".into(),
                file_match: vec!["*.test".into()],
                versions: BTreeMap::new(),
            }],
            vec![],
        );
        let html = generate_index_html(&catalog, &[]);
        assert!(html.contains("Test Schema"));
        assert!(html.contains("A test"));
        assert!(html.contains("schemas/test.json"));
    }
}
