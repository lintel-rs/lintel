#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

pub mod catalog;
pub mod config;
pub mod diagnostics;
pub mod discover;
pub mod parsers;
pub mod retriever;
pub mod validate;
