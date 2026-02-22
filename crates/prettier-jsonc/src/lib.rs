pub mod json;
pub mod jsonc;

use anyhow::Result;

pub use prettier_config::{self, PrettierConfig};
pub use wadler_lindig;

/// Backwards-compatible type alias.
pub type PrettierOptions = PrettierConfig;

/// Supported JSON format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonFormat {
    Json,
    Jsonc,
}

/// Format a string given its JSON format type.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed as the specified format.
pub fn format_str(content: &str, format: JsonFormat, options: &PrettierConfig) -> Result<String> {
    match format {
        JsonFormat::Json => {
            // Use JSONC parser for JSON too — it preserves number literals
            // and handles all valid JSON. For strict JSON format, disable
            // trailing commas since JSON doesn't support them.
            let mut json_options = options.clone();
            json_options.trailing_comma = prettier_config::TrailingComma::None;
            jsonc::format_jsonc(content, &json_options)
        }
        JsonFormat::Jsonc => jsonc::format_jsonc(content, options),
    }
}

/// Detect JSON format from file extension.
pub fn detect_format(ext: &str) -> Option<JsonFormat> {
    match ext {
        "json" => Some(JsonFormat::Json),
        "jsonc" => Some(JsonFormat::Jsonc),
        _ => None,
    }
}
