pub mod json;
pub mod json5;
pub mod jsonc;
pub mod options;
pub mod printer;

use anyhow::Result;

pub use options::PrettierOptions;

/// Supported JSON format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonFormat {
    Json,
    Jsonc,
    Json5,
}

/// Format a string given its JSON format type.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed as the specified format.
pub fn format_str(content: &str, format: JsonFormat, options: &PrettierOptions) -> Result<String> {
    match format {
        JsonFormat::Json => {
            // Use JSONC parser for JSON too — it preserves number literals
            // and handles all valid JSON. For strict JSON format, disable
            // trailing commas since JSON doesn't support them.
            let mut json_options = options.clone();
            json_options.trailing_comma = options::TrailingComma::None;
            if let Ok(result) = jsonc::format_jsonc(content, &json_options) {
                Ok(result)
            } else {
                // If JSONC parser fails (e.g. +123, Infinity), try JSON5
                let mut j5_options = options.clone();
                j5_options.trailing_comma = options::TrailingComma::None;
                j5_options.single_quote = false;
                j5_options.quote_props = options::QuoteProps::Consistent;
                json5::format_json5(content, &j5_options)
            }
        }
        JsonFormat::Jsonc => jsonc::format_jsonc(content, options),
        JsonFormat::Json5 => json5::format_json5(content, options),
    }
}

/// Detect JSON format from file extension.
pub fn detect_format(ext: &str) -> Option<JsonFormat> {
    match ext {
        "json" => Some(JsonFormat::Json),
        "jsonc" => Some(JsonFormat::Jsonc),
        "json5" => Some(JsonFormat::Json5),
        _ => None,
    }
}
