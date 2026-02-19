#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

pub mod catalog;
pub use lintel_config as config;
pub mod diagnostics;
pub mod discover;
pub mod parsers;
pub mod registry;
pub mod retriever;
pub mod validate;
