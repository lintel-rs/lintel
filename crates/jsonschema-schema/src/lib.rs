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
pub use schema::{Schema, SchemaValue, TypeValue, navigate_pointer, ref_name, resolve_ref};
pub use validate::SchemaError;
