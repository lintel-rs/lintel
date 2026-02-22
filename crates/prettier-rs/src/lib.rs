pub mod config;

use std::path::Path;

use anyhow::Result;

// Re-export from prettier-jsonc for backwards compatibility
pub use prettier_jsonc::options;
pub use prettier_jsonc::options::PrettierOptions;
pub use prettier_jsonc::printer;

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
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Json, options)
        }
        Format::Jsonc => {
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Jsonc, options)
        }
        Format::Json5 => {
            prettier_jsonc::format_str(content, prettier_jsonc::JsonFormat::Json5, options)
        }
        Format::Yaml => {
            let yaml_opts = prettier_yaml::YamlFormatOptions {
                print_width: options.print_width,
                tab_width: options.tab_width,
                use_tabs: options.use_tabs,
                single_quote: options.single_quote,
                bracket_spacing: options.bracket_spacing,
                prose_wrap: match options.prose_wrap {
                    options::ProseWrap::Always => prettier_yaml::ProseWrap::Always,
                    options::ProseWrap::Never => prettier_yaml::ProseWrap::Never,
                    options::ProseWrap::Preserve => prettier_yaml::ProseWrap::Preserve,
                },
            };
            prettier_yaml::format_yaml(content, &yaml_opts)
        }
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
