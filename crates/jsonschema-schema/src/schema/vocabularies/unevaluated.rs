use crate::SchemaValue;

/// Unevaluated vocabulary — `unevaluatedItems` and `unevaluatedProperties`.
///
/// See [JSON Schema Core §11](https://json-schema.org/draft/2020-12/json-schema-core#section-11).
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
pub struct UnevaluatedVocabulary {
    /// The `unevaluatedProperties` keyword — schema for properties not
    /// covered by other keywords.
    ///
    /// The value of `"unevaluatedProperties"` MUST be a valid JSON
    /// Schema.
    ///
    /// The behavior of this keyword depends on the annotation results
    /// of adjacent keywords that apply to the instance location being
    /// validated. Specifically, the annotations from `"properties"`,
    /// `"patternProperties"`, and `"additionalProperties"`, which can
    /// come from those keywords when they are adjacent to the
    /// `"unevaluatedProperties"` keyword. Those three annotations, as
    /// well as `"unevaluatedProperties"`, can also result from any and
    /// all adjacent in-place applicator keywords.
    ///
    /// Validation with `"unevaluatedProperties"` applies only to the
    /// child values of instance names that do not appear in the
    /// `"properties"`, `"patternProperties"`, `"additionalProperties"`,
    /// or `"unevaluatedProperties"` annotation results that apply to
    /// the instance location being validated.
    ///
    /// For all such properties, validation succeeds if the child
    /// instance validates against the `"unevaluatedProperties"` schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names validated by this keyword's subschema. This
    /// annotation affects the behavior of `"unevaluatedProperties"` in
    /// parent schemas.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §11.3](https://json-schema.org/draft/2020-12/json-schema-core#section-11.3).
    #[serde(
        rename = "unevaluatedProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub unevaluated_properties: Option<Box<SchemaValue>>,

    /// The `unevaluatedItems` keyword — schema for array items not
    /// covered by other keywords.
    ///
    /// The value of `"unevaluatedItems"` MUST be a valid JSON Schema.
    ///
    /// The behavior of this keyword depends on the annotation results
    /// of adjacent keywords that apply to the instance location being
    /// validated. Specifically, the annotations from `"prefixItems"`,
    /// `"items"`, and `"contains"`, which can come from those keywords
    /// when they are adjacent to the `"unevaluatedItems"` keyword.
    /// Those three annotations, as well as `"unevaluatedItems"`, can
    /// also result from any and all adjacent in-place applicator
    /// keywords.
    ///
    /// If no relevant annotations are present, the
    /// `"unevaluatedItems"` subschema MUST be applied to all locations
    /// in the array. If a boolean true value is present from any of
    /// the relevant annotations, `"unevaluatedItems"` MUST be ignored.
    /// Otherwise, the subschema MUST be applied to any index greater
    /// than the largest annotation value for `"prefixItems"`, which
    /// does not appear in any annotation value for `"contains"`.
    ///
    /// If the `"unevaluatedItems"` subschema is applied to any
    /// positions within the instance array, it produces an annotation
    /// result of boolean true, analogous to the behavior of `"items"`.
    /// This annotation affects the behavior of `"unevaluatedItems"` in
    /// parent schemas.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §11.2](https://json-schema.org/draft/2020-12/json-schema-core#section-11.2).
    #[serde(rename = "unevaluatedItems", skip_serializing_if = "Option::is_none")]
    pub unevaluated_items: Option<Box<SchemaValue>>,
}
