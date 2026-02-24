/// Extract the `title` and `description` from a JSON Schema string.
///
/// Returns `(title, description)` — either or both may be `None` if the schema
/// doesn't contain the corresponding top-level property or isn't valid JSON.
pub(super) fn extract_schema_meta(text: &str) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return (None, None);
    };
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(String::from);
    let description = value
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);
    (title, description)
}

/// Convert a key like `"github"` to title case (`"Github"`).
pub(super) fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Simple slugification for fallback filenames.
pub(super) fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let mut result = String::new();
    let mut prev_hyphen = true;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_simple() {
        assert_eq!(slugify("GitHub Workflow"), "github-workflow");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("foo/bar (baz)"), "foo-bar-baz");
    }
}
