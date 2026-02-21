use alloc::collections::BTreeMap;

use serde::Deserialize;

/// Top-level configuration loaded from `lintel-catalog.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogConfig {
    pub catalog: CatalogMeta,
    #[serde(default)]
    pub groups: BTreeMap<String, BTreeMap<String, SchemaDefinition>>,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceConfig>,
}

/// Metadata about the catalog being built.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogMeta {
    pub base_url: String,
}

/// A single schema definition within a group.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaDefinition {
    /// URL to download the schema from. If absent, the schema is expected to
    /// already exist locally at `schemas/<group>/<key>.json`.
    pub url: Option<String>,
    pub name: String,
    pub description: String,
    #[serde(default, rename = "file-match")]
    pub file_match: Vec<String>,
}

/// An external catalog source (e.g. `SchemaStore`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    /// URL to the external catalog JSON.
    pub url: String,
    /// Map of directory name → list of matchers (URL prefixes or glob patterns).
    #[serde(default)]
    pub organize: BTreeMap<String, Vec<String>>,
}

/// Load a `CatalogConfig` from a TOML string.
///
/// # Errors
///
/// Returns an error if the TOML is invalid or does not match the expected schema.
pub fn load_config(toml_str: &str) -> Result<CatalogConfig, toml::de::Error> {
    toml::from_str(toml_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
[catalog]
base_url = "https://example.com/"
"#;
        let config = load_config(toml).expect("parse");
        assert_eq!(config.catalog.base_url, "https://example.com/");
        assert!(config.groups.is_empty());
        assert!(config.sources.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[catalog]
base_url = "https://raw.githubusercontent.com/lintel-rs/catalog/master/"

[groups.claude-code]
agent = { name = "Claude Code Agent", description = "Agent definition", file-match = ["**/.claude/agents/*.md"] }
skill = { name = "Claude Code Skill", description = "Skill definition", file-match = ["**/skills/*.md"] }

[groups.devenv]
devenv = { url = "https://devenv.sh/devenv.schema.json", name = "devenv.yaml", description = "devenv config", file-match = ["devenv.yaml"] }

[sources.schemastore]
url = "https://www.schemastore.org/api/json/catalog.json"
organize = { github = ["**.github**"] }
"#;
        let config = load_config(toml).expect("parse");
        assert_eq!(
            config.catalog.base_url,
            "https://raw.githubusercontent.com/lintel-rs/catalog/master/"
        );

        // Groups
        assert_eq!(config.groups.len(), 2);
        let claude_code = &config.groups["claude-code"];
        assert_eq!(claude_code.len(), 2);
        assert_eq!(claude_code["agent"].name, "Claude Code Agent");
        assert!(claude_code["agent"].url.is_none());
        assert_eq!(
            claude_code["agent"].file_match,
            vec!["**/.claude/agents/*.md"]
        );

        let devenv = &config.groups["devenv"];
        assert_eq!(
            devenv["devenv"].url.as_deref(),
            Some("https://devenv.sh/devenv.schema.json")
        );

        // Sources
        assert_eq!(config.sources.len(), 1);
        let ss = &config.sources["schemastore"];
        assert_eq!(ss.url, "https://www.schemastore.org/api/json/catalog.json");
        assert_eq!(ss.organize["github"], vec!["**.github**"]);
    }

    #[test]
    fn unknown_fields_rejected() {
        let toml = r#"
[catalog]
base_url = "https://example.com/"
unknown_field = "bad"
"#;
        assert!(load_config(toml).is_err());
    }
}
