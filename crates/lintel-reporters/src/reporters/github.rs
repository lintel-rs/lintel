use core::time::Duration;

use lintel_validate::diagnostics::{DEFAULT_LABEL, offset_to_line_col};
use lintel_validate::format_checked_verbose;
use lintel_validate::reporter::Reporter;
use lintel_validate::validate::{CheckedFile, LintError, ValidateResult};

/// GitHub Actions reporter: emits `::error` workflow commands to stdout.
pub struct GithubReporter {
    pub verbose: bool,
}

/// Escape a string for GitHub Actions workflow commands.
///
/// `%` -> `%25`, `\n` -> `%0A`, `\r` -> `%0D`
fn escape_workflow(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Normalize path separators to forward slashes for GitHub.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

impl Reporter for GithubReporter {
    fn report(&mut self, result: ValidateResult, elapsed: Duration) {
        let error_count = result.errors.len();
        let n = result.files_checked();

        for error in &result.errors {
            let path = normalize_path(error.path());
            let message = escape_workflow(error.message());

            let (line, col) = match error {
                LintError::Parse { src, span, .. }
                | LintError::Validation { src, span, .. }
                | LintError::Config { src, span, .. } => {
                    offset_to_line_col(src.inner(), span.offset())
                }
                LintError::Io { .. }
                | LintError::SchemaFetch { .. }
                | LintError::SchemaCompile { .. } => (1, 1),
            };

            let title = match error {
                LintError::Parse { .. } => "parse error",
                LintError::Validation { instance_path, .. } if instance_path != DEFAULT_LABEL => {
                    instance_path
                }
                LintError::Validation { .. } => "validation error",
                LintError::Config { .. } => "config error",
                LintError::Io { .. } => "io error",
                LintError::SchemaFetch { .. } => "schema fetch error",
                LintError::SchemaCompile { .. } => "schema compile error",
            };

            println!("::error file={path},line={line},col={col},title={title}::{message}");
        }

        // Summary to stderr (not parsed as workflow commands)
        let ms = elapsed.as_millis();
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
