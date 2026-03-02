use serde_json::Value;

/// Meta-data vocabulary — annotations (title, description, etc.).
///
/// See [JSON Schema Validation §9](https://json-schema.org/draft/2020-12/json-schema-validation#section-9).
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
pub struct MetaDataVocabulary {
    /// The `title` keyword — short summary annotation.
    ///
    /// The value of this keyword MUST be a string.
    ///
    /// Both `"title"` and `"description"` can be used to decorate a
    /// user interface with information about the data produced by this
    /// user interface. A title will preferably be short, whereas a
    /// description will provide explanation about the purpose of the
    /// instance described by this schema.
    ///
    /// See [JSON Schema Validation §9.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// The `description` keyword — explanatory annotation.
    ///
    /// The value of this keyword MUST be a string.
    ///
    /// Both `"title"` and `"description"` can be used to decorate a
    /// user interface with information about the data produced by this
    /// user interface. A title will preferably be short, whereas a
    /// description will provide explanation about the purpose of the
    /// instance described by this schema.
    ///
    /// See [JSON Schema Validation §9.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The `default` keyword — default value annotation.
    ///
    /// There are no restrictions placed on the value of this keyword.
    /// When multiple occurrences of this keyword are applicable to a
    /// single sub-instance, implementations SHOULD remove duplicates.
    ///
    /// This keyword can be used to supply a default JSON value
    /// associated with a particular schema. It is RECOMMENDED that a
    /// default value be valid against the associated schema.
    ///
    /// See [JSON Schema Validation §9.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// The `deprecated` keyword — deprecation annotation.
    ///
    /// The value of this keyword MUST be a boolean. When multiple
    /// occurrences of this keyword are applicable to a single
    /// sub-instance, applications SHOULD consider the instance location
    /// to be deprecated if any occurrence specifies a true value.
    ///
    /// If `"deprecated"` has a value of boolean true, it indicates that
    /// applications SHOULD refrain from usage of the declared property.
    /// It MAY mean the property is going to be removed in the future.
    ///
    /// A root schema containing `"deprecated"` with a value of true
    /// indicates that the entire resource being described MAY be removed
    /// in the future.
    ///
    /// The `"deprecated"` keyword applies to each instance location to
    /// which the schema object containing the keyword successfully
    /// applies. This can result in scenarios where every array item or
    /// object property is deprecated even though the containing array
    /// or object is not.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.3).
    #[serde(default, skip_serializing_if = "crate::schema::is_false")]
    #[schemars(extend("default" = false))]
    pub deprecated: bool,

    /// The `readOnly` keyword — read-only annotation.
    ///
    /// The value of this keyword MUST be a boolean. When multiple
    /// occurrences of this keyword are applicable to a single
    /// sub-instance, the resulting behavior SHOULD be as for a true
    /// value if any occurrence specifies a true value, and SHOULD be as
    /// for a false value otherwise.
    ///
    /// If `"readOnly"` has a value of boolean true, it indicates that
    /// the value of the instance is managed exclusively by the owning
    /// authority, and attempts by an application to modify the value of
    /// this property are expected to be ignored or rejected by that
    /// owning authority.
    ///
    /// An instance document that is marked as `"readOnly"` for the
    /// entire document MAY be ignored if sent to the owning authority,
    /// or MAY result in an error, at the authority's discretion.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.4).
    #[serde(
        default,
        rename = "readOnly",
        skip_serializing_if = "crate::schema::is_false"
    )]
    #[schemars(extend("default" = false))]
    pub read_only: bool,

    /// The `writeOnly` keyword — write-only annotation.
    ///
    /// The value of this keyword MUST be a boolean.
    ///
    /// If `"writeOnly"` has a value of boolean true, it indicates that
    /// the value is never present when the instance is retrieved from
    /// the owning authority. It can be present when sent to the owning
    /// authority to update or create the document (or the resource it
    /// represents), but it will not be included in any updated or newly
    /// created version of the instance.
    ///
    /// An instance document that is marked as `"writeOnly"` for the
    /// entire document MAY be returned as a blank document of some
    /// sort, or MAY produce an error upon retrieval, or have the
    /// retrieval request ignored, at the authority's discretion.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.4).
    #[serde(
        default,
        rename = "writeOnly",
        skip_serializing_if = "crate::schema::is_false"
    )]
    #[schemars(extend("default" = false))]
    pub write_only: bool,

    /// The `examples` keyword — example values annotation.
    ///
    /// The value of this keyword MUST be an array. There are no
    /// restrictions placed on the values within the array. When
    /// multiple occurrences of this keyword are applicable to a single
    /// sub-instance, implementations MUST provide a flat array of all
    /// values rather than an array of arrays.
    ///
    /// This keyword can be used to provide sample JSON values
    /// associated with a particular schema, for the purpose of
    /// illustrating usage. It is RECOMMENDED that these values be valid
    /// against the associated schema.
    ///
    /// Implementations MAY use the value(s) of `"default"`, if present,
    /// as an additional example. If `"examples"` is absent, `"default"`
    /// MAY still be used in this manner.
    ///
    /// See [JSON Schema Validation §9.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<Value>>,
}
