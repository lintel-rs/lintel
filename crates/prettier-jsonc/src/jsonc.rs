use anyhow::{Result, anyhow};
use biome_formatter::{IndentStyle, IndentWidth, LineWidth};
use biome_json_formatter::{context::JsonFormatOptions, format_node};
use biome_json_parser::{JsonParserOptions, parse_json};

use crate::PrettierConfig;
use prettier_config::TrailingComma;

/// Format JSONC content using biome's JSON formatter.
///
/// # Errors
///
/// Returns an error if the content is not valid JSONC.
pub fn format_jsonc(content: &str, options: &PrettierConfig) -> Result<String> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    let parse_options = JsonParserOptions::default()
        .with_allow_comments()
        .with_allow_trailing_commas();

    let parsed = parse_json(content, parse_options);

    if parsed.has_errors() {
        let msg = parsed
            .diagnostics()
            .iter()
            .map(|d| format!("{d:?}"))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow!("JSON parse error: {msg}"));
    }

    let trailing = match options.trailing_comma {
        TrailingComma::All | TrailingComma::Es5 => {
            biome_json_formatter::context::TrailingCommas::All
        }
        TrailingComma::None => biome_json_formatter::context::TrailingCommas::None,
    };

    let indent_style = if options.use_tabs {
        IndentStyle::Tab
    } else {
        IndentStyle::Space
    };

    let format_options = JsonFormatOptions::default()
        .with_indent_style(indent_style)
        .with_indent_width(IndentWidth::from(
            u8::try_from(options.tab_width).unwrap_or(u8::MAX),
        ))
        .with_line_width(
            LineWidth::try_from(u16::try_from(options.print_width).unwrap_or(u16::MAX))
                .unwrap_or_default(),
        )
        .with_trailing_commas(trailing);

    let formatted =
        format_node(format_options, &parsed.syntax()).map_err(|e| anyhow!("format error: {e}"))?;

    let printed = formatted.print().map_err(|e| anyhow!("print error: {e}"))?;

    Ok(printed.into_code())
}
