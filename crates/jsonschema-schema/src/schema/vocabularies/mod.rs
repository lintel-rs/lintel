//! JSON Schema 2020-12 vocabulary structs.
//!
//! Each vocabulary is defined in its own file and derives
//! [`combine_structs::Fields`], which caches its field definitions for
//! merging into the flat [`Schema`](crate::Schema) struct via
//! [`combine_fields`](combine_structs::combine_fields).

mod applicator;
mod content;
mod core;
mod format_annotation;
mod meta_data;
mod unevaluated;
mod validation;

pub use self::applicator::ApplicatorVocabulary;
pub use self::content::ContentVocabulary;
pub use self::core::CoreVocabulary;
pub use self::format_annotation::FormatAnnotationVocabulary;
pub use self::meta_data::MetaDataVocabulary;
pub use self::unevaluated::UnevaluatedVocabulary;
pub use self::validation::ValidationVocabulary;
