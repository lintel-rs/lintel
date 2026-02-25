#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

pub mod json;
pub mod markdown;
pub mod toml;
pub mod typescript;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Example helpers (for schemars annotations)
// ---------------------------------------------------------------------------

fn example_plugin_urls() -> Vec<String> {
    vec![
        "https://plugins.dprint.dev/typescript-0.93.3.wasm".into(),
        "https://plugins.dprint.dev/json-0.19.4.wasm".into(),
    ]
}

fn example_includes() -> Vec<String> {
    vec!["src/**/*.{ts,tsx,json}".into()]
}

fn example_excludes() -> Vec<String> {
    vec!["**/*-lock.json".into(), "**/node_modules".into()]
}

fn example_associations() -> Vec<String> {
    vec!["**/*.myconfig".into(), ".myconfigrc".into()]
}

// ---------------------------------------------------------------------------
// Extends
// ---------------------------------------------------------------------------

/// One or more configuration files to extend.
///
/// Can be a single path/URL string or an array of paths/URLs. Properties
/// from extended configs are merged, with the current file taking
/// precedence. Earlier entries in an array take priority over later ones.
///
/// Supports the `${configDir}` variable (resolves to the directory
/// containing the current config file) and `${originConfigDir}` (resolves
/// to the directory of the original/root config file).
///
/// When extending remote configs (URLs), `includes` and non-Wasm `plugins`
/// entries are ignored for security.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Extends {
    /// A single file path or URL to a configuration file to extend.
    Single(String),
    /// A collection of file paths and/or URLs to configuration files to
    /// extend.
    Multiple(Vec<String>),
}

// ---------------------------------------------------------------------------
// NewLineKind
// ---------------------------------------------------------------------------

/// The newline character style to use when formatting files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(title = "New Line Kind")]
pub enum NewLineKind {
    /// For each file, uses the newline kind found at the end of the last
    /// line.
    Auto,
    /// Uses carriage return followed by line feed (`\r\n`).
    Crlf,
    /// Uses line feed (`\n`).
    Lf,
    /// Uses the system standard (e.g., CRLF on Windows, LF on
    /// macOS/Linux).
    System,
}

// ---------------------------------------------------------------------------
// PluginConfig (generic, for unknown plugins)
// ---------------------------------------------------------------------------

/// Configuration for a dprint plugin not covered by the typed plugin
/// structs.
///
/// Known plugins (`typescript`, `json`, `toml`, `markdown`) have dedicated
/// typed structs. This catch-all is used for any other plugin name.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "Plugin Configuration")]
pub struct PluginConfig {
    /// Prevent properties in this plugin section from being overridden by
    /// extended configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Locked")]
    pub locked: Option<bool>,

    /// File patterns to associate with this plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "File Associations", example = example_associations())]
    pub associations: Option<Vec<String>>,

    /// Plugin-specific configuration settings.
    #[serde(flatten)]
    pub settings: BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
// DprintConfig
// ---------------------------------------------------------------------------

/// dprint configuration file (`dprint.json` / `.dprint.json`).
///
/// Configuration for the [dprint](https://dprint.dev) code formatter.
/// The config file is typically named `dprint.json`, `.dprint.json`,
/// `dprint.jsonc`, or `.dprint.jsonc` and placed at the root of your
/// project.
///
/// Global formatting options ([`lineWidth`](DprintConfig::line_width),
/// [`indentWidth`](DprintConfig::indent_width),
/// [`useTabs`](DprintConfig::use_tabs),
/// [`newLineKind`](DprintConfig::new_line_kind)) apply as defaults to all
/// plugins but can be overridden in per-plugin configuration sections.
///
/// Plugin-specific configuration is placed at the top level, keyed by the
/// plugin name:
///
/// ```json
/// {
///   "lineWidth": 80,
///   "typescript": {
///     "quoteStyle": "preferSingle"
///   },
///   "json": {
///     "indentWidth": 2
///   },
///   "plugins": [
///     "https://plugins.dprint.dev/typescript-0.93.3.wasm",
///     "https://plugins.dprint.dev/json-0.19.4.wasm"
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(
    title = "dprint configuration file",
    description = "Schema for a dprint configuration file."
)]
pub struct DprintConfig {
    /// The JSON schema URL for editor validation and autocompletion.
    ///
    /// The dprint VS Code extension automatically constructs a composite
    /// schema based on the installed plugins, so you usually don't need
    /// to set this manually.
    #[serde(default, rename = "$schema", skip_serializing_if = "Option::is_none")]
    #[schemars(title = "JSON Schema")]
    pub schema: Option<String>,

    /// Only format files that have changed since the last formatting run.
    ///
    /// When `true` (the default), dprint tracks which files have been
    /// formatted and skips unchanged files on subsequent runs. Set to
    /// `false` to always reformat all matched files.
    ///
    /// Can also be toggled via the `--incremental` / `--incremental=false`
    /// CLI flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Incremental")]
    pub incremental: Option<bool>,

    /// One or more configuration files to extend.
    ///
    /// Can be a single path/URL or an array of paths/URLs. Properties from
    /// extended configs are merged, with the current file taking
    /// precedence. Earlier entries in an array take priority over later
    /// ones.
    ///
    /// Supports the `${configDir}` variable (resolves to the directory of
    /// the current config file) and `${originConfigDir}` (resolves to the
    /// directory of the original/root config file).
    ///
    /// When extending remote configs (URLs), `includes` and non-Wasm
    /// `plugins` entries are ignored for security.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Extends")]
    pub extends: Option<Extends>,

    /// The maximum line width the formatter tries to stay under.
    ///
    /// The formatter may exceed this width in certain cases where breaking
    /// the line would produce worse output. This is a global default that
    /// individual plugins can override in their configuration sections.
    #[serde(
        default,
        rename = "lineWidth",
        alias = "line-width",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(title = "Line Width")]
    pub line_width: Option<u32>,

    /// The number of characters for each indent level.
    ///
    /// When `useTabs` is `true`, this controls the visual width of each
    /// tab character. When `useTabs` is `false`, this is the number of
    /// spaces inserted per indent level. This is a global default that
    /// individual plugins can override in their configuration sections.
    #[serde(
        default,
        rename = "indentWidth",
        alias = "indent-width",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(title = "Indent Width")]
    pub indent_width: Option<u32>,

    /// Whether to use tabs (`true`) or spaces (`false`) for indentation.
    ///
    /// This is a global default that individual plugins can override in
    /// their configuration sections.
    #[serde(
        default,
        rename = "useTabs",
        alias = "use-tabs",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(title = "Use Tabs")]
    pub use_tabs: Option<bool>,

    /// The newline character style to use when formatting.
    ///
    /// This is a global default that individual plugins can override in
    /// their configuration sections.
    #[serde(
        default,
        rename = "newLineKind",
        alias = "new-line-kind",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(title = "New Line Kind")]
    pub new_line_kind: Option<NewLineKind>,

    /// Glob patterns for files to include in formatting.
    ///
    /// When specified, only files matching at least one of these patterns
    /// are formatted. When omitted, all files matched by installed plugins
    /// are formatted (respecting `excludes`).
    ///
    /// Uses gitignore-style glob syntax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Includes", example = example_includes())]
    pub includes: Option<Vec<String>>,

    /// Glob patterns for files or directories to exclude from formatting.
    ///
    /// Uses gitignore-style glob syntax. Files already ignored by
    /// `.gitignore` are excluded automatically. Prefix a pattern with `!`
    /// to un-exclude files that were previously excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Excludes", example = example_excludes())]
    pub excludes: Option<Vec<String>>,

    /// Plugin URLs or local file paths.
    ///
    /// Each entry is a URL to a WebAssembly plugin (`.wasm`) or a local
    /// file path. The order determines precedence when multiple plugins
    /// can handle the same file extension — the first matching plugin
    /// wins.
    ///
    /// Can also be specified via the `--plugins` CLI flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(title = "Plugins", example = example_plugin_urls())]
    pub plugins: Option<Vec<String>>,

    // ----- Known plugin configurations -----
    /// TypeScript / JavaScript plugin configuration.
    ///
    /// See: <https://dprint.dev/plugins/typescript/config/>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typescript: Option<typescript::TypeScriptConfig>,

    /// JSON plugin configuration.
    ///
    /// See: <https://dprint.dev/plugins/json/config/>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<json::JsonConfig>,

    /// TOML plugin configuration.
    ///
    /// See: <https://dprint.dev/plugins/toml/config/>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toml: Option<toml::TomlConfig>,

    /// Markdown plugin configuration.
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<markdown::MarkdownConfig>,

    // ----- Unknown plugins (catch-all) -----
    /// Configuration for plugins not covered by the typed fields above.
    #[serde(flatten)]
    pub plugin_configs: BTreeMap<String, PluginConfig>,
}

/// Generate the JSON Schema for [`DprintConfig`] as a
/// [`serde_json::Value`].
///
/// # Panics
///
/// Panics if the schema cannot be serialized to JSON (should never happen).
pub fn schema() -> Value {
    serde_json::to_value(schema_for!(DprintConfig)).expect("schema serialization cannot fail")
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn deserialize_minimal_config() {
        let json = r#"{"plugins":["https://plugins.dprint.dev/typescript-0.93.3.wasm"]}"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(config.plugins.as_ref().expect("plugins").len(), 1);
    }

    #[test]
    fn deserialize_typed_typescript_config() {
        let json = r#"{
            "typescript": {
                "quoteStyle": "preferSingle",
                "semiColons": "asi",
                "lineWidth": 100,
                "locked": true,
                "associations": ["!**/*.js"]
            }
        }"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        let ts = config.typescript.as_ref().expect("typescript config");
        assert_eq!(ts.locked, Some(true));
        assert_eq!(ts.quote_style, Some(typescript::QuoteStyle::PreferSingle));
        assert_eq!(ts.semi_colons, Some(typescript::SemiColons::Asi));
        assert_eq!(ts.line_width, Some(100));
    }

    #[test]
    fn deserialize_typed_json_config() {
        let json = r#"{
            "json": {
                "indentWidth": 4,
                "trailingCommas": "never"
            }
        }"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        let j = config.json.as_ref().expect("json config");
        assert_eq!(j.indent_width, Some(4));
        assert_eq!(j.trailing_commas, Some(json::TrailingCommas::Never));
    }

    #[test]
    fn deserialize_typed_toml_config() {
        let json = r#"{
            "toml": {
                "cargo.applyConventions": false
            }
        }"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        let t = config.toml.as_ref().expect("toml config");
        assert_eq!(t.cargo_apply_conventions, Some(false));
    }

    #[test]
    fn deserialize_typed_markdown_config() {
        let json = r#"{
            "markdown": {
                "textWrap": "always",
                "emphasisKind": "asterisks"
            }
        }"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        let md = config.markdown.as_ref().expect("markdown config");
        assert_eq!(md.text_wrap, Some(markdown::TextWrap::Always));
        assert_eq!(md.emphasis_kind, Some(markdown::StrongKind::Asterisks));
    }

    #[test]
    fn unknown_plugin_falls_through() {
        let json = r#"{
            "prettier": {
                "tabWidth": 4
            }
        }"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        assert!(config.typescript.is_none());
        let prettier = config.plugin_configs.get("prettier").expect("prettier");
        assert_eq!(
            prettier.settings.get("tabWidth"),
            Some(&serde_json::json!(4))
        );
    }

    #[test]
    fn extends_single_string() {
        let json = r#"{"extends": "base.json"}"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        assert!(matches!(config.extends, Some(Extends::Single(ref s)) if s == "base.json"));
    }

    #[test]
    fn extends_multiple_strings() {
        let json = r#"{"extends": ["a.json", "b.json"]}"#;
        let config: DprintConfig = serde_json::from_str(json).expect("parse");
        assert!(matches!(config.extends, Some(Extends::Multiple(ref v)) if v.len() == 2));
    }

    #[test]
    fn new_line_kind_values() {
        for (input, expected) in [
            ("\"auto\"", NewLineKind::Auto),
            ("\"crlf\"", NewLineKind::Crlf),
            ("\"lf\"", NewLineKind::Lf),
            ("\"system\"", NewLineKind::System),
        ] {
            let parsed: NewLineKind = serde_json::from_str(input).expect(input);
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn round_trip_config() {
        let config = DprintConfig {
            schema: None,
            incremental: Some(true),
            extends: Some(Extends::Single("base.json".to_string())),
            line_width: Some(80),
            indent_width: Some(2),
            use_tabs: Some(false),
            new_line_kind: Some(NewLineKind::Lf),
            includes: Some(vec!["src/**/*.ts".to_string()]),
            excludes: Some(vec!["**/*-lock.json".to_string()]),
            plugins: Some(vec![
                "https://plugins.dprint.dev/typescript-0.93.3.wasm".to_string(),
            ]),
            typescript: None,
            json: None,
            toml: None,
            markdown: None,
            plugin_configs: BTreeMap::new(),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: DprintConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn schema_has_expected_properties() {
        let s = schema();
        let text = serde_json::to_string(&s).expect("serialize");
        for prop in [
            "lineWidth",
            "indentWidth",
            "useTabs",
            "newLineKind",
            "plugins",
            "includes",
            "excludes",
            "incremental",
            "extends",
            "$schema",
            "typescript",
            "json",
            "toml",
            "markdown",
        ] {
            assert!(text.contains(prop), "schema should contain {prop}");
        }
    }

    #[test]
    fn empty_config_deserializes() {
        let config: DprintConfig = serde_json::from_str("{}").expect("parse");
        assert!(config.plugins.is_none());
        assert!(config.line_width.is_none());
        assert!(config.typescript.is_none());
        assert!(config.plugin_configs.is_empty());
    }
}
