use alloc::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// [IntelliJ IDEA] JSON Schema extensions (`x-intellij-*`).
///
/// IntelliJ-based IDEs recognise these vendor extensions on any JSON Schema
/// property to enhance editing, completion, and validation UX. Each key is a
/// separate top-level property on the schema object, so this struct is
/// [`#[serde(flatten)]`](https://serde.rs/attr-flatten.html)-ed into
/// [`Schema`](crate::Schema).
///
/// # Keys
///
/// | JSON key | Rust field | Purpose |
/// |---|---|---|
/// | `x-intellij-html-description` | [`html_description`](Self::html_description) | Rich HTML description for hover/docs |
/// | `x-intellij-language-injection` | [`language_injection`](Self::language_injection) | Language ID for editor injection |
/// | `x-intellij-enum-metadata` | [`enum_metadata`](Self::enum_metadata) | Per-enum-value descriptions |
///
/// # Example
///
/// ```json
/// {
///   "type": "string",
///   "x-intellij-html-description": "<b>Greeting</b> message",
///   "x-intellij-language-injection": "Shell Script",
///   "x-intellij-enum-metadata": {
///     "system": { "description": "Use system default" }
///   }
/// }
/// ```
///
/// [IntelliJ IDEA]: https://www.jetbrains.com/help/idea/json.html
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntellijSchemaExt {
    /// Rich HTML description shown in editor hover popups and documentation
    /// panels.
    ///
    /// Analogous to the [VSCode] [`markdownDescription`] extension but uses
    /// HTML markup instead of Markdown.
    ///
    /// [VSCode]: https://code.visualstudio.com/docs/languages/json
    /// [`markdownDescription`]: https://code.visualstudio.com/docs/languages/json#_use-rich-formatting-in-hovers
    #[serde(
        rename = "x-intellij-html-description",
        skip_serializing_if = "Option::is_none"
    )]
    pub html_description: Option<String>,

    /// Language identifier for [language injection].
    ///
    /// When set on a string-typed property, [IntelliJ IDEA] injects syntax
    /// highlighting and code intelligence for the named language
    /// (e.g. `"Shell Script"`, `"RegExp"`, `"ini"`).
    ///
    /// [IntelliJ IDEA]: https://www.jetbrains.com/help/idea/json.html
    /// [language injection]: https://www.jetbrains.com/help/idea/language-injections-settings.html
    #[serde(
        rename = "x-intellij-language-injection",
        skip_serializing_if = "Option::is_none"
    )]
    pub language_injection: Option<String>,

    /// Per-enum-value metadata, keyed by the enum value string.
    ///
    /// Provides additional descriptions for each `enum` value that
    /// [IntelliJ IDEA](https://www.jetbrains.com/help/idea/json.html)
    /// surfaces in completion popups and documentation panels.
    #[serde(
        rename = "x-intellij-enum-metadata",
        skip_serializing_if = "Option::is_none"
    )]
    pub enum_metadata: Option<BTreeMap<String, EnumValueMeta>>,
}

/// Metadata for a single enum value in
/// [`x-intellij-enum-metadata`](IntellijSchemaExt::enum_metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValueMeta {
    /// Human-readable description of what this enum value means.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
