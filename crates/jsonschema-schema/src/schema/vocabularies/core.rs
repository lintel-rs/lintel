use alloc::collections::BTreeMap;

use indexmap::IndexMap;
use url::Url;

use crate::SchemaValue;

/// Core vocabulary — identifiers, references, and definitions.
///
/// See [JSON Schema Core §8](https://json-schema.org/draft/2020-12/json-schema-core#section-8).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    combine_structs::Fields,
)]
pub struct CoreVocabulary {
    /// The `$schema` keyword — JSON Schema dialect identifier.
    ///
    /// The `"$schema"` keyword is both used as a JSON Schema dialect
    /// identifier and as the identifier of a resource which is itself a
    /// JSON Schema, which describes the set of valid schemas written
    /// for this particular dialect.
    ///
    /// The value of this keyword MUST be a URI (containing a scheme)
    /// and this URI MUST be normalized. The current schema MUST be
    /// valid against the meta-schema identified by this URI.
    ///
    /// If this URI identifies a retrievable resource, that resource
    /// SHOULD be of media type `"application/schema+json"`.
    ///
    /// The `"$schema"` keyword SHOULD be used in the document root
    /// schema object, and MAY be used in the root schema objects of
    /// embedded schema resources. It MUST NOT appear in non-resource
    /// root schema objects. If absent from the document root schema,
    /// the resulting behavior is implementation-defined.
    ///
    /// Values for this property are defined elsewhere in this and
    /// other documents, and by other parties.
    ///
    /// See [JSON Schema Core §8.1.1](https://json-schema.org/draft/2020-12/json-schema-core#section-8.1.1).
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<Url>,

    /// The `$id` keyword — schema resource identifier.
    ///
    /// The `"$id"` keyword identifies a schema resource with its
    /// canonical URI.
    ///
    /// Note that this URI is an identifier and not necessarily a
    /// network locator. In the case of a network-addressable URL, a
    /// schema need not be downloadable from its canonical URI.
    ///
    /// If present, the value for this keyword MUST be a string, and
    /// MUST represent a valid URI-reference. This URI-reference SHOULD
    /// be normalized, and MUST resolve to an absolute-URI (without a
    /// fragment), or to a URI with an empty fragment.
    ///
    /// The absolute-URI also serves as the base URI for relative
    /// URI-references in keywords within the schema resource, in
    /// accordance with RFC 3986 section 5.1.1 regarding base URIs
    /// embedded in content.
    ///
    /// The presence of `"$id"` in a subschema indicates that the
    /// subschema constitutes a distinct schema resource within a
    /// single schema document.
    ///
    /// See [JSON Schema Core §8.2.1](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.1).
    #[serde(rename = "$id", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "uri-reference", "pattern" = "^[^#]*#?$"))]
    pub id: Option<String>,

    /// The `$ref` keyword — static schema reference.
    ///
    /// The `"$ref"` keyword is an applicator that is used to reference
    /// a statically identified schema. Its results are the results of
    /// the referenced schema. Note that this definition of how the
    /// results are determined means that other keywords can appear
    /// alongside of `"$ref"` in the same schema object.
    ///
    /// The value of the `"$ref"` keyword MUST be a string which is a
    /// URI-Reference. Resolved against the current URI base, it
    /// produces the URI of the schema to apply. This resolution is
    /// safe to perform on schema load, as the process of evaluating an
    /// instance cannot change how the reference resolves.
    ///
    /// See [JSON Schema Core §8.2.3.1](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.3.1).
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "uri-reference"))]
    pub ref_: Option<String>,

    /// The `$anchor` keyword — plain-name fragment identifier.
    ///
    /// The `"$anchor"` and `"$dynamicAnchor"` keywords are used to
    /// specify plain name fragments. They are identifier keywords that
    /// can only be used to create plain name fragments, rather than
    /// absolute URIs as seen with `"$id"`.
    ///
    /// If present, the value of this keyword MUST be a string and MUST
    /// start with a letter (`[A-Za-z]`) or underscore (`"_"`),
    /// followed by any number of letters, digits (`[0-9]`), hyphens
    /// (`"-"`), underscores (`"_"`), and periods (`"."`).
    ///
    /// See [JSON Schema Core §8.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.2).
    #[serde(rename = "$anchor", skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[A-Za-z_][-A-Za-z0-9._]*$"))]
    pub anchor: Option<String>,

    /// The `$dynamicRef` keyword — dynamic schema reference.
    ///
    /// The `"$dynamicRef"` keyword is an applicator that allows for
    /// deferring the full resolution until runtime, at which point it
    /// is resolved each time it is encountered while evaluating an
    /// instance.
    ///
    /// Together with `"$dynamicAnchor"`, `"$dynamicRef"` implements a
    /// cooperative extension mechanism that is primarily useful with
    /// recursive schemas (schemas that reference themselves). Both the
    /// extension point and the runtime-determined extension target are
    /// defined with `"$dynamicAnchor"`, and only exhibit runtime
    /// dynamic behavior when referenced with `"$dynamicRef"`.
    ///
    /// The value of the `"$dynamicRef"` property MUST be a string
    /// which is a URI-Reference. Resolved against the current URI
    /// base, it produces the URI used as the starting point for
    /// runtime resolution.
    ///
    /// See [JSON Schema Core §8.2.3.2](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.3.2).
    #[serde(rename = "$dynamicRef", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "uri-reference"))]
    pub dynamic_ref: Option<String>,

    /// The `$dynamicAnchor` keyword — dynamic extension point.
    ///
    /// Separately from the usual usage of URIs, `"$dynamicAnchor"`
    /// indicates that the fragment is an extension point when used
    /// with the `"$dynamicRef"` keyword. This low-level, advanced
    /// feature makes it easier to extend recursive schemas such as the
    /// meta-schemas, without imposing any particular semantics on that
    /// extension.
    ///
    /// If present, the value of this keyword MUST be a string and MUST
    /// conform to the same rules as `"$anchor"`.
    ///
    /// See [JSON Schema Core §8.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.2).
    #[serde(rename = "$dynamicAnchor", skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^[A-Za-z_][-A-Za-z0-9._]*$"))]
    pub dynamic_anchor: Option<String>,

    /// The `$comment` keyword — schema author comments.
    ///
    /// This keyword reserves a location for comments from schema
    /// authors to readers or maintainers of the schema.
    ///
    /// The value of this keyword MUST be a string. Implementations
    /// MUST NOT present this string to end users. Tools for editing
    /// schemas SHOULD support displaying and editing this keyword. The
    /// value of this keyword MAY be used in debug or error output
    /// which is intended for developers making use of schemas.
    ///
    /// Implementations MAY strip `"$comment"` values at any point
    /// during processing. In particular, this allows for shortening
    /// schemas when the size of deployed schemas is a concern.
    ///
    /// Implementations MUST NOT take any other action based on the
    /// presence, absence, or contents of `"$comment"` properties. In
    /// particular, the value of `"$comment"` MUST NOT be collected as
    /// an annotation result.
    ///
    /// See [JSON Schema Core §8.3](https://json-schema.org/draft/2020-12/json-schema-core#section-8.3).
    #[serde(rename = "$comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// The `$defs` keyword — inline re-usable schema definitions.
    ///
    /// The `$defs` keyword reserves a location for schema authors to
    /// inline re-usable JSON Schemas into a more general schema. The
    /// keyword does not directly affect the validation result.
    ///
    /// This keyword's value MUST be an object. Each member value of
    /// this object MUST be a valid JSON Schema.
    ///
    /// As an example, here is a schema describing an array of positive
    /// integers, where the positive integer constraint is a subschema in
    /// `$defs`:
    ///
    /// ```json
    /// {
    ///     "type": "array",
    ///     "items": { "$ref": "#/$defs/positiveInteger" },
    ///     "$defs": {
    ///         "positiveInteger": {
    ///             "type": "integer",
    ///             "exclusiveMinimum": 0
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// See [JSON Schema Core §8.2.4](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.4).
    #[serde(rename = "$defs", skip_serializing_if = "Option::is_none")]
    pub defs: Option<BTreeMap<String, SchemaValue>>,

    /// The `$vocabulary` keyword — meta-schema vocabulary declaration.
    ///
    /// The `"$vocabulary"` keyword is used in meta-schemas to identify
    /// the vocabularies available for use in schemas described by that
    /// meta-schema. It is also used to indicate whether each vocabulary
    /// is required or optional, in the sense that an implementation
    /// MUST understand the required vocabularies in order to
    /// successfully process the schema. Together, this information
    /// forms a dialect. Any vocabulary that is understood by the
    /// implementation MUST be processed in a manner consistent with
    /// the semantic definitions contained within the vocabulary.
    ///
    /// The value of this keyword MUST be an object. The property names
    /// in the object MUST be URIs (containing a scheme) and this URI
    /// MUST be normalized. Each URI that appears as a property name
    /// identifies a specific set of keywords and their semantics.
    ///
    /// The values of the object properties MUST be booleans. If the
    /// value is true, then implementations that do not recognize the
    /// vocabulary MUST refuse to process any schemas that declare this
    /// meta-schema with `"$schema"`. If the value is false,
    /// implementations that do not recognize the vocabulary SHOULD
    /// proceed with processing such schemas. The value has no impact
    /// if the implementation understands the vocabulary.
    ///
    /// The `"$vocabulary"` keyword SHOULD be used in the root schema
    /// of any schema document intended for use as a meta-schema. It
    /// MUST NOT appear in subschemas.
    ///
    /// The `"$vocabulary"` keyword MUST be ignored in schema documents
    /// that are not being processed as a meta-schema.
    ///
    /// See [JSON Schema Core §8.1.2](https://json-schema.org/draft/2020-12/json-schema-core#section-8.1.2).
    #[serde(rename = "$vocabulary", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("propertyNames" = { "format": "uri" }))]
    pub vocabulary: Option<IndexMap<Url, bool>>,
}
