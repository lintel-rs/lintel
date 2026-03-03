use indexmap::IndexMap;

use crate::SchemaValue;

/// Applicator vocabulary — composition, conditionals, and object/array subschemas.
///
/// See [JSON Schema Core §10](https://json-schema.org/draft/2020-12/json-schema-core#section-10).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    combine_structs::Fields,
)]
pub struct ApplicatorVocabulary {
    /// The `properties` keyword — per-property subschemas.
    ///
    /// The value of `"properties"` MUST be an object. Each value of
    /// this object MUST be a valid JSON Schema.
    ///
    /// Validation succeeds if, for each name that appears in both the
    /// instance and as a name within this keyword's value, the child
    /// instance for that name successfully validates against the
    /// corresponding schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names matched by this keyword. This annotation affects
    /// the behavior of `"additionalProperties"` (in this vocabulary)
    /// and `"unevaluatedProperties"` in the Unevaluated vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty object.
    ///
    /// See [JSON Schema Core §10.3.2.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.1).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[schemars(extend("default" = {}))]
    pub properties: IndexMap<String, SchemaValue>,

    /// The `patternProperties` keyword — regex-matched property
    /// subschemas.
    ///
    /// The value of `"patternProperties"` MUST be an object. Each
    /// property name of this object SHOULD be a valid regular
    /// expression, according to the ECMA-262 regular expression
    /// dialect. Each property value of this object MUST be a valid
    /// JSON Schema.
    ///
    /// Validation succeeds if, for each instance name that matches any
    /// regular expressions that appear as a property name in this
    /// keyword's value, the child instance for that name successfully
    /// validates against each schema that corresponds to a matching
    /// regular expression.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names matched by this keyword. This annotation affects
    /// the behavior of `"additionalProperties"` (in this vocabulary)
    /// and `"unevaluatedProperties"` (in the Unevaluated vocabulary).
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty object.
    ///
    /// See [JSON Schema Core §10.3.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.2).
    #[serde(
        default,
        rename = "patternProperties",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    #[schemars(extend("default" = {}), extend("propertyNames" = { "format": "regex" }))]
    pub pattern_properties: IndexMap<String, SchemaValue>,

    /// The `additionalProperties` keyword — schema for unmatched
    /// properties.
    ///
    /// The value of `"additionalProperties"` MUST be a valid JSON
    /// Schema.
    ///
    /// The behavior of this keyword depends on the presence and
    /// annotation results of `"properties"` and `"patternProperties"`
    /// within the same schema object. Validation with
    /// `"additionalProperties"` applies only to the child values of
    /// instance names that do not appear in the annotation results of
    /// either `"properties"` or `"patternProperties"`.
    ///
    /// For all such properties, validation succeeds if the child
    /// instance validates against the `"additionalProperties"` schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names validated by this keyword's subschema. This
    /// annotation affects the behavior of `"unevaluatedProperties"` in
    /// the Unevaluated vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §10.3.2.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.3).
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<Box<SchemaValue>>,

    /// The `propertyNames` keyword — property name schema.
    ///
    /// The value of `"propertyNames"` MUST be a valid JSON Schema.
    ///
    /// If the instance is an object, this keyword validates if every
    /// property name in the instance validates against the provided
    /// schema. Note the property name that the schema is testing will
    /// always be a string.
    ///
    /// Omitting this keyword has the same behavior as an empty schema.
    ///
    /// See [JSON Schema Core §10.3.2.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.4).
    #[serde(rename = "propertyNames", skip_serializing_if = "Option::is_none")]
    pub property_names: Option<Box<SchemaValue>>,

    /// The `dependentSchemas` keyword — conditional subschemas by
    /// property name.
    ///
    /// This keyword specifies subschemas that are evaluated if the
    /// instance is an object and contains a certain property.
    ///
    /// This keyword's value MUST be an object. Each value in the
    /// object MUST be a valid JSON Schema.
    ///
    /// If the object key is a property in the instance, the entire
    /// instance must validate against the subschema. Its use is
    /// dependent on the presence of the property.
    ///
    /// Omitting this keyword has the same behavior as an empty object.
    ///
    /// See [JSON Schema Core §10.2.2.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.4).
    #[serde(
        default,
        rename = "dependentSchemas",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    #[schemars(extend("default" = {}))]
    pub dependent_schemas: IndexMap<String, SchemaValue>,

    /// The `items` keyword — schema for remaining array items.
    ///
    /// The value of `"items"` MUST be a valid JSON Schema.
    ///
    /// This keyword applies its subschema to all instance elements at
    /// indexes greater than the length of the `"prefixItems"` array in
    /// the same schema object, as reported by the annotation result of
    /// that `"prefixItems"` keyword. If no such annotation result
    /// exists, `"items"` applies its subschema to all instance array
    /// elements. Note that the behavior of `"items"` without
    /// `"prefixItems"` is identical to that of the schema form of
    /// `"items"` in prior drafts. When `"prefixItems"` is present, the
    /// behavior of `"items"` is identical to the former
    /// `"additionalItems"` keyword.
    ///
    /// If the `"items"` subschema is applied to any positions within
    /// the instance array, it produces an annotation result of boolean
    /// true, indicating that all remaining array elements have been
    /// evaluated against this keyword's subschema. This annotation
    /// affects the behavior of `"unevaluatedItems"` in the Unevaluated
    /// vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §10.3.1.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<SchemaValue>>,

    /// The `prefixItems` keyword — positional array item schemas.
    ///
    /// The value of `"prefixItems"` MUST be a non-empty array of valid
    /// JSON Schemas.
    ///
    /// Validation succeeds if each element of the instance validates
    /// against the schema at the same position, if any. This keyword
    /// does not constrain the length of the array. If the array is
    /// longer than this keyword's value, this keyword validates only
    /// the prefix of matching length.
    ///
    /// This keyword produces an annotation value which is the largest
    /// index to which this keyword applied a subschema. The value MAY
    /// be a boolean true if a subschema was applied to every index of
    /// the instance, such as is produced by the `"items"` keyword.
    /// This annotation affects the behavior of `"items"` and
    /// `"unevaluatedItems"`.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty array.
    ///
    /// See [JSON Schema Core §10.3.1.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.1).
    #[serde(rename = "prefixItems", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub prefix_items: Option<Vec<SchemaValue>>,

    /// The `contains` keyword — array containment schema.
    ///
    /// The value of this keyword MUST be a valid JSON Schema.
    ///
    /// An array instance is valid against `"contains"` if at least one
    /// of its elements is valid against the given schema, except when
    /// `"minContains"` is present and has a value of 0, in which case
    /// an array instance MUST be considered valid against the
    /// `"contains"` keyword, even if none of its elements is valid
    /// against the given schema.
    ///
    /// This keyword produces an annotation value which is an array of
    /// the indexes to which this keyword validates successfully when
    /// applying its subschema, in ascending order. The value MAY be a
    /// boolean `true` if the subschema validates successfully when
    /// applied to every index of the instance.
    ///
    /// The subschema MUST be applied to every array element even after
    /// the first match has been found, in order to collect annotations
    /// for use by other keywords. This is to ensure that all possible
    /// annotations are collected.
    ///
    /// See [JSON Schema Core §10.3.1.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<Box<SchemaValue>>,

    /// The `allOf` keyword — conjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against all schemas defined by this
    /// keyword's value.
    ///
    /// See [JSON Schema Core §10.2.1.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.1).
    #[serde(rename = "allOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub all_of: Option<Vec<SchemaValue>>,

    /// The `anyOf` keyword — disjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against at least one schema defined by
    /// this keyword's value. Note that when annotations are being
    /// collected, all subschemas MUST be examined so that annotations
    /// are collected from each subschema that validates successfully.
    ///
    /// See [JSON Schema Core §10.2.1.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.2).
    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub any_of: Option<Vec<SchemaValue>>,

    /// The `oneOf` keyword — exclusive disjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against exactly one schema defined by
    /// this keyword's value.
    ///
    /// See [JSON Schema Core §10.2.1.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.3).
    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub one_of: Option<Vec<SchemaValue>>,

    /// The `not` keyword — negation.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// An instance is valid against this keyword if it fails to
    /// validate successfully against the schema defined by this
    /// keyword.
    ///
    /// See [JSON Schema Core §10.2.1.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<SchemaValue>>,

    /// The `if` keyword — conditional guard.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// This validation outcome of this keyword's subschema has no
    /// direct effect on the overall validation result. Rather, it
    /// controls which of the `"then"` or `"else"` keywords are
    /// evaluated.
    ///
    /// Instances that successfully validate against this keyword's
    /// subschema MUST also be valid against the subschema value of the
    /// `"then"` keyword, if present.
    ///
    /// Instances that fail to validate against this keyword's
    /// subschema MUST also be valid against the subschema value of the
    /// `"else"` keyword, if present.
    ///
    /// If annotations are being collected, they are collected from
    /// this keyword's subschema in the usual way, including when the
    /// keyword is present without either `"then"` or `"else"`.
    ///
    /// See [JSON Schema Core §10.2.2.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.1).
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_: Option<Box<SchemaValue>>,

    /// The `then` keyword — consequent subschema.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// When `"if"` is present, and the instance successfully validates
    /// against its subschema, then validation succeeds against this
    /// keyword if the instance also successfully validates against
    /// this keyword's subschema.
    ///
    /// This keyword has no effect when `"if"` is absent, or when the
    /// instance fails to validate against its subschema.
    /// Implementations MUST NOT evaluate the instance against this
    /// keyword, for either validation or annotation collection
    /// purposes, in such cases.
    ///
    /// See [JSON Schema Core §10.2.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.2).
    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_: Option<Box<SchemaValue>>,

    /// The `else` keyword — alternative subschema.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// When `"if"` is present, and the instance fails to validate
    /// against its subschema, then validation succeeds against this
    /// keyword if the instance successfully validates against this
    /// keyword's subschema.
    ///
    /// This keyword has no effect when `"if"` is absent, or when the
    /// instance successfully validates against its subschema.
    /// Implementations MUST NOT evaluate the instance against this
    /// keyword, for either validation or annotation collection
    /// purposes, in such cases.
    ///
    /// See [JSON Schema Core §10.2.2.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.3).
    #[serde(rename = "else", skip_serializing_if = "Option::is_none")]
    pub else_: Option<Box<SchemaValue>>,
}
