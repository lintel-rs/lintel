use core::time::Duration;
use std::io::IsTerminal;

use ansi_term_styles::{BOLD, DIM, RESET};
use miette::Report;

use lintel_validate::format_checked_verbose;
use lintel_validate::reporter::Reporter;
use lintel_validate::validate::{CheckedFile, ValidateResult};

/// Pretty reporter: fancy miette output with colors and timing.
pub struct PrettyReporter {
    pub verbose: bool,
}

impl Reporter for PrettyReporter {
    fn report(&mut self, result: ValidateResult, elapsed: Duration) {
        let n = result.files_checked();
        for error in result.errors {
            eprintln!("{:?}", Report::new(error));
        }

        let ms = elapsed.as_millis();
        if std::io::stderr().is_terminal() {
            eprintln!("{BOLD}Checked {n} files{RESET} {DIM}in {ms}ms.{RESET}");
        } else {
            eprintln!("Checked {n} files in {ms}ms.");
        }
    }

    fn on_file_checked(&mut self, file: &CheckedFile) {
        if self.verbose {
            eprintln!("{}", format_checked_verbose(file));
        }
    }
}
