/// Post-processing transformations applied to JSON Schema values before they
/// are written to disk: key reordering, description link conversion, etc.
mod linkify_descriptions;
mod reorder_schema_keys;

use linkify_descriptions::linkify_descriptions;
use reorder_schema_keys::reorder_schema_keys;

/// Apply all post-processing transformations to a JSON Schema value.
///
/// Currently this:
/// 1. Reorders top-level keys so well-known fields come first.
/// 2. Wraps bare URLs in `description` fields as Markdown autolinks.
pub fn postprocess_schema(value: &mut serde_json::Value) {
    reorder_schema_keys(value);
    linkify_descriptions(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorders_and_linkifies() {
        let mut schema = serde_json::json!({
            "type": "object",
            "description": "See https://example.com",
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Test"
        });
        postprocess_schema(&mut schema);

        let keys: Vec<&String> = schema
            .as_object()
            .expect("test value is an object")
            .keys()
            .collect();
        assert_eq!(keys, &["$schema", "title", "description", "type"]);

        assert_eq!(schema["description"], "See <https://example.com>");
    }
}
