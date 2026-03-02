use miette::NamedSource;
use serde_json::Value;

use lintel_diagnostics::LintelDiagnostic;

use super::Parser;

pub struct TomlParser;

impl Parser for TomlParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, LintelDiagnostic> {
        let toml_value: toml::Value = toml::from_str(content).map_err(|e| {
            let offset = e.span().map_or(0, |s| s.start);
            LintelDiagnostic::Parse {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.message().to_string(),
            }
        })?;
        serde_json::to_value(toml_value).map_err(|e| LintelDiagnostic::Parse {
            src: NamedSource::new(file_name, content.to_string()),
            span: 0.into(),
            message: e.to_string(),
        })
    }

    fn annotate(&self, content: &str, schema_url: &str) -> Option<String> {
        Some(format!("# :schema {schema_url}\n{content}"))
    }

    fn strip_annotation(&self, content: &str) -> String {
        let mut offset = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                offset += line.len() + 1;
                continue;
            }
            if !trimmed.starts_with('#') {
                break;
            }
            if let Some(after_hash) = trimmed.strip_prefix('#') {
                let after_hash = after_hash.trim();
                let is_schema = after_hash
                    .strip_prefix(":schema")
                    .is_some_and(|rest| !rest.is_empty())
                    || after_hash
                        .strip_prefix("$schema:")
                        .is_some_and(|rest| !rest.is_empty());
                if is_schema {
                    let line_end = offset + line.len();
                    let remove_end = if content.as_bytes().get(line_end) == Some(&b'\n') {
                        line_end + 1
                    } else {
                        line_end
                    };
                    return format!("{}{}", &content[..offset], &content[remove_end..]);
                }
            }
            offset += line.len() + 1;
        }
        content.to_string()
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
            // Taplo / Even Better TOML convention: # :schema URL
            if let Some(uri) = after_hash.strip_prefix(":schema") {
                let uri = uri.trim();
                if !uri.is_empty() {
                    return Some(uri.to_string());
                }
            }
            // Legacy convention: # $schema: URL
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
