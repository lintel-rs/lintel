use miette::NamedSource;
use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

pub struct Json5Parser;

impl Parser for Json5Parser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        ::json5::from_str(content).map_err(|e| {
            let offset = e
                .position()
                .map(|pos| super::line_col_to_offset(content, pos.line + 1, pos.column + 1))
                .unwrap_or(0);
            ParseDiagnostic {
                src: NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: e.to_string(),
            }
        })
    }
}
