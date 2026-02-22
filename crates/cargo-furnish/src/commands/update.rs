use std::path::PathBuf;

use bpaf::Bpaf;

use crate::{cargo_toml, doc_injection, readme, workspace};

#[derive(Debug, Clone, Bpaf)]
pub struct UpdateArgs {
    /// Crate description (used in Cargo.toml and README)
    #[bpaf(long("description"), argument("TEXT"))]
    pub description: Option<String>,
    /// README body markdown (inserted between description and License section)
    #[bpaf(long("readme"), argument("TEXT"))]
    pub readme: Option<String>,
    /// Comma-separated keywords
    #[bpaf(long("keywords"), argument("K1,K2,..."))]
    pub keywords: Option<String>,
    /// Comma-separated categories
    #[bpaf(long("categories"), argument("C1,C2,..."))]
    pub categories: Option<String>,
    /// Overwrite existing README and doc comments
    #[bpaf(long("force"), switch)]
    pub force: bool,
}

#[allow(clippy::needless_pass_by_value)] // CLI args are owned
pub fn run(crate_dirs: &[PathBuf], ws: &workspace::WorkspaceInfo, args: UpdateArgs) {
    // Unescape \n in text arguments
    let description = args.description.as_deref().map(unescape_newlines);
    let readme_body = args.readme.as_deref().map(unescape_newlines);

    let keywords: Option<Vec<String>> = args
        .keywords
        .as_deref()
        .map(|s| s.split(',').map(|k| k.trim().to_string()).collect());
    let categories: Option<Vec<String>> = args
        .categories
        .as_deref()
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

    let repo = ws
        .repository
        .as_deref()
        .unwrap_or("https://github.com/lintel-rs/lintel");
    let license_text = ws.license.as_deref().unwrap_or("Apache-2.0");

    let mut total_errors: usize = 0;

    let metadata_update = cargo_toml::MetadataUpdate {
        description: description.as_deref(),
        keywords: keywords.as_deref(),
        categories: categories.as_deref(),
        force: args.force,
    };

    for crate_dir in crate_dirs {
        let meta = match cargo_toml::fix_cargo_toml(crate_dir, ws, &metadata_update) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  error: {e:#}");
                total_errors += 1;
                continue;
            }
        };

        let readme_params = readme::ReadmeParams {
            crate_name: &meta.name,
            description: meta.description.as_deref(),
            body: readme_body.as_deref(),
            repository: repo,
            license_text,
        };
        if let Err(e) = readme::fix_readme(crate_dir, &readme_params, args.force) {
            eprintln!("  {e:?}");
            total_errors += 1;
            continue;
        }

        if let Err(e) = doc_injection::fix_doc_include(crate_dir, &meta.name, args.force) {
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
