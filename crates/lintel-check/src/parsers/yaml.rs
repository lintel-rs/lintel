use miette::NamedSource;
use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

pub struct YamlParser;

impl Parser for YamlParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        // Strip UTF-8 BOM characters that can appear at the start of a file or
        // mid-stream (e.g. after a comment line), which serde_yaml misinterprets
        // as a multi-document separator.
        let clean: std::borrow::Cow<'_, str> = if content.contains('\u{FEFF}') {
            content.replace('\u{FEFF}', "").into()
        } else {
            content.into()
        };
        serde_yaml::from_str(&clean).map_err(|e| {
            let offset = e.location().map(|loc| loc.index()).unwrap_or(0);
            ParseDiagnostic {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.to_string(),
            }
        })
    }

    fn extract_schema_uri(&self, content: &str, value: &Value) -> Option<String> {
        // First check for yaml-language-server modeline in leading comments
        if let Some(uri) = extract_yaml_modeline_schema(content) {
            return Some(uri);
        }
        // Fall back to top-level $schema property
        value
            .get("$schema")
            .and_then(Value::as_str)
            .map(String::from)
    }
}

/// Extract schema URI from `# yaml-language-server: $schema=URL` comment.
fn extract_yaml_modeline_schema(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('#') {
            break;
        }
        let after_hash = trimmed.strip_prefix('#')?.trim();
        if let Some(rest) = after_hash.strip_prefix("yaml-language-server:") {
            let rest = rest.trim();
            if let Some(uri) = rest.strip_prefix("$schema=") {
                let uri = uri.trim();
                if !uri.is_empty() {
                    return Some(uri.to_string());
                }
            }
        }
    }
    None
}
