use std::time::Duration;

use lintel_check::validate::{CheckedFile, LintError, ValidateResult};

use crate::format_checked_verbose;
use crate::reporter::Reporter;

/// Text reporter: plain one-line-per-error output suitable for CI pipelines.
pub struct TextReporter {
    pub verbose: bool,
}

impl Reporter for TextReporter {
    fn report(&mut self, result: ValidateResult, elapsed: Duration) {
        let error_count = result.errors.len();
        for error in &result.errors {
            let path = error.path();
            match error {
                LintError::Validation(d) if !d.instance_path.is_empty() => {
                    eprintln!(
                        "error: {path}: {} (at {})",
                        error.message(),
                        d.instance_path
                    );
                }
                _ => {
                    eprintln!("error: {path}: {}", error.message());
                }
            }
        }

        let ms = elapsed.as_millis();
        let n = result.files_checked();
        if error_count > 0 {
            eprintln!("Checked {n} files in {ms}ms. {error_count} error(s) found.");
        } else {
            eprintln!("Checked {n} files in {ms}ms. No errors.");
        }
    }

    fn on_file_checked(&mut self, file: &CheckedFile) {
        if self.verbose {
            eprintln!("{}", format_checked_verbose(file));
        }
    }
}
