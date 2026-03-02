#![doc = include_str!("../README.md")]

extern crate alloc;

pub(crate) mod absolute;
pub mod extensions;
pub(crate) mod flatten;
mod schema;
pub(crate) mod validate;

pub use extensions::{
    EnumValueMeta, ExtDocs, ExtLinks, IntellijSchemaExt, LintelSchemaExt, TaploInfoSchemaExt,
    TaploSchemaExt, TombiSchemaExt,
};
pub use schema::{
    Schema, SchemaValue, SimpleType, TypeValue, navigate_pointer, ref_name, resolve_ref,
    vocabularies::{
        ApplicatorVocabulary, ContentVocabulary, CoreVocabulary, FormatAnnotationVocabulary,
        MetaDataVocabulary, UnevaluatedVocabulary, ValidationVocabulary,
    },
};
pub use validate::SchemaError;

/// Generate the JSON Schema for [`SchemaValue`] (a JSON Schema 2020-12
/// document).
///
/// # Panics
///
/// Panics if the generated schema cannot be serialized to
/// `serde_json::Value` (should never happen in practice).
pub fn schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(SchemaValue))
        .expect("schema serialization cannot fail")
}
