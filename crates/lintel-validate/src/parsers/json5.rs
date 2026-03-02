use miette::NamedSource;
use serde_json::Value;

use lintel_diagnostics::LintelDiagnostic;

use super::Parser;

pub struct Json5Parser;

impl Parser for Json5Parser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, LintelDiagnostic> {
        ::json5::from_str(content).map_err(|e| {
            let offset = e.position().map_or(0, |pos| {
                super::line_col_to_offset(content, pos.line + 1, pos.column + 1)
            });
            LintelDiagnostic::Parse {
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
