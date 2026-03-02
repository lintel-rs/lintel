use indexmap::IndexMap;
use serde_json::{Number, Value};

use crate::TypeValue;

/// Validation vocabulary — type checks and numeric, string, array,
/// and object constraints.
///
/// See [JSON Schema Validation §6](https://json-schema.org/draft/2020-12/json-schema-validation#section-6).
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
pub struct ValidationVocabulary {
    // -- Type (§6.1) --
    /// The `type` keyword — instance type constraint.
    ///
    /// The value of this keyword MUST be either a string or an array.
    /// If it is an array, elements of the array MUST be strings and
    /// MUST be unique.
    ///
    /// String values MUST be one of the six primitive types (`"null"`,
    /// `"boolean"`, `"object"`, `"array"`, `"number"`, or `"string"`),
    /// or `"integer"` which matches any number with a zero fractional
    /// part.
    ///
    /// If the value of `"type"` is a string, then an instance validates
    /// successfully if its type matches the type represented by the
    /// value of the string. If the value of `"type"` is an array, then
    /// an instance validates successfully if its type matches any of the
    /// types indicated by the strings in the array.
    ///
    /// See [JSON Schema Validation §6.1.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.1).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<TypeValue>,

    /// The `enum` keyword — enumerated values constraint.
    ///
    /// The value of this keyword MUST be an array. This array SHOULD
    /// have at least one element. Elements in the array SHOULD be
    /// unique.
    ///
    /// An instance validates successfully against this keyword if its
    /// value is equal to one of the elements in this keyword's array
    /// value.
    ///
    /// Elements in the array might be of any type, including null.
    ///
    /// See [JSON Schema Validation §6.1.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.2).
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_: Option<Vec<Value>>,

    /// The `const` keyword — constant value constraint.
    ///
    /// The value of this keyword MAY be of any type, including null.
    ///
    /// Use of this keyword is functionally equivalent to an `"enum"`
    /// (Section 6.1.2) with a single value.
    ///
    /// An instance validates successfully against this keyword if its
    /// value is equal to the value of the keyword.
    ///
    /// See [JSON Schema Validation §6.1.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.3).
    #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
    pub const_: Option<Value>,

    // -- Numeric (§6.2) --
    /// The `minimum` keyword — inclusive lower bound.
    ///
    /// The value of `"minimum"` MUST be a number, representing an
    /// inclusive lower limit for a numeric instance.
    ///
    /// If the instance is a number, then this keyword validates only if
    /// the instance is greater than or exactly equal to `"minimum"`.
    ///
    /// See [JSON Schema Validation §6.2.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<Number>,

    /// The `maximum` keyword — inclusive upper bound.
    ///
    /// The value of `"maximum"` MUST be a number, representing an
    /// inclusive upper limit for a numeric instance.
    ///
    /// If the instance is a number, then this keyword validates only if
    /// the instance is less than or exactly equal to `"maximum"`.
    ///
    /// See [JSON Schema Validation §6.2.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<Number>,

    /// The `exclusiveMinimum` keyword — exclusive lower bound.
    ///
    /// The value of `"exclusiveMinimum"` MUST be a number, representing
    /// an exclusive lower limit for a numeric instance.
    ///
    /// If the instance is a number, then the instance is valid only if
    /// it has a value strictly greater than (not equal to)
    /// `"exclusiveMinimum"`.
    ///
    /// See [JSON Schema Validation §6.2.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.5).
    #[serde(rename = "exclusiveMinimum", skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<Number>,

    /// The `exclusiveMaximum` keyword — exclusive upper bound.
    ///
    /// The value of `"exclusiveMaximum"` MUST be a number, representing
    /// an exclusive upper limit for a numeric instance.
    ///
    /// If the instance is a number, then the instance is valid only if
    /// it has a value strictly less than (not equal to)
    /// `"exclusiveMaximum"`.
    ///
    /// See [JSON Schema Validation §6.2.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.3).
    #[serde(rename = "exclusiveMaximum", skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<Number>,

    /// The `multipleOf` keyword — divisibility constraint.
    ///
    /// The value of `"multipleOf"` MUST be a number, strictly greater
    /// than 0.
    ///
    /// A numeric instance is valid only if division by this keyword's
    /// value results in an integer.
    ///
    /// See [JSON Schema Validation §6.2.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.1).
    #[serde(rename = "multipleOf", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub multiple_of: Option<Number>,

    // -- String (§6.3) --
    /// The `minLength` keyword — minimum string length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// A string instance is valid against this keyword if its length is
    /// greater than, or equal to, the value of this keyword.
    ///
    /// The length of a string instance is defined as the number of its
    /// characters as defined by RFC 8259.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.3.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.2).
    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,

    /// The `maxLength` keyword — maximum string length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// A string instance is valid against this keyword if its length is
    /// less than, or equal to, the value of this keyword.
    ///
    /// The length of a string instance is defined as the number of its
    /// characters as defined by RFC 8259.
    ///
    /// See [JSON Schema Validation §6.3.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.1).
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,

    /// The `pattern` keyword — regex constraint.
    ///
    /// The value of this keyword MUST be a string. This string SHOULD
    /// be a valid regular expression, according to the ECMA-262
    /// regular expression dialect.
    ///
    /// A string instance is considered valid if the regular expression
    /// matches the instance successfully. Recall: regular expressions
    /// are not implicitly anchored.
    ///
    /// See [JSON Schema Validation §6.3.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "regex"))]
    pub pattern: Option<String>,

    // -- Array (§6.4) --
    /// The `minItems` keyword — minimum array length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An array instance is valid against `"minItems"` if its size is
    /// greater than, or equal to, the value of this keyword.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.4.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.2).
    #[serde(rename = "minItems", skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,

    /// The `maxItems` keyword — maximum array length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An array instance is valid against `"maxItems"` if its size is
    /// less than, or equal to, the value of this keyword.
    ///
    /// See [JSON Schema Validation §6.4.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.1).
    #[serde(rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,

    /// The `uniqueItems` keyword — array element uniqueness.
    ///
    /// The value of this keyword MUST be a boolean.
    ///
    /// If this keyword has boolean value false, the instance validates
    /// successfully. If it has boolean value true, the instance
    /// validates successfully if all of its elements are unique.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §6.4.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.3).
    #[serde(
        default,
        rename = "uniqueItems",
        skip_serializing_if = "crate::schema::is_false"
    )]
    #[schemars(extend("default" = false))]
    pub unique_items: bool,

    /// The `minContains` keyword — minimum `contains` matches.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// If `"contains"` is not present within the same schema object,
    /// then this keyword has no effect.
    ///
    /// An instance array is valid against `"minContains"` in two ways,
    /// depending on the form of the annotation result of an adjacent
    /// `"contains"` keyword. The first way is if the annotation result
    /// is an array and the length of that array is greater than or
    /// equal to the `"minContains"` value. The second way is if the
    /// annotation result is a boolean `true` and the instance array
    /// length is greater than or equal to the `"minContains"` value.
    ///
    /// A value of 0 is allowed, but is only useful for setting a range
    /// of occurrences from 0 to the value of `"maxContains"`. A value
    /// of 0 causes `"minContains"` and `"contains"` to always pass
    /// validation (but validation can still fail against a
    /// `"maxContains"` keyword).
    ///
    /// Omitting this keyword has the same behavior as a value of 1.
    ///
    /// See [JSON Schema Validation §6.4.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.5).
    #[serde(rename = "minContains", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = 1))]
    pub min_contains: Option<u64>,

    /// The `maxContains` keyword — maximum `contains` matches.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// If `"contains"` is not present within the same schema object,
    /// then this keyword has no effect.
    ///
    /// An instance array is valid against `"maxContains"` in two ways,
    /// depending on the form of the annotation result of an adjacent
    /// `"contains"` keyword. The first way is if the annotation result
    /// is an array and the length of that array is less than or equal
    /// to the `"maxContains"` value. The second way is if the
    /// annotation result is a boolean `true` and the instance array
    /// length is less than or equal to the `"maxContains"` value.
    ///
    /// See [JSON Schema Validation §6.4.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.4).
    #[serde(rename = "maxContains", skip_serializing_if = "Option::is_none")]
    pub max_contains: Option<u64>,

    // -- Object (§6.5) --
    /// The `required` keyword — required property names.
    ///
    /// The value of this keyword MUST be an array. Elements of this
    /// array, if any, MUST be strings, and MUST be unique.
    ///
    /// An object instance is valid against this keyword if every item
    /// in the array is the name of a property in the instance.
    ///
    /// Omitting this keyword has the same behavior as an empty array.
    ///
    /// See [JSON Schema Validation §6.5.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("uniqueItems" = true))]
    pub required: Option<Vec<String>>,

    /// The `minProperties` keyword — minimum property count.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An object instance is valid against `"minProperties"` if its
    /// number of properties is greater than, or equal to, the value of
    /// this keyword.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.5.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.2).
    #[serde(rename = "minProperties", skip_serializing_if = "Option::is_none")]
    pub min_properties: Option<u64>,

    /// The `maxProperties` keyword — maximum property count.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An object instance is valid against `"maxProperties"` if its
    /// number of properties is less than, or equal to, the value of
    /// this keyword.
    ///
    /// See [JSON Schema Validation §6.5.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.1).
    #[serde(rename = "maxProperties", skip_serializing_if = "Option::is_none")]
    pub max_properties: Option<u64>,

    /// The `dependentRequired` keyword — conditional required
    /// properties.
    ///
    /// The value of this keyword MUST be an object. Properties in this
    /// object, if any, MUST be arrays. Elements in each array, if any,
    /// MUST be strings, and MUST be unique.
    ///
    /// This keyword specifies properties that are required if a
    /// specific other property is present. Their requirement is
    /// dependent on the presence of the other property.
    ///
    /// Validation succeeds if, for each name that appears in both the
    /// instance and as a name within this keyword's value, every item
    /// in the corresponding array is also the name of a property in
    /// the instance.
    ///
    /// Omitting this keyword has the same behavior as an empty object.
    ///
    /// See [JSON Schema Validation §6.5.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.4).
    #[serde(rename = "dependentRequired", skip_serializing_if = "Option::is_none")]
    pub dependent_required: Option<IndexMap<String, Vec<String>>>,
}
