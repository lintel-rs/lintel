use core::time::Duration;

use lintel_diagnostics::DEFAULT_LABEL;
use lintel_diagnostics::LintelDiagnostic;
use lintel_diagnostics::offset_to_line_col;
use lintel_diagnostics::reporter::{CheckResult, CheckedFile, Reporter, format_checked_verbose};

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

fn emit_lint_error(error: &LintelDiagnostic) {
    let path = normalize_path(error.path());
    let message = escape_workflow(error.message());

    let (line, col) = match error {
        LintelDiagnostic::Parse { src, span, .. }
        | LintelDiagnostic::Validation { src, span, .. } => {
            offset_to_line_col(src.inner(), span.offset())
        }
        LintelDiagnostic::Io { .. }
        | LintelDiagnostic::SchemaFetch { .. }
        | LintelDiagnostic::SchemaCompile { .. }
        | LintelDiagnostic::Format { .. } => (1, 1),
    };

    let title = match error {
        LintelDiagnostic::Parse { .. } => "parse error",
        LintelDiagnostic::Validation { instance_path, .. } if instance_path != DEFAULT_LABEL => {
            instance_path
        }
        LintelDiagnostic::Validation { .. } => "validation error",
        LintelDiagnostic::Io { .. } => "io error",
        LintelDiagnostic::SchemaFetch { .. } => "schema fetch error",
        LintelDiagnostic::SchemaCompile { .. } => "schema compile error",
        LintelDiagnostic::Format { .. } => "format error",
    };

    println!("::error file={path},line={line},col={col},title={title}::{message}");
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

impl Reporter for GithubReporter {
    fn report(&mut self, result: CheckResult, elapsed: Duration) {
        let n = result.files_checked();
        let error_count = result.errors.len();

        for error in &result.errors {
            emit_lint_error(error);
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
