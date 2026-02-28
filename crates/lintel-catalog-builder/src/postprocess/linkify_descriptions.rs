/// Wrap bare `http://` and `https://` URLs in Markdown angle-bracket autolinks.
///
/// Already-wrapped URLs (preceded by `<` or `(`) are left unchanged.
/// Trailing sentence punctuation (`.`, `,`, `;`) is kept outside the link.
fn linkify_urls(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last = 0;

    for (start, _) in text.match_indices("http") {
        let remaining = &text[start..];
        if !remaining.starts_with("http://") && !remaining.starts_with("https://") {
            continue;
        }
        // Skip if we've already processed past this position
        if start < last {
            continue;
        }
        // Already wrapped in <…> or (…)
        if start > 0 && matches!(text.as_bytes()[start - 1], b'<' | b'(') {
            continue;
        }

        let url_end = remaining
            .find(|c: char| c.is_whitespace() || matches!(c, '>' | '"' | '\'' | '`'))
            .map_or(text.len(), |e| start + e);

        let raw = &text[start..url_end];
        let trimmed = raw.trim_end_matches(['.', ',', ';']);

        result.push_str(&text[last..start]);
        result.push('<');
        result.push_str(trimmed);
        result.push('>');
        // Re-append any stripped trailing punctuation
        if trimmed.len() < raw.len() {
            result.push_str(&raw[trimmed.len()..]);
        }
        last = url_end;
    }

    result.push_str(&text[last..]);
    result
}

/// Recursively transform all `description` fields in a JSON value so that
/// bare URLs become Markdown angle-bracket autolinks (`<https://…>`).
pub(super) fn linkify_descriptions(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(desc)) = map.get("description") {
                let linkified = linkify_urls(desc);
                if linkified != *desc {
                    map.insert(
                        "description".to_string(),
                        serde_json::Value::String(linkified),
                    );
                }
            }
            for v in map.values_mut() {
                linkify_descriptions(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                linkify_descriptions(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_https() {
        assert_eq!(
            linkify_urls("See https://example.com for details"),
            "See <https://example.com> for details"
        );
    }

    #[test]
    fn bare_http() {
        assert_eq!(
            linkify_urls("Visit http://example.com/path"),
            "Visit <http://example.com/path>"
        );
    }

    #[test]
    fn trailing_punctuation() {
        assert_eq!(
            linkify_urls("See https://example.com."),
            "See <https://example.com>."
        );
        assert_eq!(
            linkify_urls("See https://example.com, or not"),
            "See <https://example.com>, or not"
        );
    }

    #[test]
    fn already_angle_bracketed() {
        assert_eq!(
            linkify_urls("See <https://example.com> for details"),
            "See <https://example.com> for details"
        );
    }

    #[test]
    fn already_in_parens() {
        assert_eq!(
            linkify_urls("Link (https://example.com) here"),
            "Link (https://example.com) here"
        );
    }

    #[test]
    fn multiple_urls() {
        assert_eq!(
            linkify_urls("See https://a.com and https://b.com"),
            "See <https://a.com> and <https://b.com>"
        );
    }

    #[test]
    fn no_urls() {
        assert_eq!(linkify_urls("no links here"), "no links here");
    }

    #[test]
    fn url_with_path_and_query() {
        assert_eq!(
            linkify_urls("Go to https://example.com/path?q=1&r=2#frag now"),
            "Go to <https://example.com/path?q=1&r=2#frag> now"
        );
    }

    #[test]
    fn recursive() {
        let mut schema = serde_json::json!({
            "description": "See https://example.com for details",
            "properties": {
                "foo": {
                    "description": "Docs at https://docs.rs/foo",
                    "type": "string"
                }
            }
        });
        linkify_descriptions(&mut schema);
        assert_eq!(
            schema["description"],
            "See <https://example.com> for details"
        );
        assert_eq!(
            schema["properties"]["foo"]["description"],
            "Docs at <https://docs.rs/foo>"
        );
    }
}
