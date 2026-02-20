pub mod config;
pub mod json;
pub mod json5;
pub mod jsonc;
pub mod options;
pub mod printer;
pub mod yaml;

use std::path::Path;

use anyhow::Result;

pub use options::PrettierOptions;

/// Supported format types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Jsonc,
    Json5,
    Yaml,
}

/// Format a string given its format type.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed as the specified format.
pub fn format_str(content: &str, format: Format, options: &PrettierOptions) -> Result<String> {
    match format {
        Format::Json => {
            let value: serde_json::Value = serde_json::from_str(content)
                .map_err(|e| anyhow::anyhow!("JSON parse error: {e}"))?;
            let doc = json::json_to_doc(&value, options);
            let mut result = printer::print(&doc, options);
            result.push('\n');
            Ok(result)
        }
        Format::Jsonc => jsonc::format_jsonc(content, options),
        Format::Json5 => json5::format_json5(content, options),
        Format::Yaml => yaml::format_yaml(content, options),
    }
}

/// Format a file in place. Returns true if the file was changed.
///
/// # Errors
///
/// Returns an error if the file cannot be read, parsed, or written.
pub fn format_file(path: &Path, options: Option<&PrettierOptions>) -> Result<bool> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;

    let format = detect_format(path)
        .ok_or_else(|| anyhow::anyhow!("unsupported file type: {}", path.display()))?;

    let resolved_options;
    let opts = if let Some(o) = options {
        o
    } else {
        resolved_options = config::resolve_config(path)?;
        &resolved_options
    };

    let formatted = format_str(&content, format, opts)?;

    if formatted == content {
        return Ok(false);
    }

    std::fs::write(path, &formatted)
        .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;
    Ok(true)
}

/// Resolve prettier config for a file path.
///
/// # Errors
///
/// Returns an error if a config file exists but cannot be read or parsed.
pub fn resolve_config(path: &Path) -> Result<PrettierOptions> {
    config::resolve_config(path)
}

/// Detect format from file extension.
pub fn detect_format(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "json" => Some(Format::Json),
        "jsonc" => Some(Format::Jsonc),
        "json5" => Some(Format::Json5),
        "yaml" | "yml" => Some(Format::Yaml),
        _ => None,
    }
}
