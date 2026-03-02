#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod extensions;
pub mod flatten;
mod schema;

pub use extensions::{
    EnumValueMeta, ExtDocs, ExtLinks, IntellijSchemaExt, LintelSchemaExt, TaploInfoSchemaExt,
    TaploSchemaExt, TombiSchemaExt,
};
pub use schema::{Schema, SchemaValue, TypeValue, navigate_pointer, ref_name, resolve_ref};
