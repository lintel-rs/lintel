use core::time::Duration;

use lintel_diagnostics::DEFAULT_LABEL;
use lintel_diagnostics::LintelDiagnostic;
use lintel_diagnostics::reporter::{CheckResult, CheckedFile, Reporter, format_checked_verbose};

/// Text reporter: plain one-line-per-error output suitable for CI pipelines.
pub struct TextReporter {
    pub verbose: bool,
}

fn print_lint_errors(errors: &[LintelDiagnostic]) {
    for error in errors {
        let path = error.path();
        match error {
            LintelDiagnostic::Validation { instance_path, .. }
                if instance_path != DEFAULT_LABEL =>
            {
                eprintln!("error: {path}: {} (at {instance_path})", error.message(),);
            }
            _ => {
                eprintln!("error: {path}: {}", error.message());
            }
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "error" } else { "errors" }
}

fn print_summary(n: usize, error_count: usize, ms: u128) {
    if error_count > 0 {
        let label = plural(error_count);
        eprintln!("Checked {n} files in {ms}ms. {error_count} {label} found.");
    } else {
        eprintln!("Checked {n} files in {ms}ms. No errors.");
    }
}

impl Reporter for TextReporter {
    fn report(&mut self, result: CheckResult, elapsed: Duration) {
        let n = result.files_checked();
        let error_count = result.errors.len();

        print_lint_errors(&result.errors);

        let ms = elapsed.as_millis();
        print_summary(n, error_count, ms);
    }

    fn on_file_checked(&mut self, file: &CheckedFile) {
        if self.verbose {
            eprintln!("{}", format_checked_verbose(file));
        }
    }
}
