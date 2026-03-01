use miette::NamedSource;
use serde_json::Value;

use lintel_diagnostics::LintelDiagnostic;

use super::Parser;

pub struct JsoncParser;

impl Parser for JsoncParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, LintelDiagnostic> {
        let opts = jsonc_parser::ParseOptions {
            allow_comments: true,
            allow_loose_object_property_names: false,
            allow_trailing_commas: true,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_missing_commas: false,
            allow_unary_plus_numbers: false,
        };
        jsonc_parser::parse_to_serde_value(content, &opts)
            .map_err(|e| {
                let range = e.range();
                LintelDiagnostic::Parse {
                    src: NamedSource::new(file_name, content.to_string()),
                    span: (range.start, range.end - range.start).into(),
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| LintelDiagnostic::Parse {
                src: NamedSource::new(file_name, content.to_string()),
                span: 0.into(),
                message: "empty JSONC document".to_string(),
            })
    }

    fn annotate(&self, content: &str, schema_url: &str) -> Option<String> {
        Some(super::annotate_json_content(content, schema_url))
    }

    fn strip_annotation(&self, content: &str) -> String {
        super::strip_json_schema_property(content)
    }
}
