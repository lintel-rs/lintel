use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use serde::Serialize;
use serde_json::Value;

/// Full schema documentation extracted from a JSON Schema.
#[derive(Serialize, Default)]
pub struct SchemaDoc {
    pub title: Option<String>,
    pub description_html: Option<String>,
    pub schema_type: Option<String>,
    pub properties: Vec<PropertyDoc>,
    pub items: Option<Box<SubSchemaDoc>>,
    pub examples: Vec<ExampleDoc>,
    pub compositions: Vec<CompositionDoc>,
    pub definitions: Vec<DefinitionDoc>,
    pub has_content: bool,
}

/// A single property in the schema.
#[derive(Serialize, Default)]
pub struct PropertyDoc {
    pub name: String,
    pub schema_type: Option<String>,
    pub required: bool,
    pub description_html: Option<String>,
    pub default: Option<String>,
    pub default_is_complex: bool,
    pub enum_values: Vec<String>,
    pub const_value: Option<String>,
    pub examples: Vec<String>,
    pub constraints: Vec<ConstraintDoc>,
    pub compositions: Vec<CompositionDoc>,
    pub properties: Vec<PropertyDoc>,
    pub has_nested: bool,
    /// Anchor ID linking to a definition (e.g. `def-Foo`), when the type comes from a `$ref`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_anchor: Option<String>,
}

/// A validation constraint (e.g. `format=email`).
#[derive(Serialize)]
pub struct ConstraintDoc {
    pub label: String,
    pub value: String,
}

/// A composition block (`oneOf`/`anyOf`/`allOf`).
#[derive(Serialize)]
pub struct CompositionDoc {
    pub keyword: String,
    pub label: String,
    pub variants: Vec<VariantDoc>,
}

/// A single variant within a composition.
#[derive(Serialize)]
pub struct VariantDoc {
    pub index: usize,
    pub label: String,
    pub schema_type: Option<String>,
    pub description_html: Option<String>,
    pub properties: Vec<PropertyDoc>,
    pub is_expanded: bool,
}

/// A definition from `$defs` / `definitions`.
#[derive(Serialize)]
pub struct DefinitionDoc {
    pub name: String,
    /// Anchor slug for linking (e.g. `def-Foo`).
    pub slug: String,
    pub schema_type: Option<String>,
    pub description_html: Option<String>,
    pub properties: Vec<PropertyDoc>,
}

/// A top-level example value.
#[derive(Serialize)]
pub struct ExampleDoc {
    pub is_complex: bool,
    pub content: String,
}

/// Sub-schema info for array `items`.
#[derive(Serialize, Default)]
pub struct SubSchemaDoc {
    pub schema_type: Option<String>,
    pub description_html: Option<String>,
    pub properties: Vec<PropertyDoc>,
}

const COMPOSITION_KEYWORDS: &[(&str, &str)] = &[
    ("oneOf", "One of"),
    ("anyOf", "Any of"),
    ("allOf", "All of"),
];

const MAX_DEPTH: usize = 3;

/// Extract structured documentation from a JSON Schema value.
pub fn extract_schema_doc(schema: &Value) -> SchemaDoc {
    let title = schema
        .get("title")
        .and_then(Value::as_str)
        .map(String::from);
    let desc = get_description(schema).map(md_to_html);
    let schema_type = schema_type_str(schema);

    let required = required_set(schema);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| extract_properties(props, &required, schema, 0))
        .unwrap_or_default();

    let items = extract_items(schema);
    let examples = extract_examples(schema);
    let compositions = extract_compositions(schema, schema, 0);
    let definitions = extract_definitions(schema);

    let has_content = !properties.is_empty()
        || items.is_some()
        || !compositions.is_empty()
        || !definitions.is_empty()
        || !examples.is_empty();

    SchemaDoc {
        title,
        description_html: desc,
        schema_type,
        properties,
        items,
        examples,
        compositions,
        definitions,
        has_content,
    }
}

fn extract_items(schema: &Value) -> Option<Box<SubSchemaDoc>> {
    let items = schema.get("items")?;
    let resolved = resolve_ref(items, schema);
    let ty = schema_type_str(resolved);
    let desc = get_description(resolved).map(md_to_html);
    let required = required_set(resolved);
    let properties = resolved
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| extract_properties(p, &required, schema, 1))
        .unwrap_or_default();
    Some(Box::new(SubSchemaDoc {
        schema_type: ty,
        description_html: desc,
        properties,
    }))
}

fn extract_examples(schema: &Value) -> Vec<ExampleDoc> {
    schema
        .get("examples")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let is_complex = matches!(v, Value::Object(_) | Value::Array(_));
                    let content = if is_complex {
                        serde_json::to_string_pretty(v).unwrap_or_default()
                    } else {
                        format_value(v)
                    };
                    ExampleDoc {
                        is_complex,
                        content,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_compositions(schema: &Value, root: &Value, depth: usize) -> Vec<CompositionDoc> {
    let mut result = Vec::new();
    for &(keyword, label) in COMPOSITION_KEYWORDS {
        if let Some(variants) = schema.get(keyword).and_then(Value::as_array) {
            let variants: Vec<VariantDoc> = variants
                .iter()
                .enumerate()
                .map(|(i, v)| extract_variant(v, root, i + 1, depth))
                .collect();
            if !variants.is_empty() {
                result.push(CompositionDoc {
                    keyword: String::from(keyword),
                    label: String::from(label),
                    variants,
                });
            }
        }
    }
    result
}

fn extract_variant(variant: &Value, root: &Value, index: usize, depth: usize) -> VariantDoc {
    let resolved = resolve_ref(variant, root);
    let label = variant_label(variant, resolved);
    let ty = schema_type_str(resolved);
    let desc = get_description(resolved).map(md_to_html);
    let required = required_set(resolved);
    let properties = if depth < MAX_DEPTH {
        resolved
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| extract_properties(p, &required, root, depth + 1))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let is_expanded = desc.is_some() || !properties.is_empty();
    VariantDoc {
        index,
        label,
        schema_type: ty,
        description_html: desc,
        properties,
        is_expanded,
    }
}

fn variant_label(original: &Value, resolved: &Value) -> String {
    if let Some(title) = resolved.get("title").and_then(Value::as_str) {
        return String::from(title);
    }
    if let Some(r) = original.get("$ref").and_then(Value::as_str) {
        return String::from(ref_name(r));
    }
    schema_type_str(resolved).unwrap_or_else(|| String::from("variant"))
}

fn extract_definitions(schema: &Value) -> Vec<DefinitionDoc> {
    let mut defs = Vec::new();
    for key in &["$defs", "definitions"] {
        if let Some(map) = schema.get(*key).and_then(Value::as_object) {
            for (name, def_schema) in map {
                let resolved = resolve_ref(def_schema, schema);
                let ty = schema_type_str(resolved);
                let desc = get_description(resolved).map(md_to_html);
                let required = required_set(resolved);
                let properties = resolved
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|p| extract_properties(p, &required, schema, 1))
                    .unwrap_or_default();
                defs.push(DefinitionDoc {
                    name: name.clone(),
                    slug: alloc::format!("def-{name}"),
                    schema_type: ty,
                    description_html: desc,
                    properties,
                });
            }
        }
    }
    defs
}

fn extract_properties(
    props: &serde_json::Map<String, Value>,
    required: &[String],
    root: &Value,
    depth: usize,
) -> Vec<PropertyDoc> {
    let mut sorted: Vec<_> = props.iter().collect();
    sorted.sort_by_key(|(name, _)| i32::from(!required.contains(name)));

    sorted
        .into_iter()
        .map(|(name, prop_schema)| {
            extract_single_property(name, prop_schema, required, root, depth)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn extract_single_property(
    name: &str,
    prop_schema: &Value,
    required: &[String],
    root: &Value,
    depth: usize,
) -> PropertyDoc {
    let resolved = resolve_ref(prop_schema, root);
    let ty = schema_type_str(resolved);
    let is_required = required.iter().any(|r| r == name);
    let desc = get_description(resolved).map(md_to_html);
    let (default, default_is_complex) = match resolved.get("default") {
        Some(v) if matches!(v, Value::Object(_) | Value::Array(_)) => (
            Some(serde_json::to_string_pretty(v).unwrap_or_default()),
            true,
        ),
        Some(v) => (Some(format_value(v)), false),
        None => (None, false),
    };
    let enum_values = extract_enum_values(resolved);
    let const_value = resolved.get("const").map(format_value);
    let examples = extract_property_examples(resolved);
    let constraints = extract_constraints(resolved);
    let compositions = extract_compositions(resolved, root, depth);

    // Link type badge to definition anchor when the property is a local $ref
    let ref_anchor = prop_schema
        .get("$ref")
        .and_then(Value::as_str)
        .filter(|r| r.starts_with("#/$defs/") || r.starts_with("#/definitions/"))
        .map(|r| alloc::format!("def-{}", ref_name(r)));

    let nested_required = required_set(resolved);
    let nested = if depth < MAX_DEPTH {
        resolved
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| extract_properties(p, &nested_required, root, depth + 1))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let has_nested = !nested.is_empty();

    PropertyDoc {
        name: String::from(name),
        schema_type: ty,
        required: is_required,
        description_html: desc,
        default,
        default_is_complex,
        enum_values,
        const_value,
        examples,
        constraints,
        compositions,
        properties: nested,
        has_nested,
        ref_anchor,
    }
}

fn extract_enum_values(schema: &Value) -> Vec<String> {
    schema
        .get("enum")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(format_value).collect())
        .unwrap_or_default()
}

fn extract_property_examples(schema: &Value) -> Vec<String> {
    schema
        .get("examples")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(format_value).collect())
        .unwrap_or_default()
}

fn extract_constraints(schema: &Value) -> Vec<ConstraintDoc> {
    let mut c = Vec::new();

    if let Some(v) = schema.get("format").and_then(Value::as_str) {
        c.push(ConstraintDoc {
            label: String::from("format"),
            value: String::from(v),
        });
    }
    if let Some(v) = schema.get("minLength").and_then(Value::as_u64) {
        c.push(ConstraintDoc {
            label: String::from("minLength"),
            value: alloc::format!("{v}"),
        });
    }
    if let Some(v) = schema.get("maxLength").and_then(Value::as_u64) {
        c.push(ConstraintDoc {
            label: String::from("maxLength"),
            value: alloc::format!("{v}"),
        });
    }
    if let Some(v) = schema.get("pattern").and_then(Value::as_str) {
        c.push(ConstraintDoc {
            label: String::from("pattern"),
            value: String::from(v),
        });
    }
    push_numeric_constraint(&mut c, schema, "minimum", "min");
    push_numeric_constraint(&mut c, schema, "maximum", "max");
    push_numeric_constraint(&mut c, schema, "exclusiveMinimum", "exclusiveMin");
    push_numeric_constraint(&mut c, schema, "exclusiveMaximum", "exclusiveMax");
    push_numeric_constraint(&mut c, schema, "multipleOf", "multipleOf");
    if let Some(v) = schema.get("minItems").and_then(Value::as_u64) {
        c.push(ConstraintDoc {
            label: String::from("minItems"),
            value: alloc::format!("{v}"),
        });
    }
    if let Some(v) = schema.get("maxItems").and_then(Value::as_u64) {
        c.push(ConstraintDoc {
            label: String::from("maxItems"),
            value: alloc::format!("{v}"),
        });
    }
    if schema
        .get("uniqueItems")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        c.push(ConstraintDoc {
            label: String::from("uniqueItems"),
            value: String::from("true"),
        });
    }

    c
}

fn push_numeric_constraint(c: &mut Vec<ConstraintDoc>, schema: &Value, key: &str, label: &str) {
    if let Some(v) = schema.get(key) {
        c.push(ConstraintDoc {
            label: String::from(label),
            value: v.to_string(),
        });
    }
}

// --- Schema helpers (same logic as jsonschema-explain) ---

fn resolve_ref<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str)
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        let mut current = root;
        for segment in path.split('/') {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            if let Value::Object(map) = current {
                if let Some(next) = map.get(&decoded) {
                    current = next;
                } else {
                    return schema;
                }
            } else {
                return schema;
            }
        }
        return current;
    }
    schema
}

fn ref_name(ref_str: &str) -> &str {
    ref_str.rsplit('/').next().unwrap_or(ref_str)
}

fn schema_type_str(schema: &Value) -> Option<String> {
    if let Some(ty) = schema.get("type") {
        return match ty {
            Value::String(s) if s == "array" => {
                match schema.get("items").and_then(schema_type_str) {
                    Some(item_ty) => Some(alloc::format!("{item_ty}[]")),
                    None => Some(String::from("array")),
                }
            }
            Value::String(s) => Some(s.clone()),
            Value::Array(arr) => {
                let types: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
                Some(types.join(" | "))
            }
            _ => None,
        };
    }
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        return Some(String::from(ref_name(r)));
    }
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let types: Vec<String> = variants
                .iter()
                .filter_map(|v| {
                    schema_type_str(v).or_else(|| {
                        v.get("$ref")
                            .and_then(Value::as_str)
                            .map(|r| String::from(ref_name(r)))
                    })
                })
                .collect();
            if !types.is_empty() {
                return Some(types.join(" | "));
            }
        }
    }
    if let Some(c) = schema.get("const") {
        return Some(alloc::format!("const: {c}"));
    }
    if schema.get("enum").is_some() {
        return Some(String::from("enum"));
    }
    None
}

fn get_description(schema: &Value) -> Option<&str> {
    schema
        .get("markdownDescription")
        .and_then(Value::as_str)
        .or_else(|| schema.get("description").and_then(Value::as_str))
}

fn required_set(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn format_value(val: &Value) -> String {
    match val {
        Value::String(s) => alloc::format!("\"{s}\""),
        other => other.to_string(),
    }
}

/// Convert markdown text to HTML using pulldown-cmark.
///
/// External links (`http://` and `https://`) are annotated with
/// `target="_blank" rel="noopener noreferrer"`.
fn md_to_html(text: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    externalize_links(&mut html_output);
    html_output
}

/// Add `target="_blank" rel="noopener noreferrer"` to external `<a>` tags.
fn externalize_links(html: &mut String) {
    // Schema descriptions only contain external http(s) links; our site uses
    // relative paths, so matching on the protocol is safe.
    *html = html
        .replace(
            "<a href=\"https://",
            "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://",
        )
        .replace(
            "<a href=\"http://",
            "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"http://",
        );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_simple_object() {
        let schema = json!({
            "title": "Config",
            "description": "A configuration **schema**",
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string", "description": "The name" },
                "debug": { "type": "boolean", "default": false }
            }
        });
        let doc = extract_schema_doc(&schema);
        assert_eq!(doc.title.as_deref(), Some("Config"));
        assert!(doc.description_html.as_ref().unwrap().contains("<strong>"));
        assert_eq!(doc.schema_type.as_deref(), Some("object"));
        assert_eq!(doc.properties.len(), 2);
        // Required field first
        assert_eq!(doc.properties[0].name, "name");
        assert!(doc.properties[0].required);
        assert_eq!(doc.properties[1].name, "debug");
        assert!(!doc.properties[1].required);
        assert_eq!(doc.properties[1].default.as_deref(), Some("false"));
        assert!(doc.has_content);
    }

    #[test]
    fn extract_enum_and_const() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level": { "type": "string", "enum": ["low", "high"] },
                "version": { "const": 2 }
            }
        });
        let doc = extract_schema_doc(&schema);
        let level = &doc.properties[0];
        assert_eq!(level.enum_values, vec!["\"low\"", "\"high\""]);
        let version = &doc.properties[1];
        assert_eq!(version.const_value.as_deref(), Some("2"));
    }

    #[test]
    fn extract_constraints() {
        let schema = json!({
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "format": "email",
                    "minLength": 5,
                    "maxLength": 255
                }
            }
        });
        let doc = extract_schema_doc(&schema);
        let email = &doc.properties[0];
        assert_eq!(email.constraints.len(), 3);
        assert_eq!(email.constraints[0].label, "format");
        assert_eq!(email.constraints[0].value, "email");
    }

    #[test]
    fn extract_ref_resolution() {
        let schema = json!({
            "type": "object",
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "description": "An item",
                    "properties": {
                        "id": { "type": "integer" }
                    }
                }
            }
        });
        let doc = extract_schema_doc(&schema);
        let item = &doc.properties[0];
        assert_eq!(item.schema_type.as_deref(), Some("object"));
        assert!(item.description_html.as_ref().unwrap().contains("An item"));
        assert!(item.has_nested);
        assert_eq!(item.properties[0].name, "id");
    }

    #[test]
    fn extract_compositions() {
        let schema = json!({
            "oneOf": [
                { "type": "string", "title": "String variant" },
                { "type": "integer" }
            ]
        });
        let doc = extract_schema_doc(&schema);
        assert_eq!(doc.compositions.len(), 1);
        assert_eq!(doc.compositions[0].keyword, "oneOf");
        assert_eq!(doc.compositions[0].variants.len(), 2);
        assert_eq!(doc.compositions[0].variants[0].label, "String variant");
    }

    #[test]
    fn extract_definitions() {
        let schema = json!({
            "$defs": {
                "Foo": { "type": "string", "description": "A foo" }
            }
        });
        let doc = extract_schema_doc(&schema);
        assert_eq!(doc.definitions.len(), 1);
        assert_eq!(doc.definitions[0].name, "Foo");
    }

    #[test]
    fn extract_examples() {
        let schema = json!({
            "examples": [
                "simple",
                { "key": "value" }
            ]
        });
        let doc = extract_schema_doc(&schema);
        assert_eq!(doc.examples.len(), 2);
        assert!(!doc.examples[0].is_complex);
        assert!(doc.examples[1].is_complex);
    }

    #[test]
    fn markdown_rendering() {
        let html = md_to_html("Hello **world**");
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn empty_schema_has_no_content() {
        let schema = json!({});
        let doc = extract_schema_doc(&schema);
        assert!(!doc.has_content);
    }

    #[test]
    fn prefers_markdown_description() {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": {
                    "description": "Plain",
                    "markdownDescription": "**Rich**"
                }
            }
        });
        let doc = extract_schema_doc(&schema);
        assert!(
            doc.properties[0]
                .description_html
                .as_ref()
                .unwrap()
                .contains("<strong>Rich</strong>")
        );
    }

    #[test]
    fn array_items_extracted() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "description": "An item",
                "properties": {
                    "id": { "type": "integer" }
                }
            }
        });
        let doc = extract_schema_doc(&schema);
        let items = doc.items.as_ref().unwrap();
        assert_eq!(items.schema_type.as_deref(), Some("object"));
        assert_eq!(items.properties.len(), 1);
    }

    #[test]
    fn depth_limiting() {
        // Build a deeply nested schema (depth 5).
        // MAX_DEPTH=3 means properties at depth 0, 1, 2 extract their children.
        // At depth 3, nested property extraction is skipped.
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {
                    "type": "object",
                    "properties": {
                        "b": {
                            "type": "object",
                            "properties": {
                                "c": {
                                    "type": "object",
                                    "properties": {
                                        "d": {
                                            "type": "object",
                                            "properties": {
                                                "e": { "type": "string" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let doc = extract_schema_doc(&schema);
        // a(0) -> b(1) -> c(2) -> d(3): d is extracted but d's children (e) are NOT
        let a = &doc.properties[0];
        let b = &a.properties[0];
        let c = &b.properties[0];
        assert!(!c.properties.is_empty(), "c should have d");
        let d = &c.properties[0];
        assert!(
            d.properties.is_empty(),
            "depth limit should prevent d's children"
        );
    }
}
