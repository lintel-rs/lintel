//! "Did you mean?" suggestions for `additionalProperties` validation errors.
//!
//! When a JSON Schema validation error reports an unexpected property, this
//! module finds close matches from the schema's valid properties and appends
//! a suggestion to the error message.

use core::fmt::Write;

use serde_json::Value;

/// Standard Levenshtein edit distance on Unicode characters.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        core::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// The prefix that `jsonschema` uses for additionalProperties errors.
const ADDITIONAL_PROPS_PREFIX: &str = "Additional properties are not allowed (";

/// Parse property names from an additionalProperties error message.
///
/// Messages look like:
/// `Additional properties are not allowed ('foo', 'bar' were unexpected)`
fn extract_unexpected_properties(message: &str) -> Option<Vec<String>> {
    if !message.starts_with(ADDITIONAL_PROPS_PREFIX) {
        return None;
    }

    let mut props = Vec::new();
    let mut remaining = &message[ADDITIONAL_PROPS_PREFIX.len()..];

    while let Some(start) = remaining.find('\'') {
        let after_quote = &remaining[start + 1..];
        if let Some(end) = after_quote.find('\'') {
            props.push(after_quote[..end].to_string());
            remaining = &after_quote[end + 1..];
        } else {
            break;
        }
    }

    if props.is_empty() { None } else { Some(props) }
}

/// Resolve a local `$ref` (starting with `#/`) within the schema document.
fn resolve_ref<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str)
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        let mut current = root;
        for segment in path.split('/') {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            match current {
                Value::Object(map) => {
                    if let Some(next) = map.get(&decoded) {
                        current = next;
                    } else {
                        return schema;
                    }
                }
                _ => return schema,
            }
        }
        return current;
    }
    schema
}

/// Navigate a JSON Pointer path through a schema value.
fn navigate_pointer<'a>(schema: &'a Value, pointer: &str) -> Option<&'a Value> {
    let path = pointer.strip_prefix('/').unwrap_or(pointer);
    if path.is_empty() {
        return Some(schema);
    }

    let mut current = schema;
    for segment in path.split('/') {
        let decoded = segment.replace("~1", "/").replace("~0", "~");

        if let Some(next) = current.get(&decoded) {
            current = next;
            continue;
        }

        // Try as array index
        if let Value::Array(arr) = current
            && let Ok(idx) = decoded.parse::<usize>()
            && let Some(next) = arr.get(idx)
        {
            current = next;
            continue;
        }

        return None;
    }

    Some(current)
}

/// Collect valid property names from the schema at the location indicated by
/// `schema_path`.
///
/// The `schema_path` from a validation error typically ends with
/// `/additionalProperties` — we strip that suffix to find the parent object
/// schema, then collect keys from its `properties`, `allOf` entries, and
/// local `$ref` targets.
fn collect_schema_properties(schema: &Value, schema_path: &str) -> Vec<String> {
    // Strip the /additionalProperties suffix to get the parent object path.
    let parent_path = schema_path
        .strip_suffix("/additionalProperties")
        .unwrap_or(schema_path);

    let Some(parent) = navigate_pointer(schema, parent_path) else {
        return Vec::new();
    };

    collect_properties_recursive(parent, schema)
}

/// Recursively collect property names from a schema node, following `$ref` and
/// `allOf`.
fn collect_properties_recursive(node: &Value, root: &Value) -> Vec<String> {
    let resolved = resolve_ref(node, root);
    let mut props = Vec::new();

    // Direct properties
    if let Some(Value::Object(map)) = resolved.get("properties") {
        props.extend(map.keys().cloned());
    }

    // allOf: merge properties from each sub-schema
    if let Some(Value::Array(all_of)) = resolved.get("allOf") {
        for sub in all_of {
            props.extend(collect_properties_recursive(sub, root));
        }
    }

    // patternProperties keys are regex patterns, not property names — skip them.

    props
}

/// Find the best matching property name for an unexpected property.
///
/// Returns `None` if no match is close enough. Uses case-insensitive
/// comparison but returns the correctly-cased schema property.
fn suggest_for_property(unexpected: &str, valid_properties: &[String]) -> Option<String> {
    let unexpected_lower = unexpected.to_lowercase();

    let mut best: Option<(usize, &str)> = None;

    for prop in valid_properties {
        let prop_lower = prop.to_lowercase();
        let dist = levenshtein(&unexpected_lower, &prop_lower);

        // Reject if distance is too large relative to string length.
        // This prevents nonsensical matches for short strings.
        let max_len = unexpected.len().max(prop.len());
        if dist > 3 || dist * 2 > max_len {
            continue;
        }

        match best {
            Some((best_dist, _)) if dist >= best_dist => {}
            _ => best = Some((dist, prop)),
        }
    }

    best.map(|(_, prop)| prop.to_string())
}

/// Enrich an additionalProperties error message with "did you mean?"
/// suggestions.
///
/// If the message is not an additionalProperties error or no close matches are
/// found, returns the message unchanged.
pub fn enrich_message(message: &str, schema_path: &str, schema: &Value) -> String {
    let Some(unexpected) = extract_unexpected_properties(message) else {
        return message.to_string();
    };

    let valid_properties = collect_schema_properties(schema, schema_path);
    if valid_properties.is_empty() {
        return message.to_string();
    }

    let mut suggestions: Vec<(String, String)> = Vec::new();
    for prop in &unexpected {
        if let Some(suggestion) = suggest_for_property(prop, &valid_properties) {
            suggestions.push((prop.clone(), suggestion));
        }
    }

    if suggestions.is_empty() {
        return message.to_string();
    }

    let mut result = message.to_string();
    if suggestions.len() == 1 && unexpected.len() == 1 {
        // Single unexpected property with a suggestion — simple suffix
        let _ = write!(result, "; did you mean '{}'?", suggestions[0].1);
    } else {
        // Multiple unexpected properties or multiple suggestions — annotate each
        for (prop, suggestion) in &suggestions {
            let _ = write!(result, "; did you mean '{suggestion}'? (for '{prop}')");
        }
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- levenshtein ---

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn levenshtein_substitution() {
        assert_eq!(levenshtein("kitten", "sitten"), 1);
    }

    #[test]
    fn levenshtein_insertion_deletion() {
        assert_eq!(levenshtein("abc", "ab"), 1);
        assert_eq!(levenshtein("ab", "abc"), 1);
    }

    #[test]
    fn levenshtein_classic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn levenshtein_dash_vs_underscore() {
        // "argument_hint" vs "argument-hint" — distance 1
        assert_eq!(levenshtein("argument_hint", "argument-hint"), 1);
    }

    // --- extract_unexpected_properties ---

    #[test]
    fn extract_single_property() {
        let msg = "Additional properties are not allowed ('argument_hint' was unexpected)";
        let props = extract_unexpected_properties(msg).unwrap();
        assert_eq!(props, vec!["argument_hint"]);
    }

    #[test]
    fn extract_multiple_properties() {
        let msg = "Additional properties are not allowed ('foo', 'bar', 'baz' were unexpected)";
        let props = extract_unexpected_properties(msg).unwrap();
        assert_eq!(props, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn extract_non_additional_properties_message() {
        let msg = "Some other error message";
        assert!(extract_unexpected_properties(msg).is_none());
    }

    // --- collect_schema_properties ---

    #[test]
    fn collect_from_simple_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "additionalProperties": false
        });
        let mut props = collect_schema_properties(&schema, "/additionalProperties");
        props.sort();
        assert_eq!(props, vec!["age", "name"]);
    }

    #[test]
    fn collect_from_nested_schema() {
        let schema = json!({
            "properties": {
                "config": {
                    "type": "object",
                    "properties": {
                        "debug": { "type": "boolean" },
                        "verbose": { "type": "boolean" }
                    },
                    "additionalProperties": false
                }
            }
        });
        let mut props =
            collect_schema_properties(&schema, "/properties/config/additionalProperties");
        props.sort();
        assert_eq!(props, vec!["debug", "verbose"]);
    }

    #[test]
    fn collect_from_allof() {
        let schema = json!({
            "allOf": [
                {
                    "properties": {
                        "alpha": { "type": "string" }
                    }
                },
                {
                    "properties": {
                        "beta": { "type": "string" }
                    }
                }
            ],
            "additionalProperties": false
        });
        let mut props = collect_schema_properties(&schema, "/additionalProperties");
        props.sort();
        assert_eq!(props, vec!["alpha", "beta"]);
    }

    #[test]
    fn collect_from_ref() {
        let schema = json!({
            "$defs": {
                "Base": {
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" }
                    }
                }
            },
            "$ref": "#/$defs/Base",
            "additionalProperties": false
        });
        let mut props = collect_schema_properties(&schema, "/additionalProperties");
        props.sort();
        assert_eq!(props, vec!["id", "name"]);
    }

    #[test]
    fn collect_empty_on_bad_path() {
        let schema = json!({"type": "object"});
        let props = collect_schema_properties(&schema, "/nonexistent/additionalProperties");
        assert!(props.is_empty());
    }

    // --- suggest_for_property ---

    #[test]
    fn suggest_dash_underscore() {
        let valid = vec!["argument-hint".to_string()];
        assert_eq!(
            suggest_for_property("argument_hint", &valid),
            Some("argument-hint".to_string())
        );
    }

    #[test]
    fn suggest_case_insensitive() {
        let valid = vec!["Name".to_string()];
        assert_eq!(
            suggest_for_property("name", &valid),
            Some("Name".to_string())
        );
    }

    #[test]
    fn suggest_typo() {
        let valid = vec!["description".to_string(), "name".to_string()];
        assert_eq!(
            suggest_for_property("desciption", &valid),
            Some("description".to_string())
        );
    }

    #[test]
    fn suggest_no_match_too_distant() {
        let valid = vec!["name".to_string(), "age".to_string()];
        assert_eq!(suggest_for_property("completely_different", &valid), None);
    }

    #[test]
    fn suggest_no_match_short_string() {
        // "ab" vs "xy" — distance 2, but max_len is 2, so 2*2 > 2 → rejected
        let valid = vec!["xy".to_string()];
        assert_eq!(suggest_for_property("ab", &valid), None);
    }

    #[test]
    fn suggest_picks_closest() {
        let valid = vec!["argument-hint".to_string(), "argument-type".to_string()];
        assert_eq!(
            suggest_for_property("argument_hint", &valid),
            Some("argument-hint".to_string())
        );
    }

    // --- enrich_message ---

    #[test]
    fn enrich_single_suggestion() {
        let schema = json!({
            "properties": {
                "argument-hint": { "type": "string" },
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        let msg = "Additional properties are not allowed ('argument_hint' was unexpected)";
        let enriched = enrich_message(msg, "/additionalProperties", &schema);
        assert_eq!(
            enriched,
            "Additional properties are not allowed ('argument_hint' was unexpected); did you mean 'argument-hint'?"
        );
    }

    #[test]
    fn enrich_no_suggestion() {
        let schema = json!({
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        let msg = "Additional properties are not allowed ('completely_different' was unexpected)";
        let enriched = enrich_message(msg, "/additionalProperties", &schema);
        assert_eq!(enriched, msg);
    }

    #[test]
    fn enrich_non_additional_properties() {
        let schema = json!({"type": "object"});
        let msg = "Some other validation error";
        let enriched = enrich_message(msg, "/type", &schema);
        assert_eq!(enriched, msg);
    }

    #[test]
    fn enrich_multiple_unexpected_one_suggestion() {
        let schema = json!({
            "properties": {
                "argument-hint": { "type": "string" },
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        let msg =
            "Additional properties are not allowed ('argument_hint', 'zzzzzzz' were unexpected)";
        let enriched = enrich_message(msg, "/additionalProperties", &schema);
        assert_eq!(
            enriched,
            "Additional properties are not allowed ('argument_hint', 'zzzzzzz' were unexpected); did you mean 'argument-hint'? (for 'argument_hint')"
        );
    }

    #[test]
    fn enrich_multiple_suggestions() {
        let schema = json!({
            "properties": {
                "argument-hint": { "type": "string" },
                "description": { "type": "string" }
            },
            "additionalProperties": false
        });
        let msg =
            "Additional properties are not allowed ('argument_hint', 'desciption' were unexpected)";
        let enriched = enrich_message(msg, "/additionalProperties", &schema);
        assert!(enriched.contains("did you mean 'argument-hint'? (for 'argument_hint')"));
        assert!(enriched.contains("did you mean 'description'? (for 'desciption')"));
    }

    #[test]
    fn enrich_empty_valid_properties() {
        let schema = json!({"type": "object"});
        let msg = "Additional properties are not allowed ('foo' was unexpected)";
        let enriched = enrich_message(msg, "/additionalProperties", &schema);
        assert_eq!(enriched, msg);
    }
}
