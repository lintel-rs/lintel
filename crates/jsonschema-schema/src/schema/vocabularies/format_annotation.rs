/// Format-annotation vocabulary.
///
/// See [JSON Schema Validation §7](https://json-schema.org/draft/2020-12/json-schema-validation#section-7).
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
pub struct FormatAnnotationVocabulary {
    /// The `format` keyword — semantic format annotation.
    ///
    /// Structural validation alone may be insufficient to allow an
    /// application to correctly utilize certain values. The `"format"`
    /// annotation keyword is defined to allow schema authors to convey
    /// semantic information for a fixed subset of values which are
    /// accurately described by authoritative resources, be they RFCs or
    /// other external specifications.
    ///
    /// The value of this keyword is called a format attribute. It MUST
    /// be a string.
    ///
    /// See [JSON Schema Validation §7](https://json-schema.org/draft/2020-12/json-schema-validation#section-7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}
