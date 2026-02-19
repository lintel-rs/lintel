use miette::NamedSource;
use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

pub struct TomlParser;

impl Parser for TomlParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        let toml_value: toml::Value = toml::from_str(content).map_err(|e| {
            let offset = e.span().map_or(0, |s| s.start);
            ParseDiagnostic {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.message().to_string(),
            }
        })?;
        serde_json::to_value(toml_value).map_err(|e| ParseDiagnostic {
            src: NamedSource::new(file_name, content.to_string()),
            span: 0.into(),
            message: e.to_string(),
        })
    }

    fn extract_schema_uri(&self, content: &str, _value: &Value) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !trimmed.starts_with('#') {
                break;
            }
            let after_hash = trimmed.strip_prefix('#')?.trim();
            if let Some(uri) = after_hash.strip_prefix("$schema:") {
                let uri = uri.trim();
                if !uri.is_empty() {
                    return Some(uri.to_string());
                }
            }
        }
        None
    }
}
