use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{ConvertArgs, OutputFormat};

/// Detect input format from file extension.
fn detect_input_format(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json" | "json5" | "jsonc") => Some("json"),
        Some("yaml" | "yml") => Some("yaml"),
        Some("toml") => Some("toml"),
        _ => None,
    }
}

/// Parse input file to `serde_json::Value`.
fn parse_input(content: &str, format: &str) -> Result<Value> {
    match format {
        "json" => serde_json::from_str(content).context("failed to parse JSON"),
        "yaml" => serde_yaml::from_str(content).context("failed to parse YAML"),
        "toml" => {
            let toml_value: toml::Value =
                toml::from_str(content).context("failed to parse TOML")?;
            serde_json::to_value(toml_value).context("failed to convert TOML to JSON value")
        }
        _ => bail!("unsupported input format: {format}"),
    }
}

/// Serialize a value to the target format.
fn serialize_output(value: &Value, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(value).context("failed to serialize JSON")
        }
        OutputFormat::Yaml => serde_yaml::to_string(value).context("failed to serialize YAML"),
        OutputFormat::Toml => {
            // Convert via toml::Value for proper TOML serialization
            let toml_value: toml::Value = serde_json::from_value(value.clone())
                .context("value cannot be represented as TOML")?;
            toml::to_string_pretty(&toml_value).context("failed to serialize TOML")
        }
    }
}

/// Run the `convert` command: read a file, convert it to the target format, print to stdout.
pub fn run(args: &ConvertArgs) -> Result<()> {
    let path = Path::new(&args.file);
    let input_format = detect_input_format(path).with_context(|| {
        format!(
            "cannot detect format of {}, use a known extension (.json, .yaml, .toml)",
            args.file
        )
    })?;

    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", args.file))?;

    let value = parse_input(&content, input_format)?;
    let output = serialize_output(&value, args.to)?;
    print!("{output}");
    Ok(())
}
