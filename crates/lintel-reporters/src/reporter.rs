use core::time::Duration;

use lintel_check::validate::{CheckedFile, ValidateResult};

/// Trait for formatting and outputting validation results.
pub trait Reporter {
    /// Called after validation completes with the full result and elapsed time.
    fn report(&mut self, result: ValidateResult, elapsed: Duration);

    /// Called each time a file is checked (for streaming progress).
    fn on_file_checked(&mut self, file: &CheckedFile);
}
