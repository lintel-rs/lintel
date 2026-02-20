use std::time::Duration;

use lintel_check::diagnostics::offset_to_line_col;
use lintel_check::validate::{CheckedFile, LintError, ValidateResult};

use crate::format_checked_verbose;
use crate::reporter::Reporter;

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
                LintError::Parse(d) => {
                    let content = d.src.inner();
                    offset_to_line_col(content, d.span.offset())
                }
                LintError::Validation(d) => {
                    let content = d.src.inner();
                    offset_to_line_col(content, d.span.offset())
                }
                LintError::File(_) => (1, 1),
            };

            let title = match error {
                LintError::Parse(_) => "parse error",
                LintError::Validation(d) if !d.instance_path.is_empty() => &d.instance_path,
                LintError::Validation(_) => "validation error",
                LintError::File(_) => "file error",
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
