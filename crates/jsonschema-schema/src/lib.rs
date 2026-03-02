mod ext_lintel;
mod ext_taplo;
mod ext_tombi;
mod schema;

pub use ext_lintel::LintelExt;
pub use ext_taplo::{ExtDocs, ExtLinks, TaploSchemaExt};
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
