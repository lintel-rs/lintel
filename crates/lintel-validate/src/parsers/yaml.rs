use miette::NamedSource;
use serde_json::Value;

use lintel_diagnostics::LintelDiagnostic;

use super::Parser;

pub struct YamlParser;

impl Parser for YamlParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, LintelDiagnostic> {
        // Strip UTF-8 BOM characters that can appear at the start of a file or
        // mid-stream (e.g. after a comment line), which serde_yaml misinterprets
        // as a multi-document separator.
        let clean: alloc::borrow::Cow<'_, str> = if content.contains('\u{FEFF}') {
            content.replace('\u{FEFF}', "").into()
        } else {
            content.into()
        };
        serde_yaml::from_str(&clean).map_err(|e| {
            let offset = e.location().map_or(0, |loc| loc.index());
            LintelDiagnostic::Parse {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.to_string(),
            }
        })
    }

    fn annotate(&self, content: &str, schema_url: &str) -> Option<String> {
        Some(format!(
            "# yaml-language-server: $schema={schema_url}\n{content}"
        ))
    }

    fn strip_annotation(&self, content: &str) -> String {
        strip_yaml_modeline(content)
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

/// Remove the `# yaml-language-server: $schema=URL` modeline from leading comments.
fn strip_yaml_modeline(content: &str) -> String {
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
            if let Some(rest) = after_hash.strip_prefix("yaml-language-server:")
                && rest.trim().starts_with("$schema=")
            {
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
