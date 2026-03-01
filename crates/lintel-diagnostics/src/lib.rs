#![doc = include_str!("../README.md")]
#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

pub mod diagnostics;
pub mod reporter;

pub use diagnostics::{
    DEFAULT_LABEL, LintelDiagnostic, find_instance_path_span, format_label, offset_to_line_col,
};
pub use reporter::{CheckResult, CheckedFile, Reporter};
