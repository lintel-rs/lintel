#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod extensions;
mod schema;

pub use extensions::{
    EnumValueMeta, ExtDocs, ExtLinks, IntellijSchemaExt, LintelSchemaExt, TaploInfoSchemaExt,
    TaploSchemaExt, TombiSchemaExt,
};
pub use schema::{
    Schema, SchemaValue, SimpleType, TypeValue, navigate_pointer, ref_name, resolve_ref,
};

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
