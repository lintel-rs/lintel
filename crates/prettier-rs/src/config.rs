use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::options::{OverrideFiles, PrettierOptions, RawPrettierConfig};

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

/// Resolve prettier config for a given file path.
///
/// Walks up from the file's parent directory, checking for config files
/// in precedence order. Returns default options if no config found.
///
/// # Errors
///
/// Returns an error if a config file exists but cannot be read or parsed.
pub fn resolve_config(file_path: &Path) -> Result<PrettierOptions> {
    let file_path = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let start_dir = if file_path.is_file() {
        file_path.parent().unwrap_or(Path::new("."))
    } else {
        file_path.as_path()
    };

    let mut opts = PrettierOptions::default();

    if let Some((config_dir, raw)) = find_config(start_dir)? {
        raw.apply_to(&mut opts);

        // Apply overrides
        if let Some(overrides) = &raw.overrides {
            apply_overrides(&file_path, &config_dir, overrides, &mut opts);
        }
    }

    Ok(opts)
}

/// Walk up directories looking for a prettier config file.
fn find_config(start: &Path) -> Result<Option<(PathBuf, RawPrettierConfig)>> {
    let mut dir = start.to_path_buf();
    loop {
        for &name in CONFIG_FILES {
            let path = dir.join(name);
            if path.is_file()
                && let Some(raw) = try_parse_config(&path, name)?
            {
                return Ok(Some((dir.clone(), raw)));
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
fn try_parse_config(path: &Path, name: &str) -> Result<Option<RawPrettierConfig>> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    match name {
        "package.json" => {
            let val: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            match val.get("prettier") {
                Some(prettier_val) => {
                    let raw: RawPrettierConfig = serde_json::from_value(prettier_val.clone())
                        .with_context(|| format!("parsing prettier key in {}", path.display()))?;
                    Ok(Some(raw))
                }
                None => Ok(None),
            }
        }
        "package.yaml" => {
            let val: serde_yaml::Value = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            match val.get("prettier") {
                Some(prettier_val) => {
                    let json_val = serde_json::to_value(prettier_val)
                        .with_context(|| "converting YAML prettier config")?;
                    let raw: RawPrettierConfig = serde_json::from_value(json_val)
                        .with_context(|| format!("parsing prettier key in {}", path.display()))?;
                    Ok(Some(raw))
                }
                None => Ok(None),
            }
        }
        ".prettierrc" => {
            // Try JSON first, then YAML
            if let Ok(raw) = serde_json::from_str::<RawPrettierConfig>(&content) {
                return Ok(Some(raw));
            }
            let raw: RawPrettierConfig = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {} (tried JSON and YAML)", path.display()))?;
            Ok(Some(raw))
        }
        ".prettierrc.json" => {
            let raw: RawPrettierConfig = serde_json::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(raw))
        }
        ".prettierrc.yml" | ".prettierrc.yaml" => {
            let raw: RawPrettierConfig = serde_yaml::from_str(&content)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(raw))
        }
        ".prettierrc.json5" => {
            let raw: RawPrettierConfig =
                json5::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(raw))
        }
        ".prettierrc.toml" => {
            let raw: RawPrettierConfig =
                toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
            Ok(Some(raw))
        }
        _ => Ok(None),
    }
}

/// Apply overrides based on file path glob matching.
fn apply_overrides(
    file_path: &Path,
    config_dir: &Path,
    overrides: &[crate::options::RawOverride],
    opts: &mut PrettierOptions,
) {
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
            override_opts.apply_to(opts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_when_no_config() {
        let opts = PrettierOptions::default();
        assert_eq!(opts.print_width, 80);
        assert_eq!(opts.tab_width, 2);
        assert!(!opts.use_tabs);
        assert!(!opts.single_quote);
        assert!(opts.bracket_spacing);
    }

    #[test]
    fn parse_json_config() {
        let content = r#"{"printWidth": 120, "tabWidth": 4, "useTabs": true}"#;
        let raw: RawPrettierConfig = serde_json::from_str(content).expect("parse");
        let mut opts = PrettierOptions::default();
        raw.apply_to(&mut opts);
        assert_eq!(opts.print_width, 120);
        assert_eq!(opts.tab_width, 4);
        assert!(opts.use_tabs);
    }

    #[test]
    fn parse_trailing_comma_variants() {
        for (input, expected) in [
            ("all", crate::options::TrailingComma::All),
            ("es5", crate::options::TrailingComma::Es5),
            ("none", crate::options::TrailingComma::None),
        ] {
            let content = format!(r#"{{"trailingComma": "{input}"}}"#);
            let raw: RawPrettierConfig = serde_json::from_str(&content).expect("parse");
            let mut opts = PrettierOptions::default();
            raw.apply_to(&mut opts);
            assert_eq!(opts.trailing_comma, expected);
        }
    }
}
