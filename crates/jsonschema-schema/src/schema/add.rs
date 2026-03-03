use alloc::collections::BTreeMap;
use core::ops::Add;

use indexmap::IndexMap;

use crate::extensions::IntellijSchemaExt;
use crate::extensions::TombiSchemaExt;

use super::Schema;

/// Merge two `IndexMap` values with left-bias: entries from `source`
/// are added only if the key does not already exist in `target`.
fn merge_index_map<V>(
    mut target: IndexMap<String, V>,
    source: IndexMap<String, V>,
) -> IndexMap<String, V> {
    for (k, v) in source {
        target.entry(k).or_insert(v);
    }
    target
}

/// Merge two `Option<BTreeMap>` values with left-bias: entries from `source`
/// are added only if the key does not already exist in `target`.
fn merge_option_btree_map<V>(
    target: Option<BTreeMap<String, V>>,
    source: Option<BTreeMap<String, V>>,
) -> Option<BTreeMap<String, V>> {
    match (target, source) {
        (Some(mut t), Some(s)) => {
            for (k, v) in s {
                t.entry(k).or_insert(v);
            }
            Some(t)
        }
        (t, s) => t.or(s),
    }
}

/// Merge two `Option<Vec<String>>` values by taking the union (deduplicated).
fn union_option_vec(
    target: Option<Vec<String>>,
    source: Option<Vec<String>>,
) -> Option<Vec<String>> {
    match (target, source) {
        (Some(mut t), Some(s)) => {
            for item in s {
                if !t.contains(&item) {
                    t.push(item);
                }
            }
            Some(t)
        }
        (t, s) => t.or(s),
    }
}

impl Add for Schema {
    type Output = Self;

    /// Merge two schemas with left-bias.
    ///
    /// - **Map fields** (`properties`, `pattern_properties`, `defs`, `dependent_schemas`):
    ///   merge — rhs entries added only if key doesn't exist in self.
    /// - **`required`**: union (deduplicate).
    /// - **`extra`** (`BTreeMap` catch-all): merge — rhs entries added only if key doesn't exist.
    /// - **All other `Option<T>` fields**: `self.field.or(rhs.field)` — left wins.
    /// - **`bool` fields**: `self.field || rhs.field` — true wins.
    #[allow(clippy::too_many_lines)]
    fn add(self, rhs: Self) -> Self {
        let extra = {
            let mut merged = self.extra;
            for (k, v) in rhs.extra {
                merged.entry(k).or_insert(v);
            }
            merged
        };

        let x_tombi = TombiSchemaExt {
            toml_version: self.x_tombi.toml_version.or(rhs.x_tombi.toml_version),
            table_keys_order: self
                .x_tombi
                .table_keys_order
                .or(rhs.x_tombi.table_keys_order),
            additional_key_label: self
                .x_tombi
                .additional_key_label
                .or(rhs.x_tombi.additional_key_label),
            array_values_order: self
                .x_tombi
                .array_values_order
                .or(rhs.x_tombi.array_values_order),
        };

        Self {
            // Core
            schema: self.schema.or(rhs.schema),
            id: self.id.or(rhs.id),
            ref_: self.ref_.or(rhs.ref_),
            anchor: self.anchor.or(rhs.anchor),
            dynamic_ref: self.dynamic_ref.or(rhs.dynamic_ref),
            dynamic_anchor: self.dynamic_anchor.or(rhs.dynamic_anchor),
            comment: self.comment.or(rhs.comment),
            defs: merge_option_btree_map(self.defs, rhs.defs),
            vocabulary: self.vocabulary.or(rhs.vocabulary),

            // Metadata
            title: self.title.or(rhs.title),
            description: self.description.or(rhs.description),
            default: self.default.or(rhs.default),
            deprecated: self.deprecated || rhs.deprecated,
            read_only: self.read_only || rhs.read_only,
            write_only: self.write_only || rhs.write_only,
            examples: self.examples.or(rhs.examples),

            // Type
            type_: self.type_.or(rhs.type_),
            enum_: self.enum_.or(rhs.enum_),
            markdown_enum_descriptions: self
                .markdown_enum_descriptions
                .or(rhs.markdown_enum_descriptions),
            const_: self.const_.or(rhs.const_),

            // Object — map fields are merged
            properties: merge_index_map(self.properties, rhs.properties),
            pattern_properties: merge_index_map(self.pattern_properties, rhs.pattern_properties),
            additional_properties: self.additional_properties.or(rhs.additional_properties),
            required: union_option_vec(self.required, rhs.required),
            property_names: self.property_names.or(rhs.property_names),
            min_properties: self.min_properties.or(rhs.min_properties),
            max_properties: self.max_properties.or(rhs.max_properties),
            unevaluated_properties: self.unevaluated_properties.or(rhs.unevaluated_properties),

            // Array
            items: self.items.or(rhs.items),
            prefix_items: self.prefix_items.or(rhs.prefix_items),
            contains: self.contains.or(rhs.contains),
            min_contains: self.min_contains.or(rhs.min_contains),
            max_contains: self.max_contains.or(rhs.max_contains),
            min_items: self.min_items.or(rhs.min_items),
            max_items: self.max_items.or(rhs.max_items),
            unique_items: self.unique_items || rhs.unique_items,
            unevaluated_items: self.unevaluated_items.or(rhs.unevaluated_items),

            // Number
            minimum: self.minimum.or(rhs.minimum),
            maximum: self.maximum.or(rhs.maximum),
            exclusive_minimum: self.exclusive_minimum.or(rhs.exclusive_minimum),
            exclusive_maximum: self.exclusive_maximum.or(rhs.exclusive_maximum),
            multiple_of: self.multiple_of.or(rhs.multiple_of),

            // String
            min_length: self.min_length.or(rhs.min_length),
            max_length: self.max_length.or(rhs.max_length),
            pattern: self.pattern.or(rhs.pattern),
            format: self.format.or(rhs.format),

            // Composition — NOT merged
            all_of: self.all_of.or(rhs.all_of),
            any_of: self.any_of.or(rhs.any_of),
            one_of: self.one_of.or(rhs.one_of),
            not: self.not.or(rhs.not),

            // Conditional
            if_: self.if_.or(rhs.if_),
            then_: self.then_.or(rhs.then_),
            else_: self.else_.or(rhs.else_),

            // Dependencies
            dependent_required: self.dependent_required.or(rhs.dependent_required),
            dependent_schemas: merge_index_map(self.dependent_schemas, rhs.dependent_schemas),

            // Content
            content_media_type: self.content_media_type.or(rhs.content_media_type),
            content_encoding: self.content_encoding.or(rhs.content_encoding),
            content_schema: self.content_schema.or(rhs.content_schema),

            // Extensions
            markdown_description: self.markdown_description.or(rhs.markdown_description),
            x_lintel: self.x_lintel.or(rhs.x_lintel),
            x_taplo: self.x_taplo.or(rhs.x_taplo),
            x_taplo_info: self.x_taplo_info.or(rhs.x_taplo_info),
            x_tombi,
            x_intellij: IntellijSchemaExt {
                html_description: self
                    .x_intellij
                    .html_description
                    .or(rhs.x_intellij.html_description),
                language_injection: self
                    .x_intellij
                    .language_injection
                    .or(rhs.x_intellij.language_injection),
                enum_metadata: self
                    .x_intellij
                    .enum_metadata
                    .or(rhs.x_intellij.enum_metadata),
            },

            extra,
        }
    }
}
