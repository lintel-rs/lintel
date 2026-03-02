use crate::SchemaValue;

/// Content vocabulary — `contentMediaType`, `contentEncoding`,
/// `contentSchema`.
///
/// See [JSON Schema Validation §8](https://json-schema.org/draft/2020-12/json-schema-validation#section-8).
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
pub struct ContentVocabulary {
    /// The `contentMediaType` keyword — media type of string content.
    ///
    /// If the instance is a string, this property indicates the media
    /// type of the contents of the string. If `"contentEncoding"` is
    /// present, this property describes the decoded string.
    ///
    /// The value of this property MUST be a string, which MUST be a
    /// media type, as defined by RFC 2046.
    ///
    /// See [JSON Schema Validation §8.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.4).
    #[serde(rename = "contentMediaType", skip_serializing_if = "Option::is_none")]
    pub content_media_type: Option<String>,

    /// The `contentEncoding` keyword — encoding of string content.
    ///
    /// If the instance value is a string, this property defines that
    /// the string SHOULD be interpreted as encoded binary data and
    /// decoded using the encoding named by this property.
    ///
    /// Possible values indicating base 16, 32, and 64 encodings with
    /// several variations are listed in RFC 4648. Additionally,
    /// sections 6.7 and 6.8 of RFC 2045 provide encodings used in
    /// MIME.
    ///
    /// If this keyword is absent, but `"contentMediaType"` is present,
    /// this indicates that the encoding is the identity encoding,
    /// meaning that no transformation was needed in order to represent
    /// the content in a UTF-8 string.
    ///
    /// The value of this property MUST be a string.
    ///
    /// See [JSON Schema Validation §8.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.3).
    #[serde(rename = "contentEncoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,

    /// The `contentSchema` keyword — schema for decoded string content.
    ///
    /// If the instance is a string, and if `"contentMediaType"` is
    /// present, this property contains a schema which describes the
    /// structure of the string.
    ///
    /// This keyword MAY be used with any media type that can be mapped
    /// into JSON Schema's data model.
    ///
    /// The value of this property MUST be a valid JSON schema. It
    /// SHOULD be ignored if `"contentMediaType"` is not present.
    ///
    /// See [JSON Schema Validation §8.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.5).
    #[serde(rename = "contentSchema", skip_serializing_if = "Option::is_none")]
    pub content_schema: Option<Box<SchemaValue>>,
}
