//! JSON Schema 2020-12 vocabulary structs.
//!
//! Each vocabulary is defined in its own file and derives
//! [`combine_structs::Fields`], which generates a companion callback macro.
//! These macros are used by [`merge_vocabularies!`](super) to merge all
//! vocabulary fields into the flat [`Schema`](crate::Schema) struct.

#[macro_use]
mod core;
#[macro_use]
mod applicator;
#[macro_use]
mod unevaluated;
#[macro_use]
mod validation;
#[macro_use]
mod meta_data;
#[macro_use]
mod format_annotation;
#[macro_use]
mod content;

pub use self::applicator::ApplicatorVocabulary;
pub use self::content::ContentVocabulary;
pub use self::core::CoreVocabulary;
pub use self::format_annotation::FormatAnnotationVocabulary;
pub use self::meta_data::MetaDataVocabulary;
pub use self::unevaluated::UnevaluatedVocabulary;
pub use self::validation::ValidationVocabulary;
