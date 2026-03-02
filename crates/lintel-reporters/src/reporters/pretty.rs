use core::time::Duration;
use std::io::IsTerminal;

use ansi_term_styles::{BOLD, DIM, RESET};
use miette::Report;

use lintel_diagnostics::reporter::{CheckResult, CheckedFile, Reporter, format_checked_verbose};

/// Pretty reporter: fancy miette output with colors and timing.
pub struct PrettyReporter {
    pub verbose: bool,
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "error" } else { "errors" }
}

fn print_summary(n: usize, error_count: usize, ms: u128) {
    let label = plural(error_count);
    if error_count > 0 {
        if std::io::stderr().is_terminal() {
            eprintln!(
                "{BOLD}Checked {n} files{RESET} {DIM}in {ms}ms.{RESET} {BOLD}{error_count} {label} found.{RESET}"
            );
        } else {
            eprintln!("Checked {n} files in {ms}ms. {error_count} {label} found.");
        }
    } else if std::io::stderr().is_terminal() {
        eprintln!("{BOLD}Checked {n} files{RESET} {DIM}in {ms}ms.{RESET}");
    } else {
        eprintln!("Checked {n} files in {ms}ms.");
    }
}

impl Reporter for PrettyReporter {
    fn report(&mut self, result: CheckResult, elapsed: Duration) {
        let n = result.files_checked();
        let error_count = result.errors.len();

        for error in result.errors {
            eprintln!("{:?}", Report::new(error));
        }

        let ms = elapsed.as_millis();
        print_summary(n, error_count, ms);
    }

    fn on_file_checked(&mut self, file: &CheckedFile) {
        if self.verbose {
            eprintln!("{}", format_checked_verbose(file));
        }
    }
}
