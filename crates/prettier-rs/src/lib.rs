use std::path::Path;

use anyhow::Result;

// Re-export config types from prettier-config
pub use prettier_config::resolve::resolve_config;
pub use prettier_config::{self, PrettierConfig};

/// Backwards-compatible type alias.
pub type PrettierOptions = PrettierConfig;

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
pub fn format_str(content: &str, format: Format, options: &PrettierConfig) -> Result<String> {
    match format {
        Format::Json => {
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Json, options)
        }
        Format::Jsonc => {
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Jsonc, options)
        }
        Format::Json5 => {
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Json5, options)
        }
        Format::Yaml => prettier_yaml::format_yaml(content, options),
    }
}

/// Format a file in place. Returns true if the file was changed.
///
/// # Errors
///
/// Returns an error if the file cannot be read, parsed, or written.
pub fn format_file(path: &Path, options: Option<&PrettierConfig>) -> Result<bool> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;

    let format = detect_format(path)
        .ok_or_else(|| anyhow::anyhow!("unsupported file type: {}", path.display()))?;

    let resolved_options;
    let opts = if let Some(o) = options {
        o
    } else {
        resolved_options = resolve_config(path)?;
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
