use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use crate::{cargo_toml, doc_injection, readme, workspace};

use ansi_term_styles::{BOLD, BOLD_GREEN as GREEN, BOLD_RED as RED, RESET};

pub fn run(crate_dirs: &[PathBuf], ws: &workspace::WorkspaceInfo) {
    let start = Instant::now();
    let mut total_diagnostics: usize = 0;
    let crate_count = crate_dirs.len();
    let color = std::io::stderr().is_terminal();

    for crate_dir in crate_dirs {
        let (meta, cargo_diags) = match cargo_toml::check_cargo_toml(crate_dir, ws) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  error: {e:#}");
                total_diagnostics += 1;
                continue;
            }
        };

        let readme_diags = readme::check_readme(crate_dir, &meta.name, meta.description.as_deref());
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

    let elapsed = start.elapsed();
    let cs = if crate_count == 1 { "" } else { "s" };

    if total_diagnostics > 0 {
        let is = if total_diagnostics == 1 { "" } else { "s" };
        if color {
            eprintln!(
                "\n{RED}Checked{RESET} {crate_count} crate{cs} in {elapsed:.0?}: {BOLD}{total_diagnostics} issue{is}{RESET}."
            );
        } else {
            eprintln!(
                "\nChecked {crate_count} crate{cs} in {elapsed:.0?}: {total_diagnostics} issue{is}."
            );
        }
        std::process::exit(1);
    }

    if color {
        eprintln!(
            "{GREEN}Checked{RESET} {crate_count} crate{cs} in {elapsed:.0?}: no issues found."
        );
    } else {
        eprintln!("Checked {crate_count} crate{cs} in {elapsed:.0?}: no issues found.");
    }
}

pub fn run_fix(crate_dirs: &[PathBuf], ws: &workspace::WorkspaceInfo) {
    let mut total_errors: usize = 0;

    for crate_dir in crate_dirs {
        let fix_update = cargo_toml::MetadataUpdate {
            description: None,
            keywords: None,
            categories: None,
            force: true,
        };
        let meta = match cargo_toml::fix_cargo_toml(crate_dir, ws, &fix_update) {
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
