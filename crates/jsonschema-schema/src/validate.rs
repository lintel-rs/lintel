use crate::schema::{Schema, SchemaValue, navigate_pointer};

/// A structural validation error found in a schema.
#[derive(Debug, Clone)]
pub struct SchemaError {
    /// JSON Pointer to the error location (e.g. "/properties/item").
    pub path: String,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Validate structural integrity of a schema, returning all errors found.
pub fn validate(schema: &Schema) -> Vec<SchemaError> {
    let root = SchemaValue::Schema(Box::new(schema.clone()));
    let mut errors = Vec::new();
    validate_schema(schema, &root, "", &mut errors);
    errors
}

fn validate_schema(schema: &Schema, root: &SchemaValue, path: &str, errors: &mut Vec<SchemaError>) {
    // Check $ref
    if let Some(ref ref_str) = schema.ref_
        && let Some(ref_path) = ref_str.strip_prefix("#/")
        && navigate_pointer(root, root, ref_path).is_err()
    {
        errors.push(SchemaError {
            path: path.to_string(),
            message: format!("$ref \"{ref_str}\" does not resolve"),
        });
    }

    // Map fields (non-optional IndexMap)
    for (keyword, map) in [
        ("properties", &schema.properties),
        ("patternProperties", &schema.pattern_properties),
        ("dependentSchemas", &schema.dependent_schemas),
    ] {
        for (key, sv) in map {
            validate_value(sv, root, &format!("{path}/{keyword}/{key}"), errors);
        }
    }

    // $defs uses BTreeMap
    if let Some(ref defs) = schema.defs {
        for (key, sv) in defs {
            validate_value(sv, root, &format!("{path}/$defs/{key}"), errors);
        }
    }

    // Array fields
    for (keyword, arr) in [
        ("allOf", schema.all_of.as_ref()),
        ("anyOf", schema.any_of.as_ref()),
        ("oneOf", schema.one_of.as_ref()),
        ("prefixItems", schema.prefix_items.as_ref()),
    ] {
        if let Some(items) = arr {
            for (i, sv) in items.iter().enumerate() {
                validate_value(sv, root, &format!("{path}/{keyword}/{i}"), errors);
            }
        }
    }

    // Single fields
    for (keyword, field) in [
        ("items", schema.items.as_deref()),
        ("contains", schema.contains.as_deref()),
        (
            "additionalProperties",
            schema.additional_properties.as_deref(),
        ),
        ("propertyNames", schema.property_names.as_deref()),
        (
            "unevaluatedProperties",
            schema.unevaluated_properties.as_deref(),
        ),
        ("unevaluatedItems", schema.unevaluated_items.as_deref()),
        ("not", schema.not.as_deref()),
        ("if", schema.if_.as_deref()),
        ("then", schema.then_.as_deref()),
        ("else", schema.else_.as_deref()),
        ("contentSchema", schema.content_schema.as_deref()),
    ] {
        if let Some(sv) = field {
            validate_value(sv, root, &format!("{path}/{keyword}"), errors);
        }
    }
}

fn validate_value(sv: &SchemaValue, root: &SchemaValue, path: &str, errors: &mut Vec<SchemaError>) {
    if let Some(schema) = sv.as_schema() {
        validate_schema(schema, root, path, errors);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::schema::{SimpleType, TypeValue};
    use alloc::collections::BTreeMap;
    use indexmap::IndexMap;

    #[test]
    fn valid_schema_no_errors() {
        let item_schema = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single(SimpleType::String)),
            ..Default::default()
        }));
        let mut defs = BTreeMap::new();
        defs.insert("Item".into(), item_schema);

        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Item".into()),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("item".into(), ref_schema);

        let schema = Schema {
            defs: Some(defs),
            properties: props,
            ..Default::default()
        };

        let errors = validate(&schema);
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn missing_defs_target() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Missing".into()),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("item".into(), ref_schema);

        let schema = Schema {
            properties: props,
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/properties/item");
        assert!(errors[0].message.contains("$defs/Missing"));
    }

    #[test]
    fn nested_ref_in_properties() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Nonexistent".into()),
            ..Default::default()
        }));
        let mut inner_props = IndexMap::new();
        inner_props.insert("nested".into(), ref_schema);

        let wrapper = SchemaValue::Schema(Box::new(Schema {
            properties: inner_props,
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("wrapper".into(), wrapper);

        let schema = Schema {
            properties: props,
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/properties/wrapper/properties/nested");
    }

    #[test]
    fn external_ref_not_checked() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("https://example.com/schema.json".into()),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("item".into(), ref_schema);

        let schema = Schema {
            properties: props,
            ..Default::default()
        };

        let errors = validate(&schema);
        assert!(errors.is_empty(), "external $ref should not be checked");
    }

    #[test]
    fn ref_in_all_of() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Missing".into()),
            ..Default::default()
        }));

        let schema = Schema {
            all_of: Some(vec![ref_schema]),
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/allOf/0");
    }

    #[test]
    fn ref_in_any_of() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Missing".into()),
            ..Default::default()
        }));

        let schema = Schema {
            any_of: Some(vec![SchemaValue::Bool(true), ref_schema]),
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/anyOf/1");
    }

    #[test]
    fn ref_in_one_of() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Also Missing".into()),
            ..Default::default()
        }));

        let schema = Schema {
            one_of: Some(vec![ref_schema]),
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/oneOf/0");
    }

    #[test]
    fn deep_nesting_full_path() {
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Deep".into()),
            ..Default::default()
        }));

        let inner = SchemaValue::Schema(Box::new(Schema {
            items: Some(Box::new(ref_schema)),
            ..Default::default()
        }));

        let schema = Schema {
            all_of: Some(vec![inner]),
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "/allOf/0/items");
    }

    #[test]
    fn multiple_errors_collected() {
        let ref1 = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/A".into()),
            ..Default::default()
        }));
        let ref2 = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/B".into()),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("x".into(), ref1);
        props.insert("y".into(), ref2);

        let schema = Schema {
            properties: props,
            ..Default::default()
        };

        let errors = validate(&schema);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn validate_method_on_schema() {
        let schema = Schema {
            all_of: Some(vec![SchemaValue::Schema(Box::new(Schema {
                ref_: Some("#/$defs/Nope".into()),
                ..Default::default()
            }))]),
            ..Default::default()
        };

        let errors = schema.validate();
        assert_eq!(errors.len(), 1);
    }
}
