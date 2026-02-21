#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

extern crate alloc;

pub mod catalog;
pub use lintel_config as config;
pub use lintel_validation_cache as validation_cache;
pub mod diagnostics;
pub mod discover;
pub mod parsers;
pub mod registry;
pub mod retriever;
pub mod validate;
