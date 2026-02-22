/// Convert a `JSONPath` expression or JSON Pointer into a schema-level JSON Pointer.
///
/// Supported inputs:
/// - JSON Pointer: `/properties/name` → returned as-is
/// - `JSONPath` property access: `$.name.age` → `/properties/name/properties/age`
/// - `JSONPath` bracket notation: `$["name"]` → `/properties/name`
/// - `JSONPath` array index: `$.items[0]` → `/properties/items/items`
///
/// For array indices, we navigate to the schema's `items` sub-schema rather than
/// a specific index, since JSON Schema describes the shape of all array items.
pub fn to_schema_pointer(path: &str) -> Result<String, String> {
    // Already a JSON Pointer
    if path.starts_with('/') {
        return Ok(path.to_string());
    }

    // Must start with $ for JSONPath
    let rest = path.strip_prefix('$').ok_or_else(|| {
        format!("expected a JSON Pointer (/...) or JSONPath ($...), got '{path}'")
    })?;

    if rest.is_empty() {
        return Ok(String::new());
    }

    let mut pointer = String::new();
    let mut chars = rest.chars().peekable();

    while chars.peek().is_some() {
        match chars.peek() {
            Some('.') => {
                chars.next(); // consume '.'
                let segment = consume_identifier(&mut chars);
                if segment.is_empty() {
                    return Err(format!("empty property name in path '{path}'"));
                }
                pointer.push_str("/properties/");
                pointer.push_str(&segment);
            }
            Some('[') => {
                chars.next(); // consume '['
                if chars.peek() == Some(&'"') || chars.peek() == Some(&'\'') {
                    // Bracket string notation: ["name"] or ['name']
                    let quote = chars.next().expect("checked peek");
                    let segment = consume_until(&mut chars, quote);
                    if chars.next() != Some(']') {
                        return Err(format!("missing closing ']' in path '{path}'"));
                    }
                    pointer.push_str("/properties/");
                    pointer.push_str(&segment);
                } else {
                    // Array index: [0]
                    let _index = consume_until(&mut chars, ']');
                    pointer.push_str("/items");
                }
            }
            Some(c) => {
                return Err(format!("unexpected character '{c}' in path '{path}'"));
            }
            None => break,
        }
    }

    Ok(pointer)
}

fn consume_identifier(chars: &mut core::iter::Peekable<core::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c == '.' || c == '[' {
            break;
        }
        s.push(c);
        chars.next();
    }
    s
}

fn consume_until(chars: &mut core::iter::Peekable<core::str::Chars<'_>>, end: char) -> String {
    let mut s = String::new();
    for c in chars.by_ref() {
        if c == end {
            break;
        }
        s.push(c);
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn json_pointer_passthrough() {
        assert_eq!(
            to_schema_pointer("/properties/name").unwrap(),
            "/properties/name"
        );
    }

    #[test]
    fn root_jsonpath() {
        assert_eq!(to_schema_pointer("$").unwrap(), "");
    }

    #[test]
    fn simple_property() {
        assert_eq!(to_schema_pointer("$.name").unwrap(), "/properties/name");
    }

    #[test]
    fn nested_properties() {
        assert_eq!(
            to_schema_pointer("$.config.debug").unwrap(),
            "/properties/config/properties/debug"
        );
    }

    #[test]
    fn bracket_notation() {
        assert_eq!(
            to_schema_pointer("$[\"name\"]").unwrap(),
            "/properties/name"
        );
    }

    #[test]
    fn array_index_becomes_items() {
        assert_eq!(
            to_schema_pointer("$.items[0]").unwrap(),
            "/properties/items/items"
        );
    }

    #[test]
    fn mixed_access() {
        assert_eq!(
            to_schema_pointer("$.jobs[0].name").unwrap(),
            "/properties/jobs/items/properties/name"
        );
    }

    #[test]
    fn invalid_prefix() {
        assert!(to_schema_pointer("foo.bar").is_err());
    }

    #[test]
    fn single_quote_bracket() {
        assert_eq!(to_schema_pointer("$['name']").unwrap(), "/properties/name");
    }

    #[test]
    fn deeply_nested_jsonpath() {
        assert_eq!(
            to_schema_pointer("$.a.b.c.d").unwrap(),
            "/properties/a/properties/b/properties/c/properties/d"
        );
    }

    #[test]
    fn multiple_array_indices() {
        assert_eq!(
            to_schema_pointer("$.matrix[0][1]").unwrap(),
            "/properties/matrix/items/items"
        );
    }

    #[test]
    fn empty_dot_segment_errors() {
        assert!(to_schema_pointer("$..name").is_err());
    }

    #[test]
    fn pointer_with_leading_slash() {
        assert_eq!(to_schema_pointer("/a/b/c").unwrap(), "/a/b/c");
    }

    #[test]
    fn bracket_then_dot() {
        assert_eq!(
            to_schema_pointer("$[\"config\"].debug").unwrap(),
            "/properties/config/properties/debug"
        );
    }

    #[test]
    fn dollar_only_is_root() {
        assert_eq!(to_schema_pointer("$").unwrap(), "");
    }
}
