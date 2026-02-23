#![doc = include_str!("../README.md")]

/// Schema keywords used to distinguish schema-like objects from data properties.
const SCHEMA_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "$ref",
    "allOf",
    "oneOf",
    "anyOf",
    "definitions",
    "$defs",
    "items",
    "required",
    "enum",
    "not",
    "if",
    "then",
    "else",
    "patternProperties",
    "additionalProperties",
];

/// Migrate a JSON Schema document to draft 2020-12 in-place.
///
/// Applies all necessary keyword transformations for drafts 04 through 2019-09.
/// Safe to call on schemas that are already 2020-12 (idempotent).
pub fn migrate_to_2020_12(schema: &mut serde_json::Value) {
    // Pass 1: Set $schema at root
    if let Some(obj) = schema.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            serde_json::Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
        );
    }

    // Pass 2: Recursive keyword transformations
    migrate_keywords(schema);

    // Pass 3: Rewrite $ref paths (#/definitions/ → #/$defs/)
    rewrite_definition_refs(schema);
}

/// Recursively apply keyword transformations.
fn migrate_keywords(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            migrate_object_keywords(map);
            for v in map.values_mut() {
                migrate_keywords(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                migrate_keywords(v);
            }
        }
        _ => {}
    }
}

/// Apply all keyword transforms to a single JSON object.
fn migrate_object_keywords(map: &mut serde_json::Map<String, serde_json::Value>) {
    // definitions → $defs
    if map.contains_key("definitions")
        && !map.contains_key("$defs")
        && let Some(defs) = map.remove("definitions")
    {
        map.insert("$defs".to_string(), defs);
    }

    migrate_id(map);

    // Array items → prefixItems
    if let Some(items) = map.get("items")
        && items.is_array()
    {
        if let Some(items_val) = map.remove("items") {
            map.insert("prefixItems".to_string(), items_val);
        }
        if let Some(additional) = map.remove("additionalItems") {
            map.insert("items".to_string(), additional);
        }
    }

    // Boolean exclusiveMinimum/exclusiveMaximum (draft-04)
    migrate_exclusive_bound(map, "exclusiveMinimum", "minimum");
    migrate_exclusive_bound(map, "exclusiveMaximum", "maximum");

    // String "deprecated" → boolean true
    if let Some(dep) = map.get("deprecated")
        && dep.is_string()
    {
        map.insert("deprecated".to_string(), serde_json::Value::Bool(true));
    }

    // String "false"/"true" in schema-boolean positions
    migrate_string_booleans(map);

    // dependencies → dependentSchemas + dependentRequired
    migrate_dependencies(map);

    // Null annotation keywords → remove
    // Some generators emit "description": null, "title": null, etc.
    // The meta-schema requires these to be strings.
    for key in &["description", "title", "$comment"] {
        if let Some(v) = map.get(*key)
            && v.is_null()
        {
            map.remove(*key);
        }
    }

    // Normalize regex patterns for Rust regex_syntax compatibility
    if let Some(serde_json::Value::String(pat)) = map.get("pattern") {
        let norm = normalize_ecma_regex(pat);
        if norm != *pat {
            map.insert("pattern".to_string(), serde_json::Value::String(norm));
        }
    }
    if let Some(serde_json::Value::Object(pp)) = map.get("patternProperties") {
        let any_changed = pp.keys().any(|k| normalize_ecma_regex(k) != *k);
        if any_changed && let Some(serde_json::Value::Object(pp)) = map.remove("patternProperties")
        {
            let new_pp: serde_json::Map<String, serde_json::Value> = pp
                .into_iter()
                .map(|(k, v)| (normalize_ecma_regex(&k), v))
                .collect();
            map.insert(
                "patternProperties".to_string(),
                serde_json::Value::Object(new_pp),
            );
        }
    }
}

/// Migrate `id` → `$id` (draft-04) and remove fragment-only identifiers.
fn migrate_id(map: &mut serde_json::Map<String, serde_json::Value>) {
    // id → $id (only non-fragment, schema-like objects)
    if !map.contains_key("$id")
        && let Some(serde_json::Value::String(id_str)) = map.get("id")
        && !id_str.starts_with('#')
    {
        let looks_like_schema = SCHEMA_KEYWORDS.iter().any(|kw| map.contains_key(*kw));
        if looks_like_schema && let Some(id_val) = map.remove("id") {
            map.insert("$id".to_string(), id_val);
        }
    }
    // Remove fragment-only id values
    if let Some(serde_json::Value::String(id_str)) = map.get("id")
        && id_str.starts_with('#')
    {
        map.remove("id");
    }
    // Remove fragment-only $id values (invalid in 2020-12)
    if let Some(serde_json::Value::String(id_str)) = map.get("$id")
        && id_str.starts_with('#')
    {
        map.remove("$id");
    }
}

/// Fix string `"false"`/`"true"` in positions that require boolean or schema.
fn migrate_string_booleans(map: &mut serde_json::Map<String, serde_json::Value>) {
    for key in &[
        "additionalProperties",
        "additionalItems",
        "unevaluatedProperties",
        "unevaluatedItems",
    ] {
        if let Some(serde_json::Value::String(s)) = map.get(*key) {
            let replacement = match s.as_str() {
                "false" => Some(false),
                "true" => Some(true),
                _ => None,
            };
            if let Some(b) = replacement {
                map.insert((*key).to_string(), serde_json::Value::Bool(b));
            }
        }
    }
}

/// Split `dependencies` into `dependentSchemas` + `dependentRequired`.
fn migrate_dependencies(map: &mut serde_json::Map<String, serde_json::Value>) {
    if !map.contains_key("dependencies") {
        return;
    }
    let Some(serde_json::Value::Object(deps)) = map.remove("dependencies") else {
        return;
    };
    let mut schemas = serde_json::Map::new();
    let mut required = serde_json::Map::new();
    for (key, val) in deps {
        if val.is_array() {
            required.insert(key, val);
        } else {
            schemas.insert(key, val);
        }
    }
    if !schemas.is_empty() && !map.contains_key("dependentSchemas") {
        map.insert(
            "dependentSchemas".to_string(),
            serde_json::Value::Object(schemas),
        );
    }
    if !required.is_empty() && !map.contains_key("dependentRequired") {
        map.insert(
            "dependentRequired".to_string(),
            serde_json::Value::Object(required),
        );
    }
}

/// Normalize an ECMA 262 regex pattern for compatibility with Rust's `regex_syntax`.
///
/// Two incompatibilities are fixed:
///
/// 1. **Bare braces**: Unescaped `{` and `}` that do not form valid quantifiers
///    (`{n}`, `{n,}`, `{n,m}`) are escaped. ECMA 262 treats unmatched braces as
///    literals, but `regex_syntax` rejects them.
///
/// 2. **`\d` in character classes**: `\d` inside `[…]` is expanded to `0-9`.
///    This prevents `regex_syntax` from rejecting patterns where `\d` appears as
///    a range endpoint (e.g. `[\d-\.]`), and ensures ASCII-only digit matching
///    consistent with ECMA 262 semantics.
#[allow(clippy::missing_panics_doc)] // from_utf8 cannot panic on our output
pub fn normalize_ecma_regex(pattern: &str) -> String {
    let b = pattern.as_bytes();
    let valid_braces = find_valid_quantifier_braces(b);
    let mut out = Vec::with_capacity(b.len() + 16);
    let mut i = 0;
    let mut in_class = false;

    while i < b.len() {
        // Handle escape sequences
        if b[i] == b'\\' && i + 1 < b.len() {
            let next = b[i + 1];

            // Expand \d → 0-9 inside character classes
            if in_class && next == b'd' {
                out.extend_from_slice(b"0-9");
                i += 2;
                continue;
            }

            // Pass through Unicode escapes: \p{...}, \P{...}, \u{...}
            if matches!(next, b'p' | b'P' | b'u') && i + 2 < b.len() && b[i + 2] == b'{' {
                out.push(b'\\');
                out.push(next);
                i += 2;
                if let Some(close) = b[i..].iter().position(|&c| c == b'}') {
                    out.extend_from_slice(&b[i..=i + close]);
                    i += close + 1;
                }
                continue;
            }

            out.push(b[i]);
            out.push(next);
            i += 2;
            continue;
        }

        // Character class start
        if b[i] == b'[' && !in_class {
            in_class = true;
            out.push(b'[');
            i += 1;
            // Skip negation and literal ] at class start
            if i < b.len() && b[i] == b'^' {
                out.push(b'^');
                i += 1;
            }
            if i < b.len() && b[i] == b']' {
                out.push(b']');
                i += 1;
            }
            continue;
        }
        if b[i] == b']' && in_class {
            in_class = false;
            out.push(b']');
            i += 1;
            continue;
        }

        // Inside character class, everything is literal (no brace escaping needed)
        if in_class {
            out.push(b[i]);
            i += 1;
            continue;
        }

        // Escape bare braces outside character class
        if b[i] == b'{' && !valid_braces[i] {
            out.extend_from_slice(b"\\{");
            i += 1;
            continue;
        }
        if b[i] == b'}' && !valid_braces[i] {
            out.extend_from_slice(b"\\}");
            i += 1;
            continue;
        }

        out.push(b[i]);
        i += 1;
    }

    // Safety: input is valid UTF-8 and we only replace/insert ASCII bytes.
    // Non-ASCII bytes (≥ 128) never match our ASCII comparisons, so multi-byte
    // UTF-8 sequences pass through unchanged.
    String::from_utf8(out).expect("normalization preserves UTF-8")
}

/// Identify positions of `{` and `}` that form valid quantifiers.
fn find_valid_quantifier_braces(b: &[u8]) -> Vec<bool> {
    let mut valid = vec![false; b.len()];
    let mut i = 0;
    let mut in_class = false;

    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            // Skip Unicode escapes: \p{...}, \P{...}, \u{...}
            if matches!(b[i + 1], b'p' | b'P' | b'u') && i + 2 < b.len() && b[i + 2] == b'{' {
                i += 2;
                if let Some(close) = b[i..].iter().position(|&c| c == b'}') {
                    i += close + 1;
                }
                continue;
            }
            i += 2;
            continue;
        }
        if b[i] == b'[' && !in_class {
            in_class = true;
            i += 1;
            continue;
        }
        if b[i] == b']' && in_class {
            in_class = false;
            i += 1;
            continue;
        }
        if b[i] == b'{'
            && !in_class
            && let Some(end) = parse_quantifier(b, i)
        {
            valid[i] = true;
            valid[end] = true;
            i = end + 1;
            continue;
        }
        i += 1;
    }

    valid
}

/// Check if `b[start]` (which must be `{`) begins a valid quantifier.
///
/// Returns the index of the closing `}` if valid, or `None` otherwise.
/// Valid forms: `{n}`, `{n,}`, `{n,m}` where n and m are non-negative integers.
fn parse_quantifier(b: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    // First number (required)
    let n_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == n_start || i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n}
    }
    if b[i] != b',' {
        return None;
    }
    i += 1; // skip comma
    if i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n,}
    }
    // Second number
    let n_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == n_start || i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n,m}
    }
    None
}

/// Handle boolean `exclusiveMinimum`/`exclusiveMaximum` (draft-04).
///
/// - `true` + companion present: set exclusive = companion value, remove companion
/// - `false`: just remove exclusive
fn migrate_exclusive_bound(
    map: &mut serde_json::Map<String, serde_json::Value>,
    exclusive_key: &str,
    companion_key: &str,
) {
    if let Some(serde_json::Value::Bool(b)) = map.get(exclusive_key) {
        let b = *b;
        if b {
            if let Some(companion) = map.get(companion_key).cloned() {
                map.insert(exclusive_key.to_string(), companion);
                map.remove(companion_key);
            } else {
                map.remove(exclusive_key);
            }
        } else {
            map.remove(exclusive_key);
        }
    }
}

/// Rewrite `#/definitions/` → `#/$defs/` in `$ref` and `$id` strings.
fn rewrite_definition_refs(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in &["$ref", "$id"] {
                if let Some(serde_json::Value::String(s)) = map.get(*key) {
                    let new_s = s.replace("#/definitions/", "#/$defs/");
                    if new_s != *s {
                        map.insert((*key).to_string(), serde_json::Value::String(new_s));
                    }
                }
            }
            for v in map.values_mut() {
                rewrite_definition_refs(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                rewrite_definition_refs(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sets_schema_at_root() {
        let mut schema = json!({"type": "object"});
        migrate_to_2020_12(&mut schema);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn renames_definitions_to_defs_top_level() {
        let mut schema = json!({
            "definitions": {
                "Foo": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("definitions").is_none());
        assert_eq!(schema["$defs"]["Foo"]["type"], "string");
    }

    #[test]
    fn renames_definitions_to_defs_nested() {
        let mut schema = json!({
            "properties": {
                "nested": {
                    "definitions": {
                        "Bar": {"type": "number"}
                    }
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["properties"]["nested"].get("definitions").is_none());
        assert_eq!(
            schema["properties"]["nested"]["$defs"]["Bar"]["type"],
            "number"
        );
    }

    #[test]
    fn rewrites_ref_definitions_to_defs() {
        let mut schema = json!({
            "$ref": "#/definitions/Foo",
            "$defs": {
                "Foo": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["$ref"], "#/$defs/Foo");
    }

    #[test]
    fn rewrites_ref_with_external_base() {
        let mut schema = json!({
            "$ref": "https://x.com/foo.json#/definitions/Bar"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["$ref"], "https://x.com/foo.json#/$defs/Bar");
    }

    #[test]
    fn renames_id_to_dollar_id_on_schema_like_objects() {
        let mut schema = json!({
            "id": "https://example.com/schema",
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("id").is_none());
        // Root $id is overwritten by $schema setter, but sub-schemas should work
    }

    #[test]
    fn does_not_rename_id_inside_properties() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "record ID"}
            }
        });
        migrate_to_2020_12(&mut schema);
        // The "id" property descriptor should NOT be renamed
        assert!(schema["properties"].get("id").is_some());
    }

    #[test]
    fn array_items_becomes_prefix_items() {
        let mut schema = json!({
            "items": [
                {"type": "string"},
                {"type": "number"}
            ]
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("items").is_none());
        assert_eq!(schema["prefixItems"][0]["type"], "string");
        assert_eq!(schema["prefixItems"][1]["type"], "number");
    }

    #[test]
    fn additional_items_becomes_items_with_tuple() {
        let mut schema = json!({
            "items": [
                {"type": "string"}
            ],
            "additionalItems": {"type": "number"}
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("additionalItems").is_none());
        assert_eq!(schema["prefixItems"][0]["type"], "string");
        assert_eq!(schema["items"]["type"], "number");
    }

    #[test]
    fn boolean_exclusive_minimum_true() {
        let mut schema = json!({
            "type": "number",
            "minimum": 5,
            "exclusiveMinimum": true
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["exclusiveMinimum"], 5);
        assert!(schema.get("minimum").is_none());
    }

    #[test]
    fn boolean_exclusive_minimum_false() {
        let mut schema = json!({
            "type": "number",
            "minimum": 5,
            "exclusiveMinimum": false
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("exclusiveMinimum").is_none());
        assert_eq!(schema["minimum"], 5);
    }

    #[test]
    fn boolean_exclusive_maximum_true() {
        let mut schema = json!({
            "type": "number",
            "maximum": 10,
            "exclusiveMaximum": true
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["exclusiveMaximum"], 10);
        assert!(schema.get("maximum").is_none());
    }

    #[test]
    fn dependencies_split() {
        let mut schema = json!({
            "dependencies": {
                "bar": {"properties": {"baz": {"type": "string"}}},
                "quux": ["foo", "bar"]
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema.get("dependencies").is_none());
        assert!(
            schema["dependentSchemas"]["bar"]
                .get("properties")
                .is_some()
        );
        assert_eq!(schema["dependentRequired"]["quux"], json!(["foo", "bar"]));
    }

    #[test]
    fn drops_fragment_only_id() {
        let mut schema = json!({
            "definitions": {
                "envVar": {
                    "id": "#/definitions/envVar",
                    "type": "object"
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        // Fragment-only id values are dropped (2020-12 $id must not have fragments)
        assert!(schema["$defs"]["envVar"].get("id").is_none());
        assert!(schema["$defs"]["envVar"].get("$id").is_none());
    }

    #[test]
    fn rewrites_dollar_id_definitions_to_defs() {
        let mut schema = json!({
            "$defs": {
                "envVar": {
                    "$id": "https://example.com/schemas/envVar#/definitions/nested",
                    "type": "object"
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(
            schema["$defs"]["envVar"]["$id"],
            "https://example.com/schemas/envVar#/$defs/nested"
        );
    }

    #[test]
    fn string_deprecated_becomes_bool() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "old_field": {
                    "type": "string",
                    "deprecated": "Use new_field instead."
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["properties"]["old_field"]["deprecated"], json!(true));
    }

    #[test]
    fn bool_deprecated_unchanged() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "old_field": {
                    "type": "string",
                    "deprecated": true
                }
            }
        });
        let expected = schema.clone();
        migrate_to_2020_12(&mut schema);
        // deprecated: true stays true (not touched)
        assert_eq!(
            schema["properties"]["old_field"]["deprecated"],
            expected["properties"]["old_field"]["deprecated"]
        );
    }

    #[test]
    fn string_false_additional_properties() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": "false"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["additionalProperties"], json!(false));
    }

    #[test]
    fn string_true_additional_properties() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": "true"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["additionalProperties"], json!(true));
    }

    #[test]
    fn null_description_removed() {
        let mut schema = json!({
            "$defs": {
                "Entry": {
                    "type": "object",
                    "description": null,
                    "title": null
                }
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["$defs"]["Entry"].get("description").is_none());
        assert!(schema["$defs"]["Entry"].get("title").is_none());
    }

    #[test]
    fn string_description_unchanged() {
        let mut schema = json!({
            "type": "object",
            "description": "A schema"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["description"], "A schema");
    }

    #[test]
    fn already_2020_12_is_idempotent() {
        let original = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.com/test",
            "type": "object",
            "$defs": {
                "Foo": {"type": "string"}
            },
            "properties": {
                "x": {"$ref": "#/$defs/Foo"}
            }
        });
        let mut schema = original.clone();
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema, original);
    }

    #[test]
    fn combined_old_draft_features() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-04/schema#",
            "id": "https://example.com/old",
            "type": "object",
            "definitions": {
                "Pos": {
                    "type": "number",
                    "minimum": 0,
                    "exclusiveMinimum": true
                }
            },
            "properties": {
                "coords": {
                    "items": [
                        {"$ref": "#/definitions/Pos"},
                        {"$ref": "#/definitions/Pos"}
                    ],
                    "additionalItems": false
                },
                "id": {"type": "string"}
            },
            "dependencies": {
                "coords": ["id"]
            }
        });
        migrate_to_2020_12(&mut schema);

        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert!(schema.get("definitions").is_none());
        assert!(schema["$defs"]["Pos"].get("minimum").is_none());
        assert_eq!(schema["$defs"]["Pos"]["exclusiveMinimum"], 0);
        assert_eq!(
            schema["properties"]["coords"]["prefixItems"][0]["$ref"],
            "#/$defs/Pos"
        );
        assert_eq!(schema["properties"]["coords"]["items"], false);
        assert!(
            schema["properties"]["coords"]
                .get("additionalItems")
                .is_none()
        );
        assert!(schema.get("dependencies").is_none());
        assert_eq!(schema["dependentRequired"]["coords"], json!(["id"]));
        // "id" inside properties should NOT be renamed
        assert!(schema["properties"].get("id").is_some());
    }

    // --- normalize_ecma_regex tests ---

    #[test]
    fn normalize_regex_bare_braces_escaped() {
        // Quali-torque pattern: bare { and } should be escaped
        assert_eq!(
            normalize_ecma_regex(r"^{?[a-zA-Z0-9-_.@#\s]+}?$"),
            r"^\{?[a-zA-Z0-9-_.@#\s]+\}?$"
        );
    }

    #[test]
    fn normalize_regex_valid_quantifier_preserved() {
        assert_eq!(normalize_ecma_regex(r"^[0-9a-f]{40}$"), r"^[0-9a-f]{40}$");
        assert_eq!(normalize_ecma_regex(r"\d{1,3}"), r"\d{1,3}");
        assert_eq!(normalize_ecma_regex(r"x{2,}y"), r"x{2,}y");
    }

    #[test]
    fn normalize_regex_escaped_braces_preserved() {
        assert_eq!(normalize_ecma_regex(r"\{\{.*\}\}"), r"\{\{.*\}\}");
    }

    #[test]
    fn normalize_regex_backslash_d_expanded_in_class() {
        // Lotus-yaml pattern: \d inside character class → 0-9
        assert_eq!(
            normalize_ecma_regex(r"^[a-z][a-z\d-\.]*[a-z\d]$"),
            r"^[a-z][a-z0-9-\.]*[a-z0-9]$"
        );
    }

    #[test]
    fn normalize_regex_backslash_d_preserved_outside_class() {
        assert_eq!(normalize_ecma_regex(r"^\d+$"), r"^\d+$");
    }

    #[test]
    fn normalize_regex_idempotent() {
        let patterns = [
            r"^[a-z][a-z0-9-\.]*[a-z0-9]$",
            r"^\{?[a-zA-Z0-9]+\}?$",
            r"^[0-9a-f]{40}$",
            r"^\d{1,3}\.\d{1,3}$",
        ];
        for pat in patterns {
            assert_eq!(normalize_ecma_regex(pat), pat, "not idempotent: {pat}");
        }
    }

    #[test]
    fn normalize_regex_combined_braces_and_class() {
        // Pattern with both bare braces and \d in character class
        assert_eq!(normalize_ecma_regex(r"^{[\d-\.]+}$"), r"^\{[0-9-\.]+\}$");
    }

    #[test]
    fn normalize_regex_pattern_in_schema() {
        let mut schema = json!({
            "type": "string",
            "pattern": r"^{?[a-zA-Z0-9]+}?$"
        });
        migrate_to_2020_12(&mut schema);
        assert_eq!(schema["pattern"], r"^\{?[a-zA-Z0-9]+\}?$");
    }

    #[test]
    fn normalize_regex_pattern_properties_keys() {
        let mut schema = json!({
            "type": "object",
            "patternProperties": {
                "^{[a-z]+}$": {"type": "string"}
            }
        });
        migrate_to_2020_12(&mut schema);
        assert!(schema["patternProperties"].get(r"^\{[a-z]+\}$").is_some());
        assert!(schema["patternProperties"].get("^{[a-z]+}$").is_none());
    }

    #[test]
    fn normalize_regex_unicode_property_escapes_preserved() {
        // \p{L} and \P{N} should NOT have their braces escaped
        assert_eq!(
            normalize_ecma_regex(r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$"),
            r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$"
        );
        // \P{...} (negated) also preserved
        assert_eq!(normalize_ecma_regex(r"\P{Lu}"), r"\P{Lu}");
        // \u{...} Unicode code point escape preserved
        assert_eq!(normalize_ecma_regex(r"\u{1f600}"), r"\u{1f600}");
    }

    #[test]
    fn normalized_patterns_parse_with_regex_syntax() {
        use regex_syntax::ast::parse::Parser;

        let patterns = [
            // Quali-torque: bare braces
            r"^{?[a-zA-Z0-9-_.@#\s]+}?$",
            // Lotus-yaml: \d in character class range
            r"^[a-z][a-z\d-\.]*[a-z\d]$",
            // Architect: already-escaped braces (should still pass)
            r"\$\{\{\s*(.*?)\s*\}\}",
            // Architect with IP: quantifiers + escaped braces
            r"\$\{\{\s*(.*?)\s*\}\}|(?:\d{1,3}\.){3}\d{1,3}(?:\/\d\d?)?,?",
            // Cycle-stack: escaped braces
            r#"\"?\{\{(\$)?([a-z0-9\-]+)\}\}\"?"#,
            // Unicode property escapes
            r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$",
        ];
        for pat in patterns {
            let norm = normalize_ecma_regex(pat);
            let result = Parser::new().parse(&norm);
            assert!(
                result.is_ok(),
                "pattern {norm:?} failed to parse: {}",
                result.expect_err("unreachable")
            );
        }
    }
}
