use alloc::collections::BTreeMap;

use serde::Deserialize;

/// Top-level configuration loaded from `lintel-catalog.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogConfig {
    #[allow(dead_code)]
    pub catalog: CatalogMeta,
    #[serde(default)]
    pub target: BTreeMap<String, TargetConfig>,
    #[serde(default)]
    pub groups: BTreeMap<String, GroupConfig>,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceConfig>,
}

/// Metadata about the catalog being built.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogMeta {
    /// Optional title for the catalog, included in the output `catalog.json`.
    #[serde(default)]
    pub title: Option<String>,
}

/// GitHub Pages hosting options (`.nojekyll`, `CNAME`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubPagesConfig {
    #[serde(default)]
    pub cname: Option<String>,
}

/// Target output configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TargetConfig {
    /// Write output to a local directory.
    #[serde(rename = "dir")]
    Dir {
        dir: String,
        base_url: String,
        #[serde(default)]
        github: Option<GitHubPagesConfig>,
    },
    /// Generate output optimized for GitHub Pages deployment.
    #[serde(rename = "github-pages")]
    GitHubPages {
        base_url: String,
        #[serde(default)]
        cname: Option<String>,
        #[serde(default)]
        dir: Option<String>,
    },
}

/// A named group of schema definitions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupConfig {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub schemas: BTreeMap<String, SchemaDefinition>,
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
    /// Map of directory name → organize entry with name, description, and match patterns.
    #[serde(default)]
    pub organize: BTreeMap<String, OrganizeEntry>,
}

/// An organize entry that classifies schemas from a source into a group directory.
///
/// Group metadata (name, description) is owned by the corresponding `[groups.*]`
/// entry; the organize section only handles schema routing via match patterns.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizeEntry {
    #[serde(rename = "match")]
    pub match_patterns: Vec<String>,
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
        let toml = r"
[catalog]
";
        let config = load_config(toml).expect("parse");
        assert!(config.target.is_empty());
        assert!(config.groups.is_empty());
        assert!(config.sources.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[catalog]

[target.local]
type = "dir"
dir = "../catalog-generated"
base_url = "https://raw.githubusercontent.com/lintel-rs/catalog/master/"

[target.pages]
type = "github-pages"
base_url = "https://catalog.lintel.tools/"
cname = "catalog.lintel.tools"

[groups.claude-code]
name = "Claude Code"
description = "Schemas for Claude Code configuration files"

[groups.claude-code.schemas]
agent = { name = "Claude Code Agent", description = "Agent definition", file-match = ["**/.claude/agents/*.md"] }
skill = { name = "Claude Code Skill", description = "Skill definition", file-match = ["**/skills/*.md"] }

[groups.devenv]
name = "devenv"
description = "Nix-based development environment configuration"

[groups.devenv.schemas]
devenv = { url = "https://devenv.sh/devenv.schema.json", name = "devenv.yaml", description = "devenv config", file-match = ["devenv.yaml"] }

[groups.github]
name = "GitHub"
description = "GitHub configuration files"

[sources.schemastore]
url = "https://www.schemastore.org/api/json/catalog.json"

[sources.schemastore.organize.github]
match = ["**.github**"]
"#;
        let config = load_config(toml).expect("parse");

        // Targets
        assert_eq!(config.target.len(), 2);
        match &config.target["local"] {
            TargetConfig::Dir { dir, base_url, .. } => {
                assert_eq!(dir, "../catalog-generated");
                assert_eq!(
                    base_url,
                    "https://raw.githubusercontent.com/lintel-rs/catalog/master/"
                );
            }
            TargetConfig::GitHubPages { .. } => panic!("expected Dir target"),
        }
        match &config.target["pages"] {
            TargetConfig::GitHubPages {
                base_url, cname, ..
            } => {
                assert_eq!(base_url, "https://catalog.lintel.tools/");
                assert_eq!(cname.as_deref(), Some("catalog.lintel.tools"));
            }
            TargetConfig::Dir { .. } => panic!("expected GitHubPages target"),
        }

        // Groups
        assert_eq!(config.groups.len(), 3);
        let claude_code = &config.groups["claude-code"];
        assert_eq!(claude_code.name, "Claude Code");
        assert_eq!(claude_code.schemas.len(), 2);
        assert_eq!(claude_code.schemas["agent"].name, "Claude Code Agent");
        assert!(claude_code.schemas["agent"].url.is_none());
        assert_eq!(
            claude_code.schemas["agent"].file_match,
            vec!["**/.claude/agents/*.md"]
        );

        let devenv = &config.groups["devenv"];
        assert_eq!(
            devenv.schemas["devenv"].url.as_deref(),
            Some("https://devenv.sh/devenv.schema.json")
        );

        // Sources
        assert_eq!(config.sources.len(), 1);
        let ss = &config.sources["schemastore"];
        assert_eq!(ss.url, "https://www.schemastore.org/api/json/catalog.json");
        let github_org = &ss.organize["github"];
        assert_eq!(github_org.match_patterns, vec!["**.github**"]);
    }

    #[test]
    fn unknown_fields_rejected() {
        let toml = r"
[catalog]
unknown_field = 'bad'
";
        assert!(load_config(toml).is_err());
    }

    #[test]
    fn github_pages_target_without_cname() {
        let toml = r#"
[catalog]

[target.pages]
type = "github-pages"
base_url = "https://example.github.io/catalog/"
"#;
        let config = load_config(toml).expect("parse");
        match &config.target["pages"] {
            TargetConfig::GitHubPages { cname, .. } => {
                assert!(cname.is_none());
            }
            TargetConfig::Dir { .. } => panic!("expected GitHubPages target"),
        }
    }
}
