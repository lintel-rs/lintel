#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod extensions;
mod schema;

pub use extensions::{ExtDocs, ExtLinks, LintelExt, TaploInfo, TaploSchemaExt, TombiExt};
pub use schema::{Schema, SchemaValue, TypeValue, navigate_pointer, ref_name, resolve_ref};
