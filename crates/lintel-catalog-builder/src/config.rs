use alloc::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;

/// Configuration file for the Lintel catalog builder.
///
/// Defines how to build a JSON Schema catalog from local schema definitions and
/// external sources. The catalog builder reads this file, fetches schemas,
/// organizes them into groups, and writes the output to one or more targets.
///
/// Place this file at the root of your catalog repository.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "lintel-catalog.toml")]
pub struct CatalogConfig {
    /// Catalog metadata such as the catalog title. Corresponds to the
    /// `[catalog]` TOML section.
    #[allow(dead_code)]
    pub catalog: CatalogMeta,

    /// Named build targets that control where output files are written.
    ///
    /// Each key is a target name (e.g. `local`, `pages`) and the value
    /// specifies the target type and its options. Multiple targets can be built
    /// in a single run.
    ///
    /// Corresponds to `[target.<name>]` sections in TOML.
    #[serde(default)]
    pub target: BTreeMap<String, TargetConfig>,

    /// Named schema groups.
    ///
    /// Each key is a group identifier (used as the output directory name) and
    /// the value defines the group's display name, description, and schema
    /// definitions.
    ///
    /// Corresponds to `[groups.<name>]` sections in TOML.
    #[serde(default)]
    pub groups: BTreeMap<String, GroupConfig>,

    /// Named external catalog sources to import schemas from.
    ///
    /// Each key is a source identifier and the value specifies the catalog URL
    /// and optional organization rules that route imported schemas into groups.
    ///
    /// Corresponds to `[sources.<name>]` sections in TOML.
    #[serde(default)]
    pub sources: BTreeMap<String, SourceConfig>,
}

/// Metadata for the catalog, specified in the `[catalog]` section.
///
/// This section is required even if empty.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "Catalog Metadata")]
pub struct CatalogMeta {
    /// Human-readable title for the catalog, included in the generated
    /// `catalog.json` output.
    #[schemars(example = &"Lintel Schema Catalog")]
    #[serde(default)]
    pub title: Option<String>,
}

/// Options for GitHub Pages hosting.
///
/// When present on a `dir` target, a `.nojekyll` file is created and an
/// optional `CNAME` file is written.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "GitHub Pages Options")]
pub struct GitHubPagesConfig {
    /// Custom domain for GitHub Pages. When set, a `CNAME` file is written to
    /// the output directory with this value.
    #[schemars(example = &"catalog.example.com")]
    #[serde(default)]
    pub cname: Option<String>,
}

/// Output target configuration.
///
/// Each target specifies where the built catalog and schema files are written.
/// Use `type = "dir"` for a plain directory output or `type = "github-pages"`
/// for GitHub Pages deployment with automatic `.nojekyll` and optional `CNAME`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
#[schemars(title = "Build Target")]
pub enum TargetConfig {
    /// Write output to a local directory.
    #[serde(rename = "dir")]
    Dir {
        /// Output directory path (relative to the catalog repository root).
        #[schemars(example = &"../catalog-generated")]
        dir: String,
        /// Base URL where the catalog will be hosted. Schema URLs in
        /// `catalog.json` are constructed relative to this URL.
        #[schemars(example = &"https://raw.githubusercontent.com/org/catalog/master/")]
        #[serde(rename = "base-url", alias = "base_url")]
        base_url: String,
        /// Optional GitHub Pages settings. When present, a `.nojekyll` file is
        /// created in the output directory.
        #[serde(default)]
        github: Option<GitHubPagesConfig>,
    },
    /// Generate output optimized for GitHub Pages deployment.
    #[serde(rename = "github-pages")]
    GitHubPages {
        /// Base URL where the GitHub Pages site is hosted.
        #[schemars(example = &"https://catalog.example.com/")]
        #[serde(rename = "base-url", alias = "base_url")]
        base_url: String,
        /// Custom domain for GitHub Pages. When set, a `CNAME` file is written
        /// to the output directory.
        #[serde(default)]
        cname: Option<String>,
        /// Output directory path. Defaults to `docs/` if not specified.
        #[serde(default)]
        dir: Option<String>,
    },
}

/// A named collection of related schema definitions.
///
/// Groups organize schemas into directories in the built catalog. Each group
/// has a display name and description that appear in the catalog index, and
/// contains one or more schema definitions.
///
/// Corresponds to a `[groups.<id>]` section in TOML.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "Schema Group")]
pub struct GroupConfig {
    /// Human-readable display name for this group.
    #[schemars(example = &"GitHub", example = &"Claude Code")]
    pub name: String,
    /// Short description of the schemas in this group, shown in the catalog
    /// index.
    pub description: String,
    /// Schema definitions within this group.
    ///
    /// Each key is a schema identifier (used as the filename, e.g. `agent` ->
    /// `agent.json`) and the value describes the schema source, display name,
    /// and file-match patterns.
    ///
    /// Corresponds to `[groups.<group>.schemas.<id>]` sections in TOML.
    #[serde(default)]
    pub schemas: BTreeMap<String, SchemaDefinition>,
}

/// An individual schema entry within a group.
///
/// Defines where to obtain the schema, its display metadata, and which files it
/// should match in the catalog.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "Schema Definition")]
pub struct SchemaDefinition {
    /// URL to download the schema from.
    ///
    /// If omitted, the schema is expected to already exist locally at
    /// `schemas/<group>/<key>.json`.
    pub url: Option<String>,
    /// Human-readable display name for this schema.
    #[schemars(example = &"GitHub Workflow", example = &"devenv.yaml")]
    pub name: String,
    /// Short description of what this schema validates.
    pub description: String,
    /// Glob patterns for files this schema should be auto-associated with.
    ///
    /// Editors and tools use these patterns to automatically apply the schema
    /// when a matching file is opened.
    #[schemars(title = "File Match", example = &["**/.github/workflows/*.yml"], example = &["devenv.yaml"])]
    #[serde(default)]
    pub file_match: Vec<String>,
}

/// An external schema catalog to import schemas from.
///
/// The catalog builder fetches the JSON catalog from the given URL, then uses
/// the `organize` rules to route matching schemas into local groups.
///
/// Corresponds to a `[sources.<id>]` section in TOML.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "External Catalog Source")]
pub struct SourceConfig {
    /// URL to the external catalog JSON file (in `SchemaStore` format:
    /// `{"schemas": [...]}`).
    #[schemars(example = &"https://www.schemastore.org/api/json/catalog.json")]
    pub url: String,
    /// Rules for routing schemas from this source into local groups.
    ///
    /// Each key is a group identifier (matching a key in `[groups]`) and the
    /// value contains glob patterns. Schemas whose names or URLs match any
    /// pattern are placed into that group.
    ///
    /// Corresponds to `[sources.<source>.organize.<group>]` sections in TOML.
    #[serde(default)]
    pub organize: BTreeMap<String, OrganizeEntry>,
}

/// Routing rule that assigns schemas from an external source to a local group.
///
/// Contains glob patterns to match against schema names or URLs. Group metadata
/// (display name, description) is defined in the corresponding `[groups]`
/// entry.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(title = "Organize Entry")]
pub struct OrganizeEntry {
    /// Glob patterns matched against schema names or URLs from the external
    /// catalog.
    ///
    /// Schemas matching any pattern are imported into the corresponding group
    /// directory.
    #[schemars(example = &["**.github**"], example = &["*docker*"])]
    #[serde(rename = "match")]
    pub match_patterns: Vec<String>,
}

/// Generate the JSON Schema for `lintel-catalog.toml` as a `serde_json::Value`.
///
/// # Panics
///
/// Panics if the schema cannot be serialized to JSON (should never happen).
pub fn schema() -> Value {
    serde_json::to_value(schema_for!(CatalogConfig)).expect("schema serialization cannot fail")
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
    fn parse_full_config_kebab_case() {
        let toml = r#"
[catalog]

[target.local]
type = "dir"
dir = "../catalog-generated"
base-url = "https://raw.githubusercontent.com/lintel-rs/catalog/master/"

[target.pages]
type = "github-pages"
base-url = "https://catalog.lintel.tools/"
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
    fn base_url_snake_case_alias_accepted() {
        let toml = r#"
[catalog]

[target.local]
type = "dir"
dir = "out"
base_url = "https://example.com/"
"#;
        let config = load_config(toml).expect("snake_case base_url should be accepted");
        match &config.target["local"] {
            TargetConfig::Dir { base_url, .. } => {
                assert_eq!(base_url, "https://example.com/");
            }
            TargetConfig::GitHubPages { .. } => panic!("expected Dir target"),
        }
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
base-url = "https://example.github.io/catalog/"
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
