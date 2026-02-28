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
    /// Individual type parts for linked composite types (e.g. `Foo | Bar`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub type_parts: Vec<TypePart>,
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
    /// Individual type segments for composed types (e.g. `oneOf`), each optionally linked.
    /// When non-empty, the template renders these instead of the plain `schema_type` string.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schema_type_parts: Vec<TypePart>,
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
    /// Href linking to a definition or external schema page, when the type comes from a `$ref`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_anchor: Option<String>,
}

/// A segment within a composed type string, optionally linking to a definition or schema page.
#[derive(Serialize, Default)]
pub struct TypePart {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
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
    /// Link target when the variant references another schema page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_href: Option<String>,
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
    pub examples: Vec<String>,
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

/// Site context for resolving external `$ref` links.
pub struct SiteContext<'a> {
    /// Full base URL of the catalog (e.g. `https://catalog.lintel.tools/`).
    pub base_url: &'a str,
    /// Root-relative path for internal links (e.g. `/` or `/catalog/`).
    pub base_path: &'a str,
}

/// Shared context threaded through all extraction functions.
struct ExtractContext<'a> {
    /// Root schema for resolving local `$ref`s.
    root: &'a Value,
    /// Current nesting depth (capped at [`MAX_DEPTH`]).
    depth: usize,
    /// Optional site context for resolving external links.
    site: Option<&'a SiteContext<'a>>,
}

impl ExtractContext<'_> {
    fn deeper(&self) -> Self {
        Self {
            root: self.root,
            depth: self.depth + 1,
            site: self.site,
        }
    }
}

/// Extract structured documentation from a JSON Schema value.
///
/// When `site` is provided, external `$ref` links within the same catalog are
/// resolved to clickable page URLs.
pub fn extract_schema_doc(schema: &Value, site: Option<&SiteContext<'_>>) -> SchemaDoc {
    let title = schema
        .get("title")
        .and_then(Value::as_str)
        .map(String::from);
    let desc = get_description(schema).map(md_to_html);
    let schema_type = schema_type_str(schema);

    let ctx = ExtractContext {
        root: schema,
        depth: 0,
        site,
    };

    let required = required_set(schema);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|props| extract_properties(props, &required, &ctx))
        .unwrap_or_default();

    let items = extract_items(schema, &ctx);
    let examples = extract_examples(schema);
    let compositions = extract_compositions(schema, &ctx);
    let definitions = extract_definitions(schema, &ctx);
    let type_parts = compute_type_parts(schema, site);

    let has_content = !properties.is_empty()
        || items.is_some()
        || !compositions.is_empty()
        || !definitions.is_empty()
        || !examples.is_empty();

    SchemaDoc {
        title,
        description_html: desc,
        schema_type,
        type_parts,
        properties,
        items,
        examples,
        compositions,
        definitions,
        has_content,
    }
}

fn extract_items(schema: &Value, ctx: &ExtractContext<'_>) -> Option<Box<SubSchemaDoc>> {
    let items = schema.get("items")?;
    let resolved = resolve_ref(items, ctx.root);
    let ty = schema_type_str(resolved);
    let desc = get_description(resolved).map(md_to_html);
    let required = required_set(resolved);
    let child_ctx = ExtractContext {
        root: ctx.root,
        depth: 1,
        site: ctx.site,
    };
    let properties = resolved
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| extract_properties(p, &required, &child_ctx))
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

fn extract_compositions(schema: &Value, ctx: &ExtractContext<'_>) -> Vec<CompositionDoc> {
    let mut result = Vec::new();
    for &(keyword, label) in COMPOSITION_KEYWORDS {
        if let Some(variants) = schema.get(keyword).and_then(Value::as_array) {
            let variants: Vec<VariantDoc> = variants
                .iter()
                .enumerate()
                .map(|(i, v)| extract_variant(v, i + 1, ctx))
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

fn extract_variant(variant: &Value, index: usize, ctx: &ExtractContext<'_>) -> VariantDoc {
    let resolved = resolve_ref(variant, ctx.root);
    let label = variant_label(variant, resolved);
    let ty = schema_type_str(resolved);

    // Link to a local definition or external schema page
    let ref_href = variant
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| {
            if r.starts_with("#/$defs/") || r.starts_with("#/definitions/") {
                Some(alloc::format!("#def-{}", ref_name(r)))
            } else {
                ctx.site.and_then(|s| ref_to_href(r, s))
            }
        })
        .or_else(|| {
            // Also check array items for external refs
            ctx.site.and_then(|s| {
                let ref_str = find_ref_target(variant)?;
                ref_to_href(ref_str, s)
            })
        });

    // When variant links to a definition, skip desc/properties — the definition section has them.
    let is_local_def = variant
        .get("$ref")
        .and_then(Value::as_str)
        .is_some_and(|r| r.starts_with("#/$defs/") || r.starts_with("#/definitions/"));

    let (desc, properties) = if is_local_def {
        (None, Vec::new())
    } else {
        let desc = get_description(resolved).map(md_to_html);
        let required = required_set(resolved);
        let props = if ctx.depth < MAX_DEPTH {
            resolved
                .get("properties")
                .and_then(Value::as_object)
                .map(|p| extract_properties(p, &required, &ctx.deeper()))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        (desc, props)
    };
    let is_expanded = desc.is_some() || !properties.is_empty();

    VariantDoc {
        index,
        label,
        schema_type: ty,
        description_html: desc,
        properties,
        is_expanded,
        ref_href,
    }
}

fn variant_label(original: &Value, resolved: &Value) -> String {
    if let Some(title) = resolved.get("title").and_then(Value::as_str) {
        return String::from(title);
    }
    if let Some(r) = original.get("$ref").and_then(Value::as_str) {
        return ref_name(r);
    }
    schema_type_str(resolved).unwrap_or_else(|| String::from("variant"))
}

fn extract_definitions(schema: &Value, ctx: &ExtractContext<'_>) -> Vec<DefinitionDoc> {
    let mut defs = Vec::new();
    let child_ctx = ExtractContext {
        root: ctx.root,
        depth: 1,
        site: ctx.site,
    };
    for key in &["$defs", "definitions"] {
        if let Some(map) = schema.get(*key).and_then(Value::as_object) {
            for (name, def_schema) in map {
                let resolved = resolve_ref(def_schema, ctx.root);
                let ty = schema_type_str(resolved);
                let desc = get_description(resolved).map(md_to_html);
                let required = required_set(resolved);
                let properties = resolved
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|p| extract_properties(p, &required, &child_ctx))
                    .unwrap_or_default();
                let examples = extract_raw_examples(resolved);
                defs.push(DefinitionDoc {
                    name: name.clone(),
                    slug: alloc::format!("def-{name}"),
                    schema_type: ty,
                    description_html: desc,
                    properties,
                    examples,
                });
            }
        }
    }
    defs
}

fn extract_properties(
    props: &serde_json::Map<String, Value>,
    required: &[String],
    ctx: &ExtractContext<'_>,
) -> Vec<PropertyDoc> {
    let mut sorted: Vec<_> = props.iter().collect();
    sorted.sort_by_key(|(name, _)| i32::from(!required.contains(name)));

    sorted
        .into_iter()
        .map(|(name, prop_schema)| extract_single_property(name, prop_schema, required, ctx))
        .collect()
}

fn extract_single_property(
    name: &str,
    prop_schema: &Value,
    required: &[String],
    ctx: &ExtractContext<'_>,
) -> PropertyDoc {
    let resolved = resolve_ref(prop_schema, ctx.root);
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

    // When the type badge already shows the oneOf/anyOf composition (no explicit `type` field),
    // skip the redundant inline composition section.
    let type_from_composition = resolved.get("type").is_none()
        && (resolved.get("oneOf").is_some() || resolved.get("anyOf").is_some());
    let compositions = if type_from_composition {
        Vec::new()
    } else {
        extract_compositions(resolved, ctx)
    };

    // Build individually-linkable type parts for composed types (oneOf/anyOf)
    let schema_type_parts = compute_type_parts(resolved, ctx.site);

    // Link type badge to a definition or external schema page.
    // Check the direct `$ref` first, then walk into array `items`.
    let ref_anchor = find_ref_target(prop_schema).and_then(|r| {
        if r.starts_with("#/$defs/") || r.starts_with("#/definitions/") {
            Some(alloc::format!("#def-{}", ref_name(r)))
        } else {
            ctx.site.and_then(|s| ref_to_href(r, s))
        }
    });

    let nested_required = required_set(resolved);
    let nested = if ctx.depth < MAX_DEPTH {
        resolved
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| extract_properties(p, &nested_required, &ctx.deeper()))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let has_nested = !nested.is_empty();

    PropertyDoc {
        name: String::from(name),
        schema_type: ty,
        schema_type_parts,
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

/// Extract `examples` from a definition schema as display strings.
fn extract_raw_examples(schema: &Value) -> Vec<String> {
    schema
        .get("examples")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    if matches!(v, Value::Object(_) | Value::Array(_)) {
                        serde_json::to_string_pretty(v).unwrap_or_default()
                    } else {
                        format_value(v)
                    }
                })
                .collect()
        })
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

/// Extract a human-readable name from a `$ref` string.
///
/// - Fragment refs (`#/$defs/Foo`) → `Foo`
/// - Catalog URLs (`.../permission/latest.json`) → `Permission`
/// - Relative file refs (`./permission.json`) → `Permission`
fn ref_name(ref_str: &str) -> String {
    // If the ref has a fragment, extract the definition name from it.
    if let Some((_, fragment)) = ref_str.rsplit_once('#') {
        let name = fragment.rsplit('/').next().unwrap_or(fragment);
        return String::from(name);
    }

    // Strip common JSON schema suffixes to get the meaningful path segment.
    let path = ref_str
        .strip_suffix("/latest.json")
        .or_else(|| ref_str.strip_suffix(".json"))
        .unwrap_or(ref_str);

    let segment = path.rsplit('/').next().unwrap_or(path);
    // Strip leading "." for relative refs like "./permission"
    let segment = segment.strip_prefix('.').unwrap_or(segment);

    if segment.is_empty() {
        return String::from(ref_str);
    }

    title_case(segment)
}

/// Convert a kebab-case or `snake_case` segment to title case.
///
/// `"permission"` → `"Permission"`, `"some-rule"` → `"Some Rule"`.
fn title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '-' || c == '_' {
            result.push(' ');
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Walk through a schema (following into array items) to find the first `$ref` target.
fn find_ref_target(schema: &Value) -> Option<&str> {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        return Some(r);
    }
    if schema.get("type").and_then(Value::as_str) == Some("array")
        && let Some(items) = schema.get("items")
    {
        return find_ref_target(items);
    }
    None
}

/// Convert an external `$ref` URL to a site-relative href, if it belongs to this catalog.
///
/// Returns `None` for local `#/` refs (handled separately) and for refs outside the catalog.
fn ref_to_href(ref_str: &str, site: &SiteContext<'_>) -> Option<String> {
    // Only handle refs that point to the same catalog
    let relative = ref_str.strip_prefix(site.base_url)?;
    let path = relative
        .strip_suffix("latest.json")
        .or_else(|| relative.strip_suffix(".json"))
        .unwrap_or(relative);
    let path = path.trim_end_matches('/');
    Some(alloc::format!("{}{path}/", site.base_path))
}

/// Build individually-linkable type parts for schemas whose type comes from `oneOf`/`anyOf`.
///
/// Returns an empty vec when the schema has an explicit `type` field or no composition.
fn compute_type_parts(schema: &Value, site: Option<&SiteContext<'_>>) -> Vec<TypePart> {
    if schema.get("type").is_some() {
        return Vec::new();
    }
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let parts: Vec<TypePart> = variants
                .iter()
                .filter_map(|v| {
                    let text = schema_type_str(v)
                        .or_else(|| v.get("$ref").and_then(Value::as_str).map(ref_name))?;
                    let href = resolve_type_part_href(v, site);
                    Some(TypePart { text, href })
                })
                .collect();
            if !parts.is_empty() {
                return parts;
            }
        }
    }
    Vec::new()
}

/// Resolve a link target for a single type part variant.
///
/// Checks direct `$ref` first, then walks into array `items`. Handles both
/// local definition refs (`#/$defs/Foo` → `#def-Foo`) and external catalog
/// refs (via `ref_to_href`).
fn resolve_type_part_href(variant: &Value, site: Option<&SiteContext<'_>>) -> Option<String> {
    let ref_str = find_ref_target(variant)?;
    if ref_str.starts_with("#/$defs/") || ref_str.starts_with("#/definitions/") {
        Some(alloc::format!("#def-{}", ref_name(ref_str)))
    } else {
        site.and_then(|s| ref_to_href(ref_str, s))
    }
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
        return Some(ref_name(r));
    }
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(Value::as_array) {
            let types: Vec<String> = variants
                .iter()
                .filter_map(|v| {
                    schema_type_str(v)
                        .or_else(|| v.get("$ref").and_then(Value::as_str).map(ref_name))
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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
        let doc = extract_schema_doc(&schema, None);
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

    #[test]
    fn ref_name_local_fragment() {
        assert_eq!(ref_name("#/$defs/Foo"), "Foo");
        assert_eq!(ref_name("#/definitions/Bar"), "Bar");
    }

    #[test]
    fn ref_name_catalog_url() {
        assert_eq!(
            ref_name("https://catalog.lintel.tools/schemas/claude-code/permission/latest.json"),
            "Permission"
        );
    }

    #[test]
    fn ref_name_relative_file() {
        assert_eq!(ref_name("./permission.json"), "Permission");
        assert_eq!(ref_name("./some-rule.json"), "Some Rule");
    }

    #[test]
    fn ref_name_with_fragment_and_file() {
        assert_eq!(ref_name("./rule.json#/$defs/MyRule"), "MyRule");
    }

    #[test]
    fn ref_anchor_follows_array_items_ref() {
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/Thing" }
                }
            },
            "$defs": {
                "Thing": { "type": "string" }
            }
        });
        let doc = extract_schema_doc(&schema, None);
        assert_eq!(doc.properties[0].ref_anchor.as_deref(), Some("#def-Thing"));
    }

    #[test]
    fn external_ref_produces_correct_type_display() {
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                {
                    "type": "array",
                    "items": { "$ref": "https://example.com/schemas/tool/permission/latest.json" }
                }
            ]
        });
        let doc = extract_schema_doc(&schema, None);
        // The oneOf type string should show "Permission[]" not "latest.json[]"
        assert_eq!(doc.schema_type.as_deref(), Some("string | Permission[]"));
    }

    #[test]
    fn ref_anchor_includes_hash_for_local_refs() {
        let schema = json!({
            "type": "object",
            "properties": {
                "item": { "$ref": "#/$defs/Item" }
            },
            "$defs": {
                "Item": { "type": "string" }
            }
        });
        let doc = extract_schema_doc(&schema, None);
        assert_eq!(doc.properties[0].ref_anchor.as_deref(), Some("#def-Item"));
    }

    #[test]
    fn ref_anchor_resolves_external_catalog_link() {
        let site = SiteContext {
            base_url: "https://example.com/",
            base_path: "/",
        };
        let schema = json!({
            "type": "object",
            "properties": {
                "perm": { "$ref": "https://example.com/schemas/tool/permission/latest.json" }
            }
        });
        let doc = extract_schema_doc(&schema, Some(&site));
        assert_eq!(
            doc.properties[0].ref_anchor.as_deref(),
            Some("/schemas/tool/permission/")
        );
    }

    #[test]
    fn type_parts_link_to_local_definitions() {
        let schema = json!({
            "anyOf": [
                { "$ref": "#/$defs/Foo" },
                { "$ref": "#/$defs/Bar" }
            ],
            "$defs": {
                "Foo": { "type": "string", "title": "Foo" },
                "Bar": { "type": "integer", "title": "Bar" }
            }
        });
        let doc = extract_schema_doc(&schema, None);
        assert_eq!(doc.type_parts.len(), 2);
        assert_eq!(doc.type_parts[0].text, "Foo");
        assert_eq!(doc.type_parts[0].href.as_deref(), Some("#def-Foo"));
        assert_eq!(doc.type_parts[1].text, "Bar");
        assert_eq!(doc.type_parts[1].href.as_deref(), Some("#def-Bar"));
    }

    #[test]
    fn variant_ref_href_for_external_ref() {
        let site = SiteContext {
            base_url: "https://example.com/",
            base_path: "/",
        };
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                {
                    "type": "array",
                    "items": { "$ref": "https://example.com/schemas/tool/permission/latest.json" }
                }
            ]
        });
        let doc = extract_schema_doc(&schema, Some(&site));
        assert!(doc.compositions[0].variants[0].ref_href.is_none());
        assert_eq!(
            doc.compositions[0].variants[1].ref_href.as_deref(),
            Some("/schemas/tool/permission/")
        );
    }
}
