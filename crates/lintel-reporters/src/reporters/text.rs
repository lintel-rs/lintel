use core::time::Duration;

use lintel_validate::diagnostics::DEFAULT_LABEL;
use lintel_validate::format_checked_verbose;
use lintel_validate::reporter::Reporter;
use lintel_validate::validate::{CheckedFile, LintError, ValidateResult};

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
                LintError::Validation { instance_path, .. }
                | LintError::Config { instance_path, .. }
                    if instance_path != DEFAULT_LABEL =>
                {
                    eprintln!("error: {path}: {} (at {instance_path})", error.message(),);
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
