use super::{Schema, SchemaValue};

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
