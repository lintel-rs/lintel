use std::path::PathBuf;

use crate::{cargo_toml, doc_injection, readme, workspace};

pub fn run(crate_dirs: &[PathBuf], ws: &workspace::WorkspaceInfo) {
    let mut total_diagnostics: usize = 0;

    for crate_dir in crate_dirs {
        let (meta, cargo_diags) = match cargo_toml::check_cargo_toml(crate_dir, ws) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  error: {e:#}");
                total_diagnostics += 1;
                continue;
            }
        };

        let readme_diags = readme::check_readme(crate_dir, &meta.name);
        let doc_diags = doc_injection::check_doc_include(crate_dir, &meta.name);

        let diag_count = cargo_diags.len() + readme_diags.len() + doc_diags.len();
        total_diagnostics += diag_count;

        for d in cargo_diags {
            let report = miette::Report::new_boxed(d);
            eprintln!("{report:?}");
        }
        for d in readme_diags {
            let report = miette::Report::new_boxed(d);
            eprintln!("{report:?}");
        }
        for d in doc_diags {
            let report = miette::Report::new_boxed(d);
            eprintln!("{report:?}");
        }
    }

    if total_diagnostics > 0 {
        eprintln!(
            "\nfound {total_diagnostics} issue{}.",
            if total_diagnostics == 1 { "" } else { "s" }
        );
        std::process::exit(1);
    }
}

pub fn run_fix(crate_dirs: &[PathBuf], ws: &workspace::WorkspaceInfo) {
    let mut total_errors: usize = 0;

    for crate_dir in crate_dirs {
        let meta = match cargo_toml::fix_cargo_toml(crate_dir, ws, None, None, None, true) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  error: {e:#}");
                total_errors += 1;
                continue;
            }
        };

        if let Err(e) = doc_injection::fix_doc_include(crate_dir, &meta.name, false) {
            eprintln!("  {e:?}");
            total_errors += 1;
        }
    }

    if total_errors > 0 {
        std::process::exit(1);
    }
}
