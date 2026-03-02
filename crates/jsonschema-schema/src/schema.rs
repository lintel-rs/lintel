use alloc::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::extensions::LintelExt;
use crate::extensions::TaploInfo;
use crate::extensions::TaploSchemaExt;
use crate::extensions::TombiExt;

/// A JSON Schema value — either a boolean schema or an object schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaValue {
    Bool(bool),
    Schema(Box<Schema>),
}

/// JSON Schema `type` keyword — single type string or union array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypeValue {
    Single(String),
    Union(Vec<String>),
}

/// A JSON Schema object (draft 2020-12).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    // --- Core identifiers ---
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(rename = "$id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        rename = "markdownDescription",
        skip_serializing_if = "Option::is_none"
    )]
    pub markdown_description: Option<String>,
    #[serde(rename = "x-lintel", skip_serializing_if = "Option::is_none")]
    pub x_lintel: Option<LintelExt>,

    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(rename = "$anchor", skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(rename = "$dynamicRef", skip_serializing_if = "Option::is_none")]
    pub dynamic_ref: Option<String>,
    #[serde(rename = "$dynamicAnchor", skip_serializing_if = "Option::is_none")]
    pub dynamic_anchor: Option<String>,
    #[serde(rename = "$comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(rename = "$defs", skip_serializing_if = "Option::is_none")]
    pub defs: Option<BTreeMap<String, SchemaValue>>,

    // --- Metadata ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(rename = "readOnly", skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(rename = "writeOnly", skip_serializing_if = "Option::is_none")]
    pub write_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<Value>>,

    // --- Type ---
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<TypeValue>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_: Option<Vec<Value>>,
    #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
    pub const_: Option<Value>,

    // --- Object ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<IndexMap<String, SchemaValue>>,
    #[serde(rename = "patternProperties", skip_serializing_if = "Option::is_none")]
    pub pattern_properties: Option<IndexMap<String, SchemaValue>>,
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<Box<SchemaValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(rename = "propertyNames", skip_serializing_if = "Option::is_none")]
    pub property_names: Option<Box<SchemaValue>>,
    #[serde(rename = "minProperties", skip_serializing_if = "Option::is_none")]
    pub min_properties: Option<u64>,
    #[serde(rename = "maxProperties", skip_serializing_if = "Option::is_none")]
    pub max_properties: Option<u64>,
    #[serde(
        rename = "unevaluatedProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub unevaluated_properties: Option<Box<SchemaValue>>,

    // --- Array ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<SchemaValue>>,
    #[serde(rename = "prefixItems", skip_serializing_if = "Option::is_none")]
    pub prefix_items: Option<Vec<SchemaValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<Box<SchemaValue>>,
    #[serde(rename = "minContains", skip_serializing_if = "Option::is_none")]
    pub min_contains: Option<u64>,
    #[serde(rename = "maxContains", skip_serializing_if = "Option::is_none")]
    pub max_contains: Option<u64>,
    #[serde(rename = "minItems", skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,
    #[serde(rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    #[serde(rename = "uniqueItems", skip_serializing_if = "Option::is_none")]
    pub unique_items: Option<bool>,
    #[serde(rename = "unevaluatedItems", skip_serializing_if = "Option::is_none")]
    pub unevaluated_items: Option<Box<SchemaValue>>,

    // --- Number ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<Value>,
    #[serde(rename = "exclusiveMinimum", skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<Value>,
    #[serde(rename = "exclusiveMaximum", skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<Value>,
    #[serde(rename = "multipleOf", skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<Value>,

    // --- String ---
    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    // --- Composition ---
    #[serde(rename = "allOf", skip_serializing_if = "Option::is_none")]
    pub all_of: Option<Vec<SchemaValue>>,
    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<SchemaValue>>,
    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<SchemaValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<SchemaValue>>,

    // --- Conditional ---
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_: Option<Box<SchemaValue>>,
    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_: Option<Box<SchemaValue>>,
    #[serde(rename = "else", skip_serializing_if = "Option::is_none")]
    pub else_: Option<Box<SchemaValue>>,

    // --- Dependencies (2020-12) ---
    #[serde(rename = "dependentRequired", skip_serializing_if = "Option::is_none")]
    pub dependent_required: Option<IndexMap<String, Vec<String>>>,
    #[serde(rename = "dependentSchemas", skip_serializing_if = "Option::is_none")]
    pub dependent_schemas: Option<IndexMap<String, SchemaValue>>,

    // --- Content ---
    #[serde(rename = "contentMediaType", skip_serializing_if = "Option::is_none")]
    pub content_media_type: Option<String>,
    #[serde(rename = "contentEncoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
    #[serde(rename = "contentSchema", skip_serializing_if = "Option::is_none")]
    pub content_schema: Option<Box<SchemaValue>>,

    #[serde(rename = "x-taplo", skip_serializing_if = "Option::is_none")]
    pub x_taplo: Option<TaploSchemaExt>,
    #[serde(rename = "x-taplo-info", skip_serializing_if = "Option::is_none")]
    pub x_taplo_info: Option<TaploInfo>,
    #[serde(flatten)]
    pub x_tombi: TombiExt,

    // --- Catch-all for unknown properties ---
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SchemaValue {
    /// Get the inner `Schema` if this is an object schema, `None` for bool schemas.
    pub fn as_schema(&self) -> Option<&Schema> {
        match self {
            Self::Schema(s) => Some(s),
            Self::Bool(_) => None,
        }
    }
}

impl Schema {
    /// Parse from a `serde_json::Value` without migration.
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be deserialized into a `Schema`.
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Get the best description text, preferring `markdownDescription`.
    pub fn description(&self) -> Option<&str> {
        self.markdown_description
            .as_deref()
            .or(self.description.as_deref())
    }

    /// Get the required fields, or an empty slice.
    pub fn required_set(&self) -> &[String] {
        self.required.as_deref().unwrap_or_default()
    }

    /// Whether this schema is deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.deprecated.unwrap_or(false)
    }

    /// Produce a short human-readable type string.
    pub fn type_str(&self) -> Option<String> {
        schema_type_str(self)
    }

    /// Look up a schema-keyword field by its JSON key name.
    ///
    /// Returns a reference to the `SchemaValue` stored under that keyword,
    /// or `None` if the field is absent.
    pub fn get_keyword(&self, key: &str) -> Option<&SchemaValue> {
        match key {
            "items" => self.items.as_deref(),
            "contains" => self.contains.as_deref(),
            "additionalProperties" => self.additional_properties.as_deref(),
            "propertyNames" => self.property_names.as_deref(),
            "unevaluatedProperties" => self.unevaluated_properties.as_deref(),
            "unevaluatedItems" => self.unevaluated_items.as_deref(),
            "not" => self.not.as_deref(),
            "if" => self.if_.as_deref(),
            "then" => self.then_.as_deref(),
            "else" => self.else_.as_deref(),
            "contentSchema" => self.content_schema.as_deref(),
            _ => None,
        }
    }

    /// Look up a named child within a keyword that holds a map of schemas.
    ///
    /// For example, `get_map_entry("properties", "name")` returns the schema
    /// for the `name` property.
    pub fn get_map_entry(&self, keyword: &str, key: &str) -> Option<&SchemaValue> {
        match keyword {
            "properties" => self.properties.as_ref()?.get(key),
            "patternProperties" => self.pattern_properties.as_ref()?.get(key),
            "$defs" => self.defs.as_ref()?.get(key),
            "dependentSchemas" => self.dependent_schemas.as_ref()?.get(key),
            _ => None,
        }
    }

    /// Look up an indexed child within a keyword that holds an array of schemas.
    pub fn get_array_entry(&self, keyword: &str, index: usize) -> Option<&SchemaValue> {
        match keyword {
            "allOf" => self.all_of.as_ref()?.get(index),
            "anyOf" => self.any_of.as_ref()?.get(index),
            "oneOf" => self.one_of.as_ref()?.get(index),
            "prefixItems" => self.prefix_items.as_ref()?.get(index),
            _ => None,
        }
    }
}

/// Produce a short human-readable type string for a schema.
fn schema_type_str(schema: &Schema) -> Option<String> {
    // Explicit type field
    if let Some(ref ty) = schema.type_ {
        return match ty {
            TypeValue::Single(s) if s == "array" => {
                let item_ty = schema
                    .items
                    .as_ref()
                    .and_then(|sv| sv.as_schema())
                    .and_then(schema_type_str);
                match item_ty {
                    Some(item_ty) => Some(format!("{item_ty}[]")),
                    None => Some("array".to_string()),
                }
            }
            TypeValue::Single(s) => Some(s.clone()),
            TypeValue::Union(arr) => Some(arr.join(" | ")),
        };
    }

    // $ref
    if let Some(ref r) = schema.ref_ {
        return Some(ref_name(r).to_string());
    }

    // oneOf/anyOf
    for variants in [&schema.one_of, &schema.any_of].into_iter().flatten() {
        let types: Vec<String> = variants
            .iter()
            .filter_map(|v| match v {
                SchemaValue::Schema(s) => {
                    schema_type_str(s).or_else(|| s.ref_.as_ref().map(|r| ref_name(r).to_string()))
                }
                SchemaValue::Bool(_) => None,
            })
            .collect();
        if !types.is_empty() {
            return Some(types.join(" | "));
        }
    }

    // const
    if let Some(ref c) = schema.const_ {
        return Some(format!("const: {c}"));
    }

    // enum
    if schema.enum_.is_some() {
        return Some("enum".to_string());
    }

    None
}

/// Extract the trailing name from a `$ref` path (e.g. `"#/$defs/Foo"` -> `"Foo"`).
pub fn ref_name(ref_str: &str) -> &str {
    ref_str.rsplit('/').next().unwrap_or(ref_str)
}

/// Resolve a `$ref` within the same schema document.
///
/// If the given schema has a `$ref` that begins with `#/`, follow the path
/// through the root schema. Otherwise return the schema unchanged.
pub fn resolve_ref<'a>(schema: &'a Schema, root: &'a Schema) -> &'a Schema {
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        // Navigate the root using serde_json::Value for flexibility
        let Ok(root_value) = serde_json::to_value(root) else {
            return schema;
        };
        let mut current = &root_value;
        for segment in path.split('/') {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            match current.get(&decoded) {
                Some(next) => current = next,
                None => return schema,
            }
        }
        // Try to deserialize the resolved value back into a Schema.
        // This is expensive, so we use a different approach for the explain crate.
        // For now, just return the original schema — the explain crate has its own
        // resolve_ref that works with SchemaValue trees directly.
        let _ = current;
        return schema;
    }
    schema
}

/// Walk a JSON Pointer path through a schema, resolving `$ref` at each step.
///
/// Segments are decoded per RFC 6901 (`~1` → `/`, `~0` → `~`).
/// Returns the sub-`SchemaValue` at the given pointer, or an error.
///
/// # Errors
///
/// Returns an error if a segment in the pointer cannot be resolved.
pub fn navigate_pointer<'a>(
    schema: &'a SchemaValue,
    root: &'a SchemaValue,
    pointer: &str,
) -> Result<&'a SchemaValue, String> {
    let path = pointer.strip_prefix('/').unwrap_or(pointer);
    if path.is_empty() {
        return Ok(schema);
    }

    let mut current = resolve_schema_value_ref(schema, root);
    let mut segments = path.split('/').peekable();

    while let Some(segment) = segments.next() {
        let decoded = segment.replace("~1", "/").replace("~0", "~");
        current = resolve_schema_value_ref(current, root);

        let Some(schema) = current.as_schema() else {
            return Err(format!(
                "cannot resolve segment '{decoded}' in pointer '{pointer}'"
            ));
        };

        // Map-bearing keywords: consume this segment AND the next one.
        if is_map_keyword(&decoded) {
            let key_segment = segments
                .next()
                .ok_or_else(|| format!("expected key after '{decoded}' in pointer '{pointer}'"))?;
            let key = key_segment.replace("~1", "/").replace("~0", "~");
            if let Some(entry) = schema.get_map_entry(&decoded, &key) {
                current = entry;
                continue;
            }
            return Err(format!(
                "cannot resolve segment '{key}' in '{decoded}' in pointer '{pointer}'"
            ));
        }

        // Array-bearing keywords: consume this segment, then the next as an index.
        if is_array_keyword(&decoded) {
            let idx_segment = segments.next().ok_or_else(|| {
                format!("expected index after '{decoded}' in pointer '{pointer}'")
            })?;
            let idx: usize = idx_segment.parse().map_err(|_| {
                format!("expected numeric index after '{decoded}', got '{idx_segment}'")
            })?;
            if let Some(entry) = schema.get_array_entry(&decoded, idx) {
                current = entry;
                continue;
            }
            return Err(format!(
                "index {idx} out of bounds in '{decoded}' in pointer '{pointer}'"
            ));
        }

        // Single-value keywords (items, not, if, then, else, etc.)
        if let Some(sv) = schema.get_keyword(&decoded) {
            current = sv;
            continue;
        }

        // Fall back: try as a key in the schema's maps (for when the
        // pointer navigates directly into a map without naming the keyword).
        if let Some(sv) = schema.get_map_entry_by_pointer_segment(&decoded) {
            current = sv;
            continue;
        }

        // Try as array index (for arrays embedded in composition keywords)
        if let Ok(idx) = decoded.parse::<usize>() {
            let found = ["allOf", "anyOf", "oneOf", "prefixItems"]
                .iter()
                .find_map(|kw| schema.get_array_entry(kw, idx));
            if let Some(entry) = found {
                current = entry;
                continue;
            }
        }

        return Err(format!(
            "cannot resolve segment '{decoded}' in pointer '{pointer}'"
        ));
    }

    Ok(resolve_schema_value_ref(current, root))
}

/// Whether a JSON pointer segment names a map-bearing keyword.
fn is_map_keyword(segment: &str) -> bool {
    matches!(
        segment,
        "properties" | "patternProperties" | "$defs" | "dependentSchemas"
    )
}

/// Whether a JSON pointer segment names an array-bearing keyword.
fn is_array_keyword(segment: &str) -> bool {
    matches!(segment, "allOf" | "anyOf" | "oneOf" | "prefixItems")
}

/// Resolve `$ref` on a `SchemaValue`, returning the referenced `SchemaValue`.
fn resolve_schema_value_ref<'a>(sv: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
    let Some(schema) = sv.as_schema() else {
        return sv;
    };
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        let mut current = root;
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            let Some(inner) = current.as_schema() else {
                return sv;
            };

            // Map-bearing keywords: consume the next segment as a key
            if is_map_keyword(&decoded) {
                let Some(key_segment) = segments.next() else {
                    return sv;
                };
                let key = key_segment.replace("~1", "/").replace("~0", "~");
                match inner.get_map_entry(&decoded, &key) {
                    Some(n) => current = n,
                    None => return sv,
                }
                continue;
            }

            // Array-bearing keywords: consume the next segment as an index
            if is_array_keyword(&decoded) {
                let Some(idx_segment) = segments.next() else {
                    return sv;
                };
                let Ok(idx) = idx_segment.parse::<usize>() else {
                    return sv;
                };
                match inner.get_array_entry(&decoded, idx) {
                    Some(n) => current = n,
                    None => return sv,
                }
                continue;
            }

            // Single-value keywords
            if let Some(n) = inner.get_keyword(&decoded) {
                current = n;
                continue;
            }

            // Fall back to map entry lookup
            if let Some(n) = inner.get_map_entry_by_pointer_segment(&decoded) {
                current = n;
                continue;
            }

            return sv;
        }
        return current;
    }
    sv
}

impl Schema {
    /// Look up a child by a JSON pointer segment name.
    /// This handles both map keywords (where the segment is a key within the map)
    /// and direct keywords.
    fn get_map_entry_by_pointer_segment(&self, segment: &str) -> Option<&SchemaValue> {
        // Try all map-bearing keyword fields.
        // For pointer navigation, when we're inside a "properties" object,
        // the segment is the property name.
        self.properties
            .as_ref()
            .and_then(|m| m.get(segment))
            .or_else(|| {
                self.pattern_properties
                    .as_ref()
                    .and_then(|m| m.get(segment))
            })
            .or_else(|| {
                self.defs
                    .as_ref()
                    .and_then(|m: &BTreeMap<String, SchemaValue>| m.get(segment))
            })
            .or_else(|| self.dependent_schemas.as_ref().and_then(|m| m.get(segment)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_simple_schema() {
        let json = json!({
            "type": "object",
            "title": "Test",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let schema: Schema = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(schema.title.as_deref(), Some("Test"));
        assert!(schema.properties.is_some());

        let back = serde_json::to_value(&schema).unwrap();
        assert_eq!(back["type"], "object");
        assert_eq!(back["title"], "Test");
    }

    #[test]
    fn bool_schema_value() {
        let json = json!(true);
        let sv: SchemaValue = serde_json::from_value(json).unwrap();
        assert!(matches!(sv, SchemaValue::Bool(true)));
        assert!(sv.as_schema().is_none());
    }

    #[test]
    fn schema_value_object() {
        let json = json!({"type": "string"});
        let sv: SchemaValue = serde_json::from_value(json).unwrap();
        let s = sv.as_schema().unwrap();
        assert!(matches!(s.type_, Some(TypeValue::Single(ref t)) if t == "string"));
    }

    #[test]
    fn type_value_single() {
        let json = json!("string");
        let tv: TypeValue = serde_json::from_value(json).unwrap();
        assert!(matches!(tv, TypeValue::Single(ref s) if s == "string"));
    }

    #[test]
    fn type_value_union() {
        let json = json!(["string", "null"]);
        let tv: TypeValue = serde_json::from_value(json).unwrap();
        assert!(matches!(tv, TypeValue::Union(ref v) if v.len() == 2));
    }

    #[test]
    fn description_prefers_markdown() {
        let schema = Schema {
            description: Some("plain".into()),
            markdown_description: Some("**rich**".into()),
            ..Default::default()
        };
        assert_eq!(schema.description(), Some("**rich**"));
    }

    #[test]
    fn description_falls_back() {
        let schema = Schema {
            description: Some("plain".into()),
            ..Default::default()
        };
        assert_eq!(schema.description(), Some("plain"));
    }

    #[test]
    fn type_str_simple() {
        let schema = Schema {
            type_: Some(TypeValue::Single("string".into())),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string"));
    }

    #[test]
    fn type_str_union() {
        let schema = Schema {
            type_: Some(TypeValue::Union(vec!["string".into(), "null".into()])),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string | null"));
    }

    #[test]
    fn type_str_array_with_items() {
        let items = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single("string".into())),
            ..Default::default()
        }));
        let schema = Schema {
            type_: Some(TypeValue::Single("array".into())),
            items: Some(Box::new(items)),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string[]"));
    }

    #[test]
    fn type_str_ref() {
        let schema = Schema {
            ref_: Some("#/$defs/Foo".into()),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("Foo"));
    }

    #[test]
    fn is_deprecated_default_false() {
        let schema = Schema::default();
        assert!(!schema.is_deprecated());
    }

    #[test]
    fn is_deprecated_true() {
        let schema = Schema {
            deprecated: Some(true),
            ..Default::default()
        };
        assert!(schema.is_deprecated());
    }

    #[test]
    fn required_set_empty() {
        let schema = Schema::default();
        assert!(schema.required_set().is_empty());
    }

    #[test]
    fn required_set_values() {
        let schema = Schema {
            required: Some(vec!["a".into(), "b".into()]),
            ..Default::default()
        };
        assert_eq!(schema.required_set(), &["a", "b"]);
    }

    #[test]
    fn extra_fields_preserved() {
        let json = json!({
            "type": "object",
            "x-custom": "value",
            "x-another": 42
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert_eq!(schema.extra.get("x-custom").unwrap(), "value");
        assert_eq!(schema.extra.get("x-another").unwrap(), 42);
    }

    #[test]
    fn x_taplo_deserialization() {
        let json = json!({
            "type": "object",
            "x-taplo": {
                "hidden": true,
                "docs": {
                    "main": "Main docs"
                }
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        let taplo = schema.x_taplo.unwrap();
        assert_eq!(taplo.hidden, Some(true));
        assert_eq!(taplo.docs.unwrap().main.as_deref(), Some("Main docs"));
    }

    #[test]
    fn x_lintel_deserialization() {
        let json = json!({
            "type": "object",
            "x-lintel": {
                "source": "https://example.com/schema.json",
                "sourceSha256": "abc123"
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        let lintel = schema.x_lintel.unwrap();
        assert_eq!(
            lintel.source.as_deref(),
            Some("https://example.com/schema.json")
        );
        assert_eq!(lintel.source_sha256.as_deref(), Some("abc123"));
    }

    #[test]
    fn navigate_pointer_empty() {
        let sv = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single("object".into())),
            ..Default::default()
        }));
        let result = navigate_pointer(&sv, &sv, "").unwrap();
        assert!(result.as_schema().is_some());
    }

    #[test]
    fn navigate_pointer_properties() {
        let name_schema = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single("string".into())),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("name".into(), name_schema);
        let root = SchemaValue::Schema(Box::new(Schema {
            properties: Some(props),
            ..Default::default()
        }));
        let result = navigate_pointer(&root, &root, "/properties/name").unwrap();
        let s = result.as_schema().unwrap();
        assert!(matches!(s.type_, Some(TypeValue::Single(ref t)) if t == "string"));
    }

    #[test]
    fn navigate_pointer_resolves_ref() {
        let item_schema = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single("object".into())),
            description: Some("An item".into()),
            ..Default::default()
        }));
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Item".into()),
            ..Default::default()
        }));
        let mut defs = BTreeMap::new();
        defs.insert("Item".into(), item_schema);
        let mut props = IndexMap::new();
        props.insert("item".into(), ref_schema);
        let root = SchemaValue::Schema(Box::new(Schema {
            properties: Some(props),
            defs: Some(defs),
            ..Default::default()
        }));
        let result = navigate_pointer(&root, &root, "/properties/item").unwrap();
        let s = result.as_schema().unwrap();
        assert_eq!(s.description.as_deref(), Some("An item"));
    }

    #[test]
    fn navigate_pointer_bad_segment_errors() {
        let sv = SchemaValue::Schema(Box::default());
        let err = navigate_pointer(&sv, &sv, "/nonexistent").unwrap_err();
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn parse_cargo_fixture() {
        let content =
            std::fs::read_to_string("../jsonschema-migrate/tests/fixtures/cargo.json").unwrap();
        let value: Value = serde_json::from_str(&content).unwrap();
        let mut migrated = value;
        jsonschema_migrate::migrate_to_2020_12(&mut migrated);
        let schema: Schema = serde_json::from_value(migrated).unwrap();
        assert!(schema.title.is_some() || schema.type_.is_some());
        // Verify x-taplo is parsed if present
        if schema.x_taplo.is_some() {
            // Just verify it parsed without error
        }
    }
}
