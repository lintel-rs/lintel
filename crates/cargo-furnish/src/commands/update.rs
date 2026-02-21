use std::path::PathBuf;

use crate::{cargo_toml, doc_injection, readme, workspace};

#[allow(clippy::needless_pass_by_value)] // CLI args are owned
pub fn run(
    crate_dirs: &[PathBuf],
    ws: &workspace::WorkspaceInfo,
    description: Option<String>,
    readme: Option<String>,
    keywords: Option<String>,
    categories: Option<String>,
    force: bool,
) {
    // Unescape \n in text arguments
    let description = description.as_deref().map(unescape_newlines);
    let readme_body = readme.as_deref().map(unescape_newlines);

    let keywords: Option<Vec<String>> = keywords
        .as_deref()
        .map(|s| s.split(',').map(|k| k.trim().to_string()).collect());
    let categories: Option<Vec<String>> = categories
        .as_deref()
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

    let repo = ws
        .repository
        .as_deref()
        .unwrap_or("https://github.com/lintel-rs/lintel");
    let license_text = ws.license.as_deref().unwrap_or("Apache-2.0");

    let mut total_errors: usize = 0;

    for crate_dir in crate_dirs {
        let meta = match cargo_toml::fix_cargo_toml(
            crate_dir,
            ws,
            description.as_deref(),
            keywords.as_deref(),
            categories.as_deref(),
            force,
        ) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  error: {e:#}");
                total_errors += 1;
                continue;
            }
        };

        if let Err(e) = readme::fix_readme(
            crate_dir,
            &meta.name,
            meta.description.as_deref(),
            readme_body.as_deref(),
            repo,
            license_text,
            force,
        ) {
            eprintln!("  {e:?}");
            total_errors += 1;
            continue;
        }

        if let Err(e) = doc_injection::fix_doc_include(crate_dir, &meta.name, force) {
            eprintln!("  {e:?}");
            total_errors += 1;
        }
    }

    if total_errors > 0 {
        std::process::exit(1);
    }
}

fn unescape_newlines(s: &str) -> String {
    s.replace("\\n", "\n")
}
