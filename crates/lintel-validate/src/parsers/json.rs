use miette::NamedSource;
use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

pub struct JsonParser;

impl Parser for JsonParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        serde_json::from_str(content).map_err(|e| {
            let offset = super::line_col_to_offset(content, e.line(), e.column());
            ParseDiagnostic {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.to_string(),
            }
        })
    }

    fn annotate(&self, content: &str, schema_url: &str) -> Option<String> {
        Some(super::annotate_json_content(content, schema_url))
    }

    fn strip_annotation(&self, content: &str) -> String {
        super::strip_json_schema_property(content)
    }
}
