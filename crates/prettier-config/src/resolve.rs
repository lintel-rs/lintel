use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::PrettierConfig;

/// Config file names to check, in precedence order.
const CONFIG_FILES: &[&str] = &[
    "package.json",
    "package.yaml",
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.yml",
    ".prettierrc.yaml",
    ".prettierrc.json5",
    ".prettierrc.toml",
];

/// Top-level config file representation with overrides support.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ConfigFile {
    #[serde(flatten)]
    config: PrettierConfig,
    overrides: Vec<Override>,
}

/// A single override entry.
#[derive(Debug, Clone, Default, Deserialize)]
struct Override {
    #[serde(default)]
    files: OverrideFiles,
    #[serde(default)]
    options: Option<serde_json::Value>,
}

/// Override file patterns — single string or array of strings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
enum OverrideFiles {
    Single(String),
    Multiple(Vec<String>),
    #[default]
    None,
}

/// Resolve prettier config for a given file path.
///
/// Walks up from the file's parent directory, checking for config files
/// in precedence order. Returns default options if no config found.
///
/// # Errors
///
/// Returns an error if a config file exists but cannot be read or parsed.
pub fn resolve_config(file_path: &Path) -> Result<PrettierConfig> {
    let file_path = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let start_dir = if file_path.is_file() {
        file_path.parent().unwrap_or(Path::new("."))
    } else {
        file_path.as_path()
    };

    let Some((config_dir, config_value)) = find_config(start_dir)? else {
        return Ok(PrettierConfig::default());
    };

    let config_file: ConfigFile = serde_json::from_value(config_value.clone())
        .with_context(|| "deserializing resolved config")?;

    let mut config = config_file.config;

    // Apply overrides
    apply_overrides(&file_path, &config_dir, &config_file.overrides, &mut config)?;

    Ok(config)
}

/// Walk up directories looking for a prettier config file.
/// Returns the directory and the raw JSON Value of the config.
fn find_config(start: &Path) -> Result<Option<(PathBuf, serde_json::Value)>> {
    let mut dir = start.to_path_buf();
    loop {
        for &name in CONFIG_FILES {
            let path = dir.join(name);
            if path.is_file()
                && let Some(val) = try_parse_config(&path, name)?
            {
                return Ok(Some((dir.clone(), val)));
            }
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(None)
}

/// Try to parse a config file. Returns None if the file doesn't contain
/// prettier config (e.g. package.json without "prettier" key).
fn try_parse_config(path: &Path, name: &str) -> Result<Option<serde_json::Value>> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    match name {
        "package.json" => {
            let val: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(val.get("prettier").cloned())
        }
        "package.yaml" => {
            let val: serde_yaml::Value = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            match val.get("prettier") {
                Some(prettier_val) => {
                    let json_val = serde_json::to_value(prettier_val)
                        .with_context(|| "converting YAML prettier config")?;
                    Ok(Some(json_val))
                }
                None => Ok(None),
            }
        }
        ".prettierrc" => {
            // Empty file is valid — means "use all defaults"
            if content.trim().is_empty() {
                return Ok(Some(serde_json::Value::Object(serde_json::Map::default())));
            }
            // Try JSON first, then YAML
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                return Ok(Some(val));
            }
            let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {} (tried JSON and YAML)", path.display()))?;
            let json_val = match serde_json::to_value(yaml_val)
                .with_context(|| "converting YAML config to JSON")?
            {
                // YAML parses empty/whitespace-only as Null — treat as empty object
                serde_json::Value::Null => serde_json::Value::Object(serde_json::Map::default()),
                v => v,
            };
            Ok(Some(json_val))
        }
        ".prettierrc.json" => {
            let val: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(val))
        }
        ".prettierrc.yml" | ".prettierrc.yaml" => {
            let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            let json_val =
                serde_json::to_value(yaml_val).with_context(|| "converting YAML config to JSON")?;
            Ok(Some(json_val))
        }
        ".prettierrc.json5" => {
            let val: serde_json::Value =
                json5::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(val))
        }
        ".prettierrc.toml" => {
            let toml_val: toml::Value =
                toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
            let json_val =
                serde_json::to_value(toml_val).with_context(|| "converting TOML config to JSON")?;
            Ok(Some(json_val))
        }
        _ => Ok(None),
    }
}

/// Apply overrides based on file path glob matching.
///
/// Override merging works by serializing the base config to a JSON Value,
/// shallow-merging the override options on top, then deserializing back.
fn apply_overrides(
    file_path: &Path,
    config_dir: &Path,
    overrides: &[Override],
    config: &mut PrettierConfig,
) -> Result<()> {
    let relative = file_path.strip_prefix(config_dir).unwrap_or(file_path);
    let relative_str = relative.to_string_lossy();

    for override_entry in overrides {
        let patterns = match &override_entry.files {
            OverrideFiles::Single(s) => vec![s.as_str()],
            OverrideFiles::Multiple(v) => v.iter().map(String::as_str).collect(),
            OverrideFiles::None => continue,
        };

        let matches = patterns
            .iter()
            .any(|pattern| glob_match::glob_match(pattern, &relative_str));

        if matches && let Some(ref override_opts) = override_entry.options {
            // Serialize current config to Value, merge override on top, deserialize back
            let mut base_val = serde_json::to_value(&*config)
                .with_context(|| "serializing config for override merge")?;
            json_merge(&mut base_val, override_opts);
            *config = serde_json::from_value(base_val)
                .with_context(|| "deserializing merged override config")?;
        }
    }

    Ok(())
}

/// Shallow-merge `overlay` into `base`. Overlay values overwrite base values.
fn json_merge(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    if let (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) =
        (base, overlay)
    {
        for (key, value) in overlay_map {
            base_map.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_when_no_config() {
        let opts = PrettierConfig::default();
        assert_eq!(opts.print_width, 80);
        assert_eq!(opts.tab_width, 2);
        assert!(!opts.use_tabs);
        assert!(!opts.single_quote);
        assert!(opts.bracket_spacing);
    }

    #[test]
    fn parse_json_config() {
        let content = r#"{"printWidth": 120, "tabWidth": 4, "useTabs": true}"#;
        let val: serde_json::Value = serde_json::from_str(content).expect("parse");
        let config: PrettierConfig = serde_json::from_value(val).expect("deser");
        assert_eq!(config.print_width, 120);
        assert_eq!(config.tab_width, 4);
        assert!(config.use_tabs);
    }

    #[test]
    fn parse_trailing_comma_variants() {
        for (input, expected) in [
            ("all", crate::TrailingComma::All),
            ("es5", crate::TrailingComma::Es5),
            ("none", crate::TrailingComma::None),
        ] {
            let content = format!(r#"{{"trailingComma": "{input}"}}"#);
            let val: serde_json::Value = serde_json::from_str(&content).expect("parse");
            let config: PrettierConfig = serde_json::from_value(val).expect("deser");
            assert_eq!(config.trailing_comma, expected);
        }
    }

    #[test]
    fn parse_end_of_line_variants() {
        for (input, expected) in [
            ("lf", crate::EndOfLine::Lf),
            ("crlf", crate::EndOfLine::Crlf),
            ("cr", crate::EndOfLine::Cr),
            ("auto", crate::EndOfLine::Auto),
        ] {
            let content = format!(r#"{{"endOfLine": "{input}"}}"#);
            let val: serde_json::Value = serde_json::from_str(&content).expect("parse");
            let config: PrettierConfig = serde_json::from_value(val).expect("deser");
            assert_eq!(config.end_of_line, expected);
        }
    }

    #[test]
    fn parse_prose_wrap_variants() {
        for (input, expected) in [
            ("always", crate::ProseWrap::Always),
            ("never", crate::ProseWrap::Never),
            ("preserve", crate::ProseWrap::Preserve),
        ] {
            let content = format!(r#"{{"proseWrap": "{input}"}}"#);
            let val: serde_json::Value = serde_json::from_str(&content).expect("parse");
            let config: PrettierConfig = serde_json::from_value(val).expect("deser");
            assert_eq!(config.prose_wrap, expected);
        }
    }

    #[test]
    fn parse_quote_props_variants() {
        for (input, expected) in [
            ("as-needed", crate::QuoteProps::AsNeeded),
            ("consistent", crate::QuoteProps::Consistent),
            ("preserve", crate::QuoteProps::Preserve),
        ] {
            let content = format!(r#"{{"quoteProps": "{input}"}}"#);
            let val: serde_json::Value = serde_json::from_str(&content).expect("parse");
            let config: PrettierConfig = serde_json::from_value(val).expect("deser");
            assert_eq!(config.quote_props, expected);
        }
    }

    #[test]
    fn partial_config_fills_defaults() {
        let content = r#"{"printWidth": 120}"#;
        let val: serde_json::Value = serde_json::from_str(content).expect("parse");
        let config: PrettierConfig = serde_json::from_value(val).expect("deser");
        assert_eq!(config.print_width, 120);
        // All other fields should be defaults
        assert_eq!(config.tab_width, 2);
        assert!(!config.use_tabs);
        assert!(config.bracket_spacing);
        assert_eq!(config.trailing_comma, crate::TrailingComma::All);
    }

    #[test]
    fn json_merge_works() {
        let mut base = serde_json::json!({"a": 1, "b": 2});
        let overlay = serde_json::json!({"b": 3, "c": 4});
        json_merge(&mut base, &overlay);
        assert_eq!(base, serde_json::json!({"a": 1, "b": 3, "c": 4}));
    }
}
