use alloc::collections::{BTreeMap, BTreeSet};

use crate::schema::{Schema, SchemaValue, navigate_pointer};

/// Flatten `allOf` entries into the root schema.
///
/// Clones the schema, resolves each `allOf` entry, merges its properties into
/// the root using `Schema`'s `Add` implementation (left-bias), and replaces
/// inline entries with `$ref` pointers into `$defs`.
///
/// The returned schema keeps `allOf` (now all `$ref` entries) so the ALL OF
/// section shows what was composed, while PROPERTIES shows the merged view.
pub fn flatten_all_of(schema: &Schema, root: &SchemaValue) -> Schema {
    let mut merged = schema.clone();
    let Some(all_of) = merged.all_of.take() else {
        return merged;
    };

    let mut new_all_of = Vec::new();

    for entry in all_of {
        let is_ref = entry.as_schema().is_some_and(|s| s.ref_.is_some());

        // Resolve $ref against the original root (before mutations)
        let resolved = if is_ref {
            let resolved_sv = resolve_entry_in_root(&entry, root);
            let Some(s) = resolved_sv.as_schema() else {
                new_all_of.push(entry);
                continue;
            };
            s.clone()
        } else {
            let Some(s) = entry.as_schema() else {
                new_all_of.push(entry);
                continue;
            };
            s.clone()
        };

        // Keep original entry in allOf for provenance
        new_all_of.push(entry);

        // Merge resolved properties into root (left-bias)
        let mut clean = resolved;
        // Don't carry over composition keywords
        clean.all_of = None;
        clean.any_of = None;
        clean.one_of = None;
        // Don't carry over identity fields from sub-schemas
        clean.schema = None;
        clean.id = None;
        clean.title = None;
        clean.description = None;
        clean.markdown_description = None;
        clean.x_lintel = None;

        merged = merged + clean;
    }

    merged.all_of = Some(new_all_of);

    // Remove $defs entries that are no longer referenced by any $ref
    if let Some(defs) = merged.defs.take() {
        let refs = collect_ref_targets(&merged, &defs);
        let pruned: BTreeMap<_, _> = defs
            .into_iter()
            .filter(|(name, _)| refs.contains(name))
            .collect();
        merged.defs = if pruned.is_empty() {
            None
        } else {
            Some(pruned)
        };
    }

    merged
}

/// Collect all `$defs` names that are referenced by `$ref` pointers in the schema.
///
/// Walks the schema tree (excluding `$defs` itself) and extracts the def name
/// from any `$ref` matching `#/$defs/<name>`.
fn collect_ref_targets(schema: &Schema, defs: &BTreeMap<String, SchemaValue>) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    collect_refs_in_schema(schema, &mut targets, true);
    // Transitively walk within referenced defs — a def may reference another def
    let mut changed = true;
    while changed {
        changed = false;
        for (name, sv) in defs {
            if targets.contains(name.as_str()) {
                let prev_len = targets.len();
                collect_refs_in_value(sv, &mut targets);
                changed |= targets.len() > prev_len;
            }
        }
    }
    targets
}

fn collect_refs_in_schema(schema: &Schema, targets: &mut BTreeSet<String>, skip_defs: bool) {
    if let Some(ref r) = schema.ref_
        && let Some(name) = extract_def_name(r)
    {
        targets.insert(name.to_string());
    }

    // Map fields (non-optional IndexMap)
    for sv in schema.properties.values() {
        collect_refs_in_value(sv, targets);
    }
    for sv in schema.pattern_properties.values() {
        collect_refs_in_value(sv, targets);
    }
    for sv in schema.dependent_schemas.values() {
        collect_refs_in_value(sv, targets);
    }

    // $defs — only walk when not skipping (i.e. when called recursively from within a def)
    if !skip_defs && let Some(ref defs) = schema.defs {
        for sv in defs.values() {
            collect_refs_in_value(sv, targets);
        }
    }

    // Array fields — skip allOf since those refs are already merged
    for arr in [
        schema.any_of.as_ref(),
        schema.one_of.as_ref(),
        schema.prefix_items.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        for sv in arr {
            collect_refs_in_value(sv, targets);
        }
    }

    // Single fields
    for sv in [
        schema.items.as_deref(),
        schema.contains.as_deref(),
        schema.additional_properties.as_deref(),
        schema.property_names.as_deref(),
        schema.unevaluated_properties.as_deref(),
        schema.unevaluated_items.as_deref(),
        schema.not.as_deref(),
        schema.if_.as_deref(),
        schema.then_.as_deref(),
        schema.else_.as_deref(),
        schema.content_schema.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        collect_refs_in_value(sv, targets);
    }
}

/// Extract a `$defs` name from a `$ref` string.
///
/// Handles both local (`#/$defs/Foo`) and absolute (`https://…#/$defs/Foo`) refs.
fn extract_def_name(ref_str: &str) -> Option<&str> {
    // Local ref
    if let Some(name) = ref_str.strip_prefix("#/$defs/") {
        return Some(name);
    }
    // Absolute URL with fragment
    let fragment = ref_str.split_once('#')?.1;
    fragment.strip_prefix("/$defs/")
}

fn collect_refs_in_value(sv: &SchemaValue, targets: &mut BTreeSet<String>) {
    if let Some(schema) = sv.as_schema() {
        collect_refs_in_schema(schema, targets, false);
    }
}

/// Resolve a `$ref` entry against the root schema.
///
/// Handles both local (`#/…`) and absolute (`https://…#/…`) refs by
/// extracting the fragment portion and navigating the root.
fn resolve_entry_in_root<'a>(entry: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
    let Some(schema) = entry.as_schema() else {
        return entry;
    };
    let Some(ref ref_str) = schema.ref_ else {
        return entry;
    };
    let fragment = if let Some(path) = ref_str.strip_prefix('#') {
        path
    } else if let Some(pos) = ref_str.find('#') {
        &ref_str[pos + 1..]
    } else {
        return entry;
    };
    if let Ok(resolved) = navigate_pointer(root, root, fragment) {
        return resolved;
    }
    entry
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sv(val: serde_json::Value) -> SchemaValue {
        serde_json::from_value(val).unwrap()
    }

    fn schema(val: serde_json::Value) -> Schema {
        serde_json::from_value(val).unwrap()
    }

    #[test]
    fn no_allof_returns_unchanged() {
        let s = schema(json!({"type": "object", "title": "Root"}));
        let root = sv(json!({"type": "object", "title": "Root"}));
        let result = flatten_all_of(&s, &root);
        assert!(result.all_of.is_none());
        assert_eq!(result.title.as_deref(), Some("Root"));
    }

    #[test]
    fn merges_inline_allof_properties() {
        let val = json!({
            "type": "object",
            "properties": {
                "a": { "type": "string" }
            },
            "allOf": [
                {
                    "title": "Extra",
                    "properties": {
                        "b": { "type": "integer" }
                    }
                }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        // Properties merged
        let props = result.properties;
        assert!(props.contains_key("a"));
        assert!(props.contains_key("b"));

        // allOf kept with original inline entry
        let all_of = result.all_of.unwrap();
        assert_eq!(all_of.len(), 1);
        let entry = all_of[0].as_schema().unwrap();
        assert_eq!(entry.title.as_deref(), Some("Extra"));
    }

    #[test]
    fn merges_ref_allof() {
        let val = json!({
            "type": "object",
            "allOf": [
                { "$ref": "#/$defs/Base" }
            ],
            "$defs": {
                "Base": {
                    "title": "Base Schema",
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let props = result.properties;
        assert!(props.contains_key("name"));

        // allOf kept as original $ref
        let all_of = result.all_of.unwrap();
        let ref_str = all_of[0].as_schema().unwrap().ref_.as_deref();
        assert_eq!(ref_str, Some("#/$defs/Base"));

        // Def pruned — only referenced from allOf (already merged)
        assert!(result.defs.is_none());
    }

    #[test]
    fn root_properties_win_over_allof() {
        let val = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string" }
            },
            "allOf": [
                {
                    "properties": {
                        "x": { "type": "integer" }
                    }
                }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let props = result.properties;
        let x_schema = props["x"].as_schema().unwrap();
        assert!(matches!(
            x_schema.type_,
            Some(crate::schema::TypeValue::Single(
                crate::schema::SimpleType::String
            ))
        ));
    }

    #[test]
    fn required_union() {
        let val = json!({
            "required": ["a"],
            "allOf": [
                { "required": ["b", "a"] },
                { "required": ["c"] }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        let req = result.required.unwrap();
        assert!(req.contains(&"a".to_string()));
        assert!(req.contains(&"b".to_string()));
        assert!(req.contains(&"c".to_string()));
        assert_eq!(req.len(), 3);
    }

    #[test]
    fn inline_without_title_kept_as_is() {
        let val = json!({
            "allOf": [
                { "properties": { "x": { "type": "string" } } }
            ]
        });
        let s = schema(val.clone());
        let root = sv(val);
        let result = flatten_all_of(&s, &root);

        // Inline entry kept in allOf, properties merged
        let all_of = result.all_of.unwrap();
        assert_eq!(all_of.len(), 1);
        assert!(result.properties.contains_key("x"));
        assert!(result.defs.is_none());
    }
}
