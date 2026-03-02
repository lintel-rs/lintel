use core::fmt::Write;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single validation error with its location, typed kind, and pre-computed span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON Pointer to the failing instance (e.g. `/jobs/build`).
    pub instance_path: String,
    /// JSON Schema path that triggered the error (e.g. `/properties/jobs/oneOf`).
    pub schema_path: String,
    /// The typed error kind with structured fields.
    pub kind: ValidationErrorKind,
    /// Byte offset and length in the source file for the error span.
    pub span: (usize, usize),
}

/// Typed validation error kinds mirroring `jsonschema::ValidationErrorKind`.
///
/// Non-serializable nested errors (e.g. `AnyOf`, `OneOf*` context) drop their
/// sub-error context. Non-serializable error types store a `message: String`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::AsRefStr)]
#[serde(tag = "type")]
#[strum(serialize_all = "snake_case")]
pub enum ValidationErrorKind {
    AdditionalItems {
        limit: usize,
    },
    /// A single unexpected property — split from `AdditionalProperties` (plural).
    AdditionalProperty {
        property: String,
    },
    AnyOf,
    BacktrackLimitExceeded {
        message: String,
    },
    Constant {
        expected_value: Value,
    },
    Contains,
    ContentEncoding {
        content_encoding: String,
    },
    ContentMediaType {
        content_media_type: String,
    },
    Custom {
        keyword: String,
        message: String,
    },
    Enum {
        options: Value,
    },
    ExclusiveMaximum {
        limit: Value,
    },
    ExclusiveMinimum {
        limit: Value,
    },
    FalseSchema,
    Format {
        format: String,
    },
    FromUtf8 {
        message: String,
    },
    MaxItems {
        limit: u64,
    },
    Maximum {
        limit: Value,
    },
    MaxLength {
        limit: u64,
    },
    MaxProperties {
        limit: u64,
    },
    MinItems {
        limit: u64,
    },
    Minimum {
        limit: Value,
    },
    MinLength {
        limit: u64,
    },
    MinProperties {
        limit: u64,
    },
    MultipleOf {
        multiple_of: f64,
    },
    Not,
    OneOfMultipleValid,
    OneOfNotValid,
    Pattern {
        pattern: String,
    },
    PropertyNames {
        message: String,
    },
    Required {
        property: String,
    },
    Type {
        expected: String,
    },
    UnevaluatedItems {
        unexpected: Vec<String>,
    },
    UnevaluatedProperties {
        unexpected: Vec<String>,
    },
    UniqueItems,
    Referencing {
        message: String,
    },
}

impl ValidationErrorKind {
    /// Produce a human-readable error message from the structured fields.
    #[allow(clippy::match_same_arms)]
    pub fn message(&self) -> String {
        match self {
            Self::AdditionalItems { limit } => {
                format!("Additional items are not allowed (limit: {limit})")
            }
            Self::AdditionalProperty { property } => {
                format!("Additional properties are not allowed ('{property}' was unexpected)")
            }
            Self::AnyOf => {
                "not valid under any of the schemas listed in the 'anyOf' keyword".to_string()
            }
            Self::BacktrackLimitExceeded { message }
            | Self::Custom { message, .. }
            | Self::FromUtf8 { message }
            | Self::PropertyNames { message }
            | Self::Referencing { message } => message.clone(),
            Self::Constant { expected_value } => format!("{expected_value} was expected"),
            Self::Contains => "None of the items are valid under the given schema".to_string(),
            Self::ContentEncoding { content_encoding } => {
                format!(r#"not compliant with "{content_encoding}" content encoding"#)
            }
            Self::ContentMediaType { content_media_type } => {
                format!(r#"not compliant with "{content_media_type}" media type"#)
            }
            Self::Enum { options } => {
                let mut msg = String::new();
                if let Value::Array(arr) = options {
                    let _ = write!(msg, "value is not one of: ");
                    for (i, opt) in arr.iter().enumerate() {
                        if i > 0 {
                            let _ = write!(msg, ", ");
                        }
                        let _ = write!(msg, "{opt}");
                    }
                } else {
                    let _ = write!(msg, "{options} was expected");
                }
                msg
            }
            Self::ExclusiveMaximum { limit } => {
                format!("value is greater than or equal to the maximum of {limit}")
            }
            Self::ExclusiveMinimum { limit } => {
                format!("value is less than or equal to the minimum of {limit}")
            }
            Self::FalseSchema => "False schema does not allow any value".to_string(),
            Self::Format { format } => format!(r#"value is not a "{format}""#),
            Self::MaxItems { limit } => {
                let s = if *limit == 1 { "" } else { "s" };
                format!("array has more than {limit} item{s}")
            }
            Self::Maximum { limit } => format!("value is greater than the maximum of {limit}"),
            Self::MaxLength { limit } => {
                let s = if *limit == 1 { "" } else { "s" };
                format!("string is longer than {limit} character{s}")
            }
            Self::MaxProperties { limit } => {
                let s = if *limit == 1 { "y" } else { "ies" };
                format!("object has more than {limit} propert{s}")
            }
            Self::MinItems { limit } => {
                let s = if *limit == 1 { "" } else { "s" };
                format!("array has less than {limit} item{s}")
            }
            Self::Minimum { limit } => format!("value is less than the minimum of {limit}"),
            Self::MinLength { limit } => {
                let s = if *limit == 1 { "" } else { "s" };
                format!("string is shorter than {limit} character{s}")
            }
            Self::MinProperties { limit } => {
                let s = if *limit == 1 { "y" } else { "ies" };
                format!("object has less than {limit} propert{s}")
            }
            Self::MultipleOf { multiple_of } => {
                format!("value is not a multiple of {multiple_of}")
            }
            Self::Not => "value should not be valid under the given schema".to_string(),
            Self::OneOfMultipleValid => {
                "valid under more than one of the schemas listed in the 'oneOf' keyword".to_string()
            }
            Self::OneOfNotValid => {
                "not valid under any of the schemas listed in the 'oneOf' keyword".to_string()
            }
            Self::Pattern { pattern } => format!(r#"value does not match "{pattern}""#),
            Self::Required { property } => format!("{property} is a required property"),
            Self::Type { expected } => format!(r#"value is not of type "{expected}""#),
            Self::UnevaluatedItems { unexpected } => {
                let mut msg = "Unevaluated items are not allowed (".to_string();
                write_quoted_list(&mut msg, unexpected);
                write_unexpected_suffix(&mut msg, unexpected.len());
                msg
            }
            Self::UnevaluatedProperties { unexpected } => {
                let mut msg = "Unevaluated properties are not allowed (".to_string();
                write_quoted_list(&mut msg, unexpected);
                write_unexpected_suffix(&mut msg, unexpected.len());
                msg
            }
            Self::UniqueItems => "array has non-unique elements".to_string(),
        }
    }
}

fn write_quoted_list(buf: &mut String, items: &[String]) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            let _ = write!(buf, ", ");
        }
        let _ = write!(buf, "'{item}'");
    }
}

fn write_unexpected_suffix(buf: &mut String, count: usize) {
    if count == 1 {
        let _ = write!(buf, " was unexpected)");
    } else {
        let _ = write!(buf, " were unexpected)");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn additional_property_message() {
        let kind = ValidationErrorKind::AdditionalProperty {
            property: "foo".to_string(),
        };
        assert_eq!(
            kind.message(),
            "Additional properties are not allowed ('foo' was unexpected)"
        );
    }

    #[test]
    fn required_message() {
        let kind = ValidationErrorKind::Required {
            property: "\"name\"".to_string(),
        };
        assert_eq!(kind.message(), "\"name\" is a required property");
    }

    #[test]
    fn type_message() {
        let kind = ValidationErrorKind::Type {
            expected: "string".to_string(),
        };
        assert_eq!(kind.message(), r#"value is not of type "string""#);
    }

    #[test]
    fn enum_message() {
        let kind = ValidationErrorKind::Enum {
            options: json!(["a", "b", "c"]),
        };
        assert_eq!(kind.message(), r#"value is not one of: "a", "b", "c""#);
    }

    #[test]
    fn serialization_roundtrip() {
        let error = ValidationError {
            instance_path: "/name".to_string(),
            schema_path: "/properties/name/type".to_string(),
            kind: ValidationErrorKind::Type {
                expected: "string".to_string(),
            },
            span: (10, 5),
        };
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ValidationError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, deserialized);
    }

    #[test]
    fn additional_property_serialization() {
        let error = ValidationError {
            instance_path: "/foo".to_string(),
            schema_path: "/additionalProperties".to_string(),
            kind: ValidationErrorKind::AdditionalProperty {
                property: "foo".to_string(),
            },
            span: (5, 3),
        };
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ValidationError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, deserialized);
    }
}
