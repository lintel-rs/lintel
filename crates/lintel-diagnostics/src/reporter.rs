use core::time::Duration;

use lintel_schema_cache::CacheStatus;
use lintel_validation_cache::ValidationCacheStatus;

use crate::diagnostics::LintelDiagnostic;

/// A file that was checked and the schema it resolved to.
pub struct CheckedFile {
    pub path: String,
    pub schema: String,
    /// `None` for local schemas and builtins; `Some` for remote schemas.
    pub cache_status: Option<CacheStatus>,
    /// `None` when validation caching is not applicable; `Some` for validation cache hits/misses.
    pub validation_cache_status: Option<ValidationCacheStatus>,
}

/// Result of a check run (validation + optional format checking).
pub struct CheckResult {
    pub errors: Vec<LintelDiagnostic>,
    pub checked: Vec<CheckedFile>,
}

impl CheckResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn files_checked(&self) -> usize {
        self.checked.len()
    }
}

/// Format a verbose line for a checked file, including cache status tags.
pub fn format_checked_verbose(file: &CheckedFile) -> String {
    let schema_tag = match file.cache_status {
        Some(CacheStatus::Hit) => " [cached]",
        Some(CacheStatus::Miss | CacheStatus::Disabled) => " [fetched]",
        None => "",
    };
    let validation_tag = match file.validation_cache_status {
        Some(ValidationCacheStatus::Hit) => " [validated:cached]",
        Some(ValidationCacheStatus::Miss) => " [validated]",
        None => "",
    };
    format!(
        "  {} ({}){schema_tag}{validation_tag}",
        file.path, file.schema
    )
}

/// Trait for formatting and outputting check results.
pub trait Reporter {
    /// Called after all checks complete with the full result and elapsed time.
    fn report(&mut self, result: CheckResult, elapsed: Duration);

    /// Called each time a file is checked (for streaming progress).
    fn on_file_checked(&mut self, file: &CheckedFile);
}
