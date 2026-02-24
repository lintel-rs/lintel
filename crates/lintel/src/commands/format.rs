use std::io::IsTerminal;
use std::time::Instant;

use ansi_term_styles::{BOLD, DIM, RESET};
use anyhow::Result;

/// Run the `format` command: format files in place, or check with `--check`.
pub fn run(args: &lintel_format::FormatArgs, verbose: bool) -> Result<bool> {
    let start = Instant::now();
    let result = lintel_format::run(args)?;
    let had_errors = !result.errors.is_empty();

    if verbose {
        for path in &result.formatted {
            eprintln!("  ~ {path}");
        }
    }

    // In non-check mode, report write errors
    if !args.check {
        for (path, err) in &result.errors {
            eprintln!("error: {path}: {err}");
        }
    }

    let formatted = result.formatted.len();
    let unchanged = result.unchanged;
    let skipped = result.skipped;
    let ms = start.elapsed().as_millis();

    if args.check {
        let error_count = result.errors.len();
        if std::io::stderr().is_terminal() {
            eprintln!(
                "{BOLD}{error_count} files need formatting{RESET} {DIM}({unchanged} already formatted, {skipped} skipped) in {ms}ms.{RESET}"
            );
        } else {
            eprintln!(
                "{error_count} files need formatting ({unchanged} already formatted, {skipped} skipped) in {ms}ms."
            );
        }
    } else if std::io::stderr().is_terminal() {
        eprintln!(
            "{BOLD}Formatted {formatted} files{RESET} {DIM}({unchanged} unchanged, {skipped} skipped) in {ms}ms.{RESET}"
        );
    } else {
        eprintln!(
            "Formatted {formatted} files ({unchanged} unchanged, {skipped} skipped) in {ms}ms."
        );
    }

    Ok(had_errors)
}
