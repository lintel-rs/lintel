use alloc::collections::BTreeMap;

use indexmap::IndexMap;

/// Helper for `#[serde(skip_serializing_if)]` on `bool` fields.
#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if requires &T
fn is_false(v: &bool) -> bool {
    !v
}
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use url::Url;

use crate::extensions::IntellijSchemaExt;
use crate::extensions::LintelSchemaExt;
use crate::extensions::TaploInfoSchemaExt;
use crate::extensions::TaploSchemaExt;
use crate::extensions::TombiSchemaExt;

/// A JSON Schema value — either a boolean schema or an object schema.
///
/// A schema can be a JSON object or a JSON boolean. Boolean schemas are
/// equivalent to certain object schemas:
///
/// - `true` — always validates successfully (equivalent to `{}`).
/// - `false` — never validates successfully (equivalent to `{"not": {}}`).
///
/// See [JSON Schema Core §4.3.2](https://json-schema.org/draft/2020-12/json-schema-core#section-4.3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SchemaValue {
    /// A boolean schema: `true` accepts everything, `false` rejects everything.
    Bool(bool),
    /// An object schema with keyword-based constraints.
    Schema(Box<Schema>),
}

/// Primitive type names defined by JSON Schema (`simpleTypes`).
///
/// String values MUST be one of the six primitive types (`"null"`,
/// `"boolean"`, `"object"`, `"array"`, `"number"`, or `"string"`), or
/// `"integer"` which matches any number with a zero fractional part.
///
/// See [JSON Schema Validation §6.1.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, strum::Display,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "lowercase")]
pub enum SimpleType {
    /// A JSON array (ordered sequence of values).
    Array,
    /// A JSON `true` or `false` value.
    Boolean,
    /// A JSON number with a zero fractional part (subset of `Number`).
    Integer,
    /// The JSON `null` value.
    Null,
    /// A JSON number (any numeric value, including integers).
    Number,
    /// A JSON object (unordered set of name/value pairs).
    Object,
    /// A JSON string.
    String,
}

/// The value of the JSON Schema `type` keyword.
///
/// The value of this keyword MUST be either a string or an array. If it is
/// an array, elements of the array MUST be strings and MUST be unique.
///
/// See [JSON Schema Validation §6.1.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TypeValue {
    /// A single type constraint, e.g. `"type": "string"`.
    Single(SimpleType),
    /// A union of types, e.g. `"type": ["string", "null"]`.
    /// The array SHOULD have at least one element, and elements MUST be unique.
    Union(Vec<SimpleType>),
}

/// A JSON Schema object (draft 2020-12).
///
/// Represents a single schema resource as defined by the
/// [JSON Schema Core](https://json-schema.org/draft/2020-12/json-schema-core) and
/// [JSON Schema Validation](https://json-schema.org/draft/2020-12/json-schema-validation)
/// specifications.
///
/// Fields are grouped by vocabulary:
///
/// - **Core** (`$schema`, `$id`, `$ref`, `$anchor`, `$dynamicRef`,
///   `$dynamicAnchor`, `$comment`, `$defs`, `$vocabulary`)
/// - **Metadata / Annotation** (`title`, `description`, `default`,
///   `deprecated`, `readOnly`, `writeOnly`, `examples`)
/// - **Validation — type** (`type`, `enum`, `const`)
/// - **Applicator — object** (`properties`, `patternProperties`,
///   `additionalProperties`, `propertyNames`, `unevaluatedProperties`)
/// - **Validation — object** (`required`, `minProperties`,
///   `maxProperties`, `dependentRequired`)
/// - **Applicator — array** (`items`, `prefixItems`, `contains`,
///   `unevaluatedItems`)
/// - **Validation — array** (`minItems`, `maxItems`, `uniqueItems`,
///   `minContains`, `maxContains`)
/// - **Validation — number** (`minimum`, `maximum`, `exclusiveMinimum`,
///   `exclusiveMaximum`, `multipleOf`)
/// - **Validation — string** (`minLength`, `maxLength`, `pattern`, `format`)
/// - **Applicator — composition** (`allOf`, `anyOf`, `oneOf`, `not`)
/// - **Applicator — conditional** (`if`, `then`, `else`,
///   `dependentSchemas`)
/// - **Content** (`contentMediaType`, `contentEncoding`, `contentSchema`)
#[allow(clippy::struct_excessive_bools)] // mirrors the JSON Schema spec
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Schema {
    // ---------------------------------------------------------------
    // Core vocabulary (JSON Schema Core §8)
    // ---------------------------------------------------------------
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

    /// The `title` keyword — short summary annotation.
    ///
    /// The value of this keyword MUST be a string.
    ///
    /// Both `"title"` and `"description"` can be used to decorate a
    /// user interface with information about the data produced by this
    /// user interface. A title will preferably be short, whereas a
    /// description will provide explanation about the purpose of the
    /// instance described by this schema.
    ///
    /// See [JSON Schema Validation §9.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// The `description` keyword — explanatory annotation.
    ///
    /// The value of this keyword MUST be a string.
    ///
    /// Both `"title"` and `"description"` can be used to decorate a
    /// user interface with information about the data produced by this
    /// user interface. A title will preferably be short, whereas a
    /// description will provide explanation about the purpose of the
    /// instance described by this schema.
    ///
    /// See [JSON Schema Validation §9.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The `markdownDescription` keyword — Markdown-formatted
    /// description (VS Code / non-standard extension).
    ///
    /// Not part of the JSON Schema specification. When present, it is
    /// preferred over [`description`](Self::description) by editors
    /// that support Markdown rendering.
    #[serde(
        rename = "markdownDescription",
        skip_serializing_if = "Option::is_none"
    )]
    pub markdown_description: Option<String>,

    /// Lintel provenance metadata (`x-lintel`).
    #[serde(rename = "x-lintel", skip_serializing_if = "Option::is_none")]
    pub x_lintel: Option<LintelSchemaExt>,

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

    // ---------------------------------------------------------------
    // Metadata / annotation vocabulary (JSON Schema Validation §9)
    // ---------------------------------------------------------------
    /// The `default` keyword — default value annotation.
    ///
    /// There are no restrictions placed on the value of this keyword.
    /// When multiple occurrences of this keyword are applicable to a
    /// single sub-instance, implementations SHOULD remove duplicates.
    ///
    /// This keyword can be used to supply a default JSON value
    /// associated with a particular schema. It is RECOMMENDED that a
    /// default value be valid against the associated schema.
    ///
    /// See [JSON Schema Validation §9.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// The `deprecated` keyword — deprecation annotation.
    ///
    /// The value of this keyword MUST be a boolean. When multiple
    /// occurrences of this keyword are applicable to a single
    /// sub-instance, applications SHOULD consider the instance location
    /// to be deprecated if any occurrence specifies a true value.
    ///
    /// If `"deprecated"` has a value of boolean true, it indicates that
    /// applications SHOULD refrain from usage of the declared property.
    /// It MAY mean the property is going to be removed in the future.
    ///
    /// A root schema containing `"deprecated"` with a value of true
    /// indicates that the entire resource being described MAY be removed
    /// in the future.
    ///
    /// The `"deprecated"` keyword applies to each instance location to
    /// which the schema object containing the keyword successfully
    /// applies. This can result in scenarios where every array item or
    /// object property is deprecated even though the containing array
    /// or object is not.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.3).
    #[serde(default, skip_serializing_if = "is_false")]
    #[schemars(extend("default" = false))]
    pub deprecated: bool,

    /// The `readOnly` keyword — read-only annotation.
    ///
    /// The value of this keyword MUST be a boolean. When multiple
    /// occurrences of this keyword are applicable to a single
    /// sub-instance, the resulting behavior SHOULD be as for a true
    /// value if any occurrence specifies a true value, and SHOULD be as
    /// for a false value otherwise.
    ///
    /// If `"readOnly"` has a value of boolean true, it indicates that
    /// the value of the instance is managed exclusively by the owning
    /// authority, and attempts by an application to modify the value of
    /// this property are expected to be ignored or rejected by that
    /// owning authority.
    ///
    /// An instance document that is marked as `"readOnly"` for the
    /// entire document MAY be ignored if sent to the owning authority,
    /// or MAY result in an error, at the authority's discretion.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.4).
    #[serde(default, rename = "readOnly", skip_serializing_if = "is_false")]
    #[schemars(extend("default" = false))]
    pub read_only: bool,

    /// The `writeOnly` keyword — write-only annotation.
    ///
    /// The value of this keyword MUST be a boolean.
    ///
    /// If `"writeOnly"` has a value of boolean true, it indicates that
    /// the value is never present when the instance is retrieved from
    /// the owning authority. It can be present when sent to the owning
    /// authority to update or create the document (or the resource it
    /// represents), but it will not be included in any updated or newly
    /// created version of the instance.
    ///
    /// An instance document that is marked as `"writeOnly"` for the
    /// entire document MAY be returned as a blank document of some
    /// sort, or MAY produce an error upon retrieval, or have the
    /// retrieval request ignored, at the authority's discretion.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §9.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.4).
    #[serde(default, rename = "writeOnly", skip_serializing_if = "is_false")]
    #[schemars(extend("default" = false))]
    pub write_only: bool,

    /// The `examples` keyword — example values annotation.
    ///
    /// The value of this keyword MUST be an array. There are no
    /// restrictions placed on the values within the array. When
    /// multiple occurrences of this keyword are applicable to a single
    /// sub-instance, implementations MUST provide a flat array of all
    /// values rather than an array of arrays.
    ///
    /// This keyword can be used to provide sample JSON values
    /// associated with a particular schema, for the purpose of
    /// illustrating usage. It is RECOMMENDED that these values be valid
    /// against the associated schema.
    ///
    /// Implementations MAY use the value(s) of `"default"`, if present,
    /// as an additional example. If `"examples"` is absent, `"default"`
    /// MAY still be used in this manner.
    ///
    /// See [JSON Schema Validation §9.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<Value>>,

    // ---------------------------------------------------------------
    // Type validation (JSON Schema Validation §6.1)
    // ---------------------------------------------------------------
    /// The `type` keyword — instance type constraint.
    ///
    /// The value of this keyword MUST be either a string or an array.
    /// If it is an array, elements of the array MUST be strings and
    /// MUST be unique.
    ///
    /// String values MUST be one of the six primitive types (`"null"`,
    /// `"boolean"`, `"object"`, `"array"`, `"number"`, or `"string"`),
    /// or `"integer"` which matches any number with a zero fractional
    /// part.
    ///
    /// If the value of `"type"` is a string, then an instance validates
    /// successfully if its type matches the type represented by the
    /// value of the string. If the value of `"type"` is an array, then
    /// an instance validates successfully if its type matches any of the
    /// types indicated by the strings in the array.
    ///
    /// See [JSON Schema Validation §6.1.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.1).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<TypeValue>,

    /// The `enum` keyword — enumerated values constraint.
    ///
    /// The value of this keyword MUST be an array. This array SHOULD
    /// have at least one element. Elements in the array SHOULD be
    /// unique.
    ///
    /// An instance validates successfully against this keyword if its
    /// value is equal to one of the elements in this keyword's array
    /// value.
    ///
    /// Elements in the array might be of any type, including null.
    ///
    /// See [JSON Schema Validation §6.1.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.2).
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_: Option<Vec<Value>>,

    /// Per-enum-value Markdown descriptions (VS Code / non-standard extension).
    #[serde(
        rename = "markdownEnumDescriptions",
        skip_serializing_if = "Option::is_none"
    )]
    pub markdown_enum_descriptions: Option<Vec<Option<String>>>,

    /// The `const` keyword — constant value constraint.
    ///
    /// The value of this keyword MAY be of any type, including null.
    ///
    /// Use of this keyword is functionally equivalent to an `"enum"`
    /// (Section 6.1.2) with a single value.
    ///
    /// An instance validates successfully against this keyword if its
    /// value is equal to the value of the keyword.
    ///
    /// See [JSON Schema Validation §6.1.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.1.3).
    #[serde(rename = "const", skip_serializing_if = "Option::is_none")]
    pub const_: Option<Value>,

    // ---------------------------------------------------------------
    // Object applicators (JSON Schema Core §10.3.2)
    // ---------------------------------------------------------------
    /// The `properties` keyword — per-property subschemas.
    ///
    /// The value of `"properties"` MUST be an object. Each value of
    /// this object MUST be a valid JSON Schema.
    ///
    /// Validation succeeds if, for each name that appears in both the
    /// instance and as a name within this keyword's value, the child
    /// instance for that name successfully validates against the
    /// corresponding schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names matched by this keyword. This annotation affects
    /// the behavior of `"additionalProperties"` (in this vocabulary)
    /// and `"unevaluatedProperties"` in the Unevaluated vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty object.
    ///
    /// See [JSON Schema Core §10.3.2.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.1).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[schemars(extend("default" = {}))]
    pub properties: IndexMap<String, SchemaValue>,

    /// The `patternProperties` keyword — regex-matched property
    /// subschemas.
    ///
    /// The value of `"patternProperties"` MUST be an object. Each
    /// property name of this object SHOULD be a valid regular
    /// expression, according to the ECMA-262 regular expression
    /// dialect. Each property value of this object MUST be a valid
    /// JSON Schema.
    ///
    /// Validation succeeds if, for each instance name that matches any
    /// regular expressions that appear as a property name in this
    /// keyword's value, the child instance for that name successfully
    /// validates against each schema that corresponds to a matching
    /// regular expression.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names matched by this keyword. This annotation affects
    /// the behavior of `"additionalProperties"` (in this vocabulary)
    /// and `"unevaluatedProperties"` (in the Unevaluated vocabulary).
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty object.
    ///
    /// See [JSON Schema Core §10.3.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.2).
    #[serde(
        default,
        rename = "patternProperties",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    #[schemars(extend("default" = {}), extend("propertyNames" = { "format": "regex" }))]
    pub pattern_properties: IndexMap<String, SchemaValue>,

    /// The `additionalProperties` keyword — schema for unmatched
    /// properties.
    ///
    /// The value of `"additionalProperties"` MUST be a valid JSON
    /// Schema.
    ///
    /// The behavior of this keyword depends on the presence and
    /// annotation results of `"properties"` and `"patternProperties"`
    /// within the same schema object. Validation with
    /// `"additionalProperties"` applies only to the child values of
    /// instance names that do not appear in the annotation results of
    /// either `"properties"` or `"patternProperties"`.
    ///
    /// For all such properties, validation succeeds if the child
    /// instance validates against the `"additionalProperties"` schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names validated by this keyword's subschema. This
    /// annotation affects the behavior of `"unevaluatedProperties"` in
    /// the Unevaluated vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §10.3.2.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.3).
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<Box<SchemaValue>>,

    /// The `required` keyword — required property names.
    ///
    /// The value of this keyword MUST be an array. Elements of this
    /// array, if any, MUST be strings, and MUST be unique.
    ///
    /// An object instance is valid against this keyword if every item
    /// in the array is the name of a property in the instance.
    ///
    /// Omitting this keyword has the same behavior as an empty array.
    ///
    /// See [JSON Schema Validation §6.5.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("uniqueItems" = true))]
    pub required: Option<Vec<String>>,

    /// The `propertyNames` keyword — property name schema.
    ///
    /// The value of `"propertyNames"` MUST be a valid JSON Schema.
    ///
    /// If the instance is an object, this keyword validates if every
    /// property name in the instance validates against the provided
    /// schema. Note the property name that the schema is testing will
    /// always be a string.
    ///
    /// Omitting this keyword has the same behavior as an empty schema.
    ///
    /// See [JSON Schema Core §10.3.2.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.2.4).
    #[serde(rename = "propertyNames", skip_serializing_if = "Option::is_none")]
    pub property_names: Option<Box<SchemaValue>>,

    /// The `minProperties` keyword — minimum property count.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An object instance is valid against `"minProperties"` if its
    /// number of properties is greater than, or equal to, the value of
    /// this keyword.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.5.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.2).
    #[serde(rename = "minProperties", skip_serializing_if = "Option::is_none")]
    pub min_properties: Option<u64>,

    /// The `maxProperties` keyword — maximum property count.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An object instance is valid against `"maxProperties"` if its
    /// number of properties is less than, or equal to, the value of
    /// this keyword.
    ///
    /// See [JSON Schema Validation §6.5.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.1).
    #[serde(rename = "maxProperties", skip_serializing_if = "Option::is_none")]
    pub max_properties: Option<u64>,

    /// The `unevaluatedProperties` keyword — schema for properties not
    /// covered by other keywords.
    ///
    /// The value of `"unevaluatedProperties"` MUST be a valid JSON
    /// Schema.
    ///
    /// The behavior of this keyword depends on the annotation results
    /// of adjacent keywords that apply to the instance location being
    /// validated. Specifically, the annotations from `"properties"`,
    /// `"patternProperties"`, and `"additionalProperties"`, which can
    /// come from those keywords when they are adjacent to the
    /// `"unevaluatedProperties"` keyword. Those three annotations, as
    /// well as `"unevaluatedProperties"`, can also result from any and
    /// all adjacent in-place applicator keywords.
    ///
    /// Validation with `"unevaluatedProperties"` applies only to the
    /// child values of instance names that do not appear in the
    /// `"properties"`, `"patternProperties"`, `"additionalProperties"`,
    /// or `"unevaluatedProperties"` annotation results that apply to
    /// the instance location being validated.
    ///
    /// For all such properties, validation succeeds if the child
    /// instance validates against the `"unevaluatedProperties"` schema.
    ///
    /// The annotation result of this keyword is the set of instance
    /// property names validated by this keyword's subschema. This
    /// annotation affects the behavior of `"unevaluatedProperties"` in
    /// parent schemas.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §11.3](https://json-schema.org/draft/2020-12/json-schema-core#section-11.3).
    #[serde(
        rename = "unevaluatedProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub unevaluated_properties: Option<Box<SchemaValue>>,

    // ---------------------------------------------------------------
    // Array applicators (JSON Schema Core §10.3.1)
    // ---------------------------------------------------------------
    /// The `items` keyword — schema for remaining array items.
    ///
    /// The value of `"items"` MUST be a valid JSON Schema.
    ///
    /// This keyword applies its subschema to all instance elements at
    /// indexes greater than the length of the `"prefixItems"` array in
    /// the same schema object, as reported by the annotation result of
    /// that `"prefixItems"` keyword. If no such annotation result
    /// exists, `"items"` applies its subschema to all instance array
    /// elements. Note that the behavior of `"items"` without
    /// `"prefixItems"` is identical to that of the schema form of
    /// `"items"` in prior drafts. When `"prefixItems"` is present, the
    /// behavior of `"items"` is identical to the former
    /// `"additionalItems"` keyword.
    ///
    /// If the `"items"` subschema is applied to any positions within
    /// the instance array, it produces an annotation result of boolean
    /// true, indicating that all remaining array elements have been
    /// evaluated against this keyword's subschema. This annotation
    /// affects the behavior of `"unevaluatedItems"` in the Unevaluated
    /// vocabulary.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §10.3.1.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<SchemaValue>>,

    /// The `prefixItems` keyword — positional array item schemas.
    ///
    /// The value of `"prefixItems"` MUST be a non-empty array of valid
    /// JSON Schemas.
    ///
    /// Validation succeeds if each element of the instance validates
    /// against the schema at the same position, if any. This keyword
    /// does not constrain the length of the array. If the array is
    /// longer than this keyword's value, this keyword validates only
    /// the prefix of matching length.
    ///
    /// This keyword produces an annotation value which is the largest
    /// index to which this keyword applied a subschema. The value MAY
    /// be a boolean true if a subschema was applied to every index of
    /// the instance, such as is produced by the `"items"` keyword.
    /// This annotation affects the behavior of `"items"` and
    /// `"unevaluatedItems"`.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty array.
    ///
    /// See [JSON Schema Core §10.3.1.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.1).
    #[serde(rename = "prefixItems", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub prefix_items: Option<Vec<SchemaValue>>,

    /// The `contains` keyword — array containment schema.
    ///
    /// The value of this keyword MUST be a valid JSON Schema.
    ///
    /// An array instance is valid against `"contains"` if at least one
    /// of its elements is valid against the given schema, except when
    /// `"minContains"` is present and has a value of 0, in which case
    /// an array instance MUST be considered valid against the
    /// `"contains"` keyword, even if none of its elements is valid
    /// against the given schema.
    ///
    /// This keyword produces an annotation value which is an array of
    /// the indexes to which this keyword validates successfully when
    /// applying its subschema, in ascending order. The value MAY be a
    /// boolean `true` if the subschema validates successfully when
    /// applied to every index of the instance.
    ///
    /// The subschema MUST be applied to every array element even after
    /// the first match has been found, in order to collect annotations
    /// for use by other keywords. This is to ensure that all possible
    /// annotations are collected.
    ///
    /// See [JSON Schema Core §10.3.1.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.3.1.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<Box<SchemaValue>>,

    /// The `minContains` keyword — minimum `contains` matches.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// If `"contains"` is not present within the same schema object,
    /// then this keyword has no effect.
    ///
    /// An instance array is valid against `"minContains"` in two ways,
    /// depending on the form of the annotation result of an adjacent
    /// `"contains"` keyword. The first way is if the annotation result
    /// is an array and the length of that array is greater than or
    /// equal to the `"minContains"` value. The second way is if the
    /// annotation result is a boolean `true` and the instance array
    /// length is greater than or equal to the `"minContains"` value.
    ///
    /// A value of 0 is allowed, but is only useful for setting a range
    /// of occurrences from 0 to the value of `"maxContains"`. A value
    /// of 0 causes `"minContains"` and `"contains"` to always pass
    /// validation (but validation can still fail against a
    /// `"maxContains"` keyword).
    ///
    /// Omitting this keyword has the same behavior as a value of 1.
    ///
    /// See [JSON Schema Validation §6.4.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.5).
    #[serde(rename = "minContains", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = 1))]
    pub min_contains: Option<u64>,

    /// The `maxContains` keyword — maximum `contains` matches.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// If `"contains"` is not present within the same schema object,
    /// then this keyword has no effect.
    ///
    /// An instance array is valid against `"maxContains"` in two ways,
    /// depending on the form of the annotation result of an adjacent
    /// `"contains"` keyword. The first way is if the annotation result
    /// is an array and the length of that array is less than or equal
    /// to the `"maxContains"` value. The second way is if the
    /// annotation result is a boolean `true` and the instance array
    /// length is less than or equal to the `"maxContains"` value.
    ///
    /// See [JSON Schema Validation §6.4.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.4).
    #[serde(rename = "maxContains", skip_serializing_if = "Option::is_none")]
    pub max_contains: Option<u64>,

    /// The `minItems` keyword — minimum array length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An array instance is valid against `"minItems"` if its size is
    /// greater than, or equal to, the value of this keyword.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.4.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.2).
    #[serde(rename = "minItems", skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,

    /// The `maxItems` keyword — maximum array length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// An array instance is valid against `"maxItems"` if its size is
    /// less than, or equal to, the value of this keyword.
    ///
    /// See [JSON Schema Validation §6.4.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.1).
    #[serde(rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,

    /// The `uniqueItems` keyword — array element uniqueness.
    ///
    /// The value of this keyword MUST be a boolean.
    ///
    /// If this keyword has boolean value false, the instance validates
    /// successfully. If it has boolean value true, the instance
    /// validates successfully if all of its elements are unique.
    ///
    /// Omitting this keyword has the same behavior as a value of false.
    ///
    /// See [JSON Schema Validation §6.4.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.4.3).
    #[serde(default, rename = "uniqueItems", skip_serializing_if = "is_false")]
    #[schemars(extend("default" = false))]
    pub unique_items: bool,

    /// The `unevaluatedItems` keyword — schema for array items not
    /// covered by other keywords.
    ///
    /// The value of `"unevaluatedItems"` MUST be a valid JSON Schema.
    ///
    /// The behavior of this keyword depends on the annotation results
    /// of adjacent keywords that apply to the instance location being
    /// validated. Specifically, the annotations from `"prefixItems"`,
    /// `"items"`, and `"contains"`, which can come from those keywords
    /// when they are adjacent to the `"unevaluatedItems"` keyword.
    /// Those three annotations, as well as `"unevaluatedItems"`, can
    /// also result from any and all adjacent in-place applicator
    /// keywords.
    ///
    /// If no relevant annotations are present, the
    /// `"unevaluatedItems"` subschema MUST be applied to all locations
    /// in the array. If a boolean true value is present from any of
    /// the relevant annotations, `"unevaluatedItems"` MUST be ignored.
    /// Otherwise, the subschema MUST be applied to any index greater
    /// than the largest annotation value for `"prefixItems"`, which
    /// does not appear in any annotation value for `"contains"`.
    ///
    /// If the `"unevaluatedItems"` subschema is applied to any
    /// positions within the instance array, it produces an annotation
    /// result of boolean true, analogous to the behavior of `"items"`.
    /// This annotation affects the behavior of `"unevaluatedItems"` in
    /// parent schemas.
    ///
    /// Omitting this keyword has the same assertion behavior as an
    /// empty schema.
    ///
    /// See [JSON Schema Core §11.2](https://json-schema.org/draft/2020-12/json-schema-core#section-11.2).
    #[serde(rename = "unevaluatedItems", skip_serializing_if = "Option::is_none")]
    pub unevaluated_items: Option<Box<SchemaValue>>,

    // ---------------------------------------------------------------
    // Numeric validation (JSON Schema Validation §6.2)
    // ---------------------------------------------------------------
    /// The `minimum` keyword — inclusive lower bound.
    ///
    /// The value of `"minimum"` MUST be a number, representing an
    /// inclusive lower limit for a numeric instance.
    ///
    /// If the instance is a number, then this keyword validates only if
    /// the instance is greater than or exactly equal to `"minimum"`.
    ///
    /// See [JSON Schema Validation §6.2.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<Number>,

    /// The `maximum` keyword — inclusive upper bound.
    ///
    /// The value of `"maximum"` MUST be a number, representing an
    /// inclusive upper limit for a numeric instance.
    ///
    /// If the instance is a number, then this keyword validates only if
    /// the instance is less than or exactly equal to `"maximum"`.
    ///
    /// See [JSON Schema Validation §6.2.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<Number>,

    /// The `exclusiveMinimum` keyword — exclusive lower bound.
    ///
    /// The value of `"exclusiveMinimum"` MUST be a number, representing
    /// an exclusive lower limit for a numeric instance.
    ///
    /// If the instance is a number, then the instance is valid only if
    /// it has a value strictly greater than (not equal to)
    /// `"exclusiveMinimum"`.
    ///
    /// See [JSON Schema Validation §6.2.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.5).
    #[serde(rename = "exclusiveMinimum", skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<Number>,

    /// The `exclusiveMaximum` keyword — exclusive upper bound.
    ///
    /// The value of `"exclusiveMaximum"` MUST be a number, representing
    /// an exclusive upper limit for a numeric instance.
    ///
    /// If the instance is a number, then the instance is valid only if
    /// it has a value strictly less than (not equal to)
    /// `"exclusiveMaximum"`.
    ///
    /// See [JSON Schema Validation §6.2.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.3).
    #[serde(rename = "exclusiveMaximum", skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<Number>,

    /// The `multipleOf` keyword — divisibility constraint.
    ///
    /// The value of `"multipleOf"` MUST be a number, strictly greater
    /// than 0.
    ///
    /// A numeric instance is valid only if division by this keyword's
    /// value results in an integer.
    ///
    /// See [JSON Schema Validation §6.2.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.2.1).
    #[serde(rename = "multipleOf", skip_serializing_if = "Option::is_none")]
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub multiple_of: Option<Number>,

    // ---------------------------------------------------------------
    // String validation (JSON Schema Validation §6.3)
    // ---------------------------------------------------------------
    /// The `minLength` keyword — minimum string length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// A string instance is valid against this keyword if its length is
    /// greater than, or equal to, the value of this keyword.
    ///
    /// The length of a string instance is defined as the number of its
    /// characters as defined by RFC 8259.
    ///
    /// Omitting this keyword has the same behavior as a value of 0.
    ///
    /// See [JSON Schema Validation §6.3.2](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.2).
    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,

    /// The `maxLength` keyword — maximum string length.
    ///
    /// The value of this keyword MUST be a non-negative integer.
    ///
    /// A string instance is valid against this keyword if its length is
    /// less than, or equal to, the value of this keyword.
    ///
    /// The length of a string instance is defined as the number of its
    /// characters as defined by RFC 8259.
    ///
    /// See [JSON Schema Validation §6.3.1](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.1).
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,

    /// The `pattern` keyword — regex constraint.
    ///
    /// The value of this keyword MUST be a string. This string SHOULD
    /// be a valid regular expression, according to the ECMA-262
    /// regular expression dialect.
    ///
    /// A string instance is considered valid if the regular expression
    /// matches the instance successfully. Recall: regular expressions
    /// are not implicitly anchored.
    ///
    /// See [JSON Schema Validation §6.3.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.3.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("format" = "regex"))]
    pub pattern: Option<String>,

    /// The `format` keyword — semantic format annotation.
    ///
    /// Structural validation alone may be insufficient to allow an
    /// application to correctly utilize certain values. The `"format"`
    /// annotation keyword is defined to allow schema authors to convey
    /// semantic information for a fixed subset of values which are
    /// accurately described by authoritative resources, be they RFCs or
    /// other external specifications.
    ///
    /// The value of this keyword is called a format attribute. It MUST
    /// be a string.
    ///
    /// See [JSON Schema Validation §7](https://json-schema.org/draft/2020-12/json-schema-validation#section-7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    // ---------------------------------------------------------------
    // Composition applicators (JSON Schema Core §10.2.1)
    // ---------------------------------------------------------------
    /// The `allOf` keyword — conjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against all schemas defined by this
    /// keyword's value.
    ///
    /// See [JSON Schema Core §10.2.1.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.1).
    #[serde(rename = "allOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub all_of: Option<Vec<SchemaValue>>,

    /// The `anyOf` keyword — disjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against at least one schema defined by
    /// this keyword's value. Note that when annotations are being
    /// collected, all subschemas MUST be examined so that annotations
    /// are collected from each subschema that validates successfully.
    ///
    /// See [JSON Schema Core §10.2.1.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.2).
    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub any_of: Option<Vec<SchemaValue>>,

    /// The `oneOf` keyword — exclusive disjunction of subschemas.
    ///
    /// This keyword's value MUST be a non-empty array. Each item of
    /// the array MUST be a valid JSON Schema.
    ///
    /// An instance validates successfully against this keyword if it
    /// validates successfully against exactly one schema defined by
    /// this keyword's value.
    ///
    /// See [JSON Schema Core §10.2.1.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.3).
    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub one_of: Option<Vec<SchemaValue>>,

    /// The `not` keyword — negation.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// An instance is valid against this keyword if it fails to
    /// validate successfully against the schema defined by this
    /// keyword.
    ///
    /// See [JSON Schema Core §10.2.1.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.1.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<SchemaValue>>,

    // ---------------------------------------------------------------
    // Conditional applicators (JSON Schema Core §10.2.2)
    // ---------------------------------------------------------------
    /// The `if` keyword — conditional guard.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// This validation outcome of this keyword's subschema has no
    /// direct effect on the overall validation result. Rather, it
    /// controls which of the `"then"` or `"else"` keywords are
    /// evaluated.
    ///
    /// Instances that successfully validate against this keyword's
    /// subschema MUST also be valid against the subschema value of the
    /// `"then"` keyword, if present.
    ///
    /// Instances that fail to validate against this keyword's
    /// subschema MUST also be valid against the subschema value of the
    /// `"else"` keyword, if present.
    ///
    /// If annotations are being collected, they are collected from
    /// this keyword's subschema in the usual way, including when the
    /// keyword is present without either `"then"` or `"else"`.
    ///
    /// See [JSON Schema Core §10.2.2.1](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.1).
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_: Option<Box<SchemaValue>>,

    /// The `then` keyword — consequent subschema.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// When `"if"` is present, and the instance successfully validates
    /// against its subschema, then validation succeeds against this
    /// keyword if the instance also successfully validates against
    /// this keyword's subschema.
    ///
    /// This keyword has no effect when `"if"` is absent, or when the
    /// instance fails to validate against its subschema.
    /// Implementations MUST NOT evaluate the instance against this
    /// keyword, for either validation or annotation collection
    /// purposes, in such cases.
    ///
    /// See [JSON Schema Core §10.2.2.2](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.2).
    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_: Option<Box<SchemaValue>>,

    /// The `else` keyword — alternative subschema.
    ///
    /// This keyword's value MUST be a valid JSON Schema.
    ///
    /// When `"if"` is present, and the instance fails to validate
    /// against its subschema, then validation succeeds against this
    /// keyword if the instance successfully validates against this
    /// keyword's subschema.
    ///
    /// This keyword has no effect when `"if"` is absent, or when the
    /// instance successfully validates against its subschema.
    /// Implementations MUST NOT evaluate the instance against this
    /// keyword, for either validation or annotation collection
    /// purposes, in such cases.
    ///
    /// See [JSON Schema Core §10.2.2.3](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.3).
    #[serde(rename = "else", skip_serializing_if = "Option::is_none")]
    pub else_: Option<Box<SchemaValue>>,

    // ---------------------------------------------------------------
    // Dependency keywords
    // ---------------------------------------------------------------
    /// The `dependentRequired` keyword — conditional required
    /// properties.
    ///
    /// The value of this keyword MUST be an object. Properties in this
    /// object, if any, MUST be arrays. Elements in each array, if any,
    /// MUST be strings, and MUST be unique.
    ///
    /// This keyword specifies properties that are required if a
    /// specific other property is present. Their requirement is
    /// dependent on the presence of the other property.
    ///
    /// Validation succeeds if, for each name that appears in both the
    /// instance and as a name within this keyword's value, every item
    /// in the corresponding array is also the name of a property in
    /// the instance.
    ///
    /// Omitting this keyword has the same behavior as an empty object.
    ///
    /// See [JSON Schema Validation §6.5.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-6.5.4).
    #[serde(rename = "dependentRequired", skip_serializing_if = "Option::is_none")]
    pub dependent_required: Option<IndexMap<String, Vec<String>>>,

    /// The `dependentSchemas` keyword — conditional subschemas by
    /// property name.
    ///
    /// This keyword specifies subschemas that are evaluated if the
    /// instance is an object and contains a certain property.
    ///
    /// This keyword's value MUST be an object. Each value in the
    /// object MUST be a valid JSON Schema.
    ///
    /// If the object key is a property in the instance, the entire
    /// instance must validate against the subschema. Its use is
    /// dependent on the presence of the property.
    ///
    /// Omitting this keyword has the same behavior as an empty object.
    ///
    /// See [JSON Schema Core §10.2.2.4](https://json-schema.org/draft/2020-12/json-schema-core#section-10.2.2.4).
    #[serde(
        default,
        rename = "dependentSchemas",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    #[schemars(extend("default" = {}))]
    pub dependent_schemas: IndexMap<String, SchemaValue>,

    // ---------------------------------------------------------------
    // Content vocabulary (JSON Schema Validation §8)
    // ---------------------------------------------------------------
    /// The `contentMediaType` keyword — media type of string content.
    ///
    /// If the instance is a string, this property indicates the media
    /// type of the contents of the string. If `"contentEncoding"` is
    /// present, this property describes the decoded string.
    ///
    /// The value of this property MUST be a string, which MUST be a
    /// media type, as defined by RFC 2046.
    ///
    /// See [JSON Schema Validation §8.4](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.4).
    #[serde(rename = "contentMediaType", skip_serializing_if = "Option::is_none")]
    pub content_media_type: Option<String>,

    /// The `contentEncoding` keyword — encoding of string content.
    ///
    /// If the instance value is a string, this property defines that
    /// the string SHOULD be interpreted as encoded binary data and
    /// decoded using the encoding named by this property.
    ///
    /// Possible values indicating base 16, 32, and 64 encodings with
    /// several variations are listed in RFC 4648. Additionally,
    /// sections 6.7 and 6.8 of RFC 2045 provide encodings used in
    /// MIME.
    ///
    /// If this keyword is absent, but `"contentMediaType"` is present,
    /// this indicates that the encoding is the identity encoding,
    /// meaning that no transformation was needed in order to represent
    /// the content in a UTF-8 string.
    ///
    /// The value of this property MUST be a string.
    ///
    /// See [JSON Schema Validation §8.3](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.3).
    #[serde(rename = "contentEncoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,

    /// The `contentSchema` keyword — schema for decoded string content.
    ///
    /// If the instance is a string, and if `"contentMediaType"` is
    /// present, this property contains a schema which describes the
    /// structure of the string.
    ///
    /// This keyword MAY be used with any media type that can be mapped
    /// into JSON Schema's data model.
    ///
    /// The value of this property MUST be a valid JSON schema. It
    /// SHOULD be ignored if `"contentMediaType"` is not present.
    ///
    /// See [JSON Schema Validation §8.5](https://json-schema.org/draft/2020-12/json-schema-validation#section-8.5).
    #[serde(rename = "contentSchema", skip_serializing_if = "Option::is_none")]
    pub content_schema: Option<Box<SchemaValue>>,

    // ---------------------------------------------------------------
    // Extension keywords (non-standard)
    // ---------------------------------------------------------------
    /// Taplo TOML toolkit extension (`x-taplo`).
    #[serde(rename = "x-taplo", skip_serializing_if = "Option::is_none")]
    pub x_taplo: Option<TaploSchemaExt>,
    /// Taplo informational metadata (`x-taplo-info`).
    #[serde(rename = "x-taplo-info", skip_serializing_if = "Option::is_none")]
    pub x_taplo_info: Option<TaploInfoSchemaExt>,
    /// Tombi TOML extensions (`x-tombi-*`).
    #[serde(flatten)]
    pub x_tombi: TombiSchemaExt,
    /// `IntelliJ` IDEA extensions (`x-intellij-*`).
    #[serde(flatten)]
    pub x_intellij: IntellijSchemaExt,

    // ---------------------------------------------------------------
    // Catch-all
    // ---------------------------------------------------------------
    /// Unknown or unsupported properties.
    ///
    /// Any JSON property that is not recognized as a standard keyword
    /// or known extension is captured here, preserving round-trip
    /// fidelity.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl SchemaValue {
    /// Get the inner `Schema` if this is an object schema, `None` for bool schemas.
    pub fn as_schema(&self) -> Option<&Schema> {
        match self {
            Self::Schema(s) => Some(s),
            Self::Bool(_) => None,
        }
    }
}

impl Schema {
    /// Parse from a `serde_json::Value` without migration.
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be deserialized into a `Schema`.
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Get the best description text, preferring `markdownDescription`.
    pub fn description(&self) -> Option<&str> {
        self.markdown_description
            .as_deref()
            .or(self.description.as_deref())
    }

    /// Get the required fields, or an empty slice.
    pub fn required_set(&self) -> &[String] {
        self.required.as_deref().unwrap_or_default()
    }

    /// Whether this schema is deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.deprecated
    }

    /// Produce a short human-readable type string.
    pub fn type_str(&self) -> Option<String> {
        schema_type_str(self)
    }

    /// Look up a schema-keyword field by its JSON key name.
    ///
    /// Returns a reference to the `SchemaValue` stored under that keyword,
    /// or `None` if the field is absent.
    pub fn get_keyword(&self, key: &str) -> Option<&SchemaValue> {
        match key {
            "items" => self.items.as_deref(),
            "contains" => self.contains.as_deref(),
            "additionalProperties" => self.additional_properties.as_deref(),
            "propertyNames" => self.property_names.as_deref(),
            "unevaluatedProperties" => self.unevaluated_properties.as_deref(),
            "unevaluatedItems" => self.unevaluated_items.as_deref(),
            "not" => self.not.as_deref(),
            "if" => self.if_.as_deref(),
            "then" => self.then_.as_deref(),
            "else" => self.else_.as_deref(),
            "contentSchema" => self.content_schema.as_deref(),
            _ => None,
        }
    }

    /// Look up a named child within a keyword that holds a map of schemas.
    ///
    /// For example, `get_map_entry("properties", "name")` returns the schema
    /// for the `name` property.
    pub fn get_map_entry(&self, keyword: &str, key: &str) -> Option<&SchemaValue> {
        match keyword {
            "properties" => self.properties.get(key),
            "patternProperties" => self.pattern_properties.get(key),
            "$defs" => self.defs.as_ref()?.get(key),
            "dependentSchemas" => self.dependent_schemas.get(key),
            _ => None,
        }
    }

    /// Look up an indexed child within a keyword that holds an array of schemas.
    pub fn get_array_entry(&self, keyword: &str, index: usize) -> Option<&SchemaValue> {
        match keyword {
            "allOf" => self.all_of.as_ref()?.get(index),
            "anyOf" => self.any_of.as_ref()?.get(index),
            "oneOf" => self.one_of.as_ref()?.get(index),
            "prefixItems" => self.prefix_items.as_ref()?.get(index),
            _ => None,
        }
    }
}

/// Produce a short human-readable type string for a schema.
fn schema_type_str(schema: &Schema) -> Option<String> {
    // Explicit type field
    if let Some(ref ty) = schema.type_ {
        return match ty {
            TypeValue::Single(s) if *s == SimpleType::Array => {
                let item_ty = schema
                    .items
                    .as_ref()
                    .and_then(|sv| sv.as_schema())
                    .and_then(schema_type_str);
                match item_ty {
                    Some(item_ty) => Some(format!("{item_ty}[]")),
                    None => Some("array".to_string()),
                }
            }
            TypeValue::Single(s) => Some(s.to_string()),
            TypeValue::Union(arr) => Some(
                arr.iter()
                    .map(SimpleType::to_string)
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
        };
    }

    // $ref
    if let Some(ref r) = schema.ref_ {
        return Some(ref_name(r).to_string());
    }

    // oneOf/anyOf
    for variants in [&schema.one_of, &schema.any_of].into_iter().flatten() {
        let types: Vec<String> = variants
            .iter()
            .filter_map(|v| match v {
                SchemaValue::Schema(s) => {
                    schema_type_str(s).or_else(|| s.ref_.as_ref().map(|r| ref_name(r).to_string()))
                }
                SchemaValue::Bool(_) => None,
            })
            .collect();
        if !types.is_empty() {
            return Some(types.join(" | "));
        }
    }

    // const
    if let Some(ref c) = schema.const_ {
        return Some(format!("const: {c}"));
    }

    // enum
    if schema.enum_.is_some() {
        return Some("enum".to_string());
    }

    None
}

/// Extract the trailing name from a `$ref` path (e.g. `"#/$defs/Foo"` -> `"Foo"`).
pub fn ref_name(ref_str: &str) -> &str {
    ref_str.rsplit('/').next().unwrap_or(ref_str)
}

/// Resolve a `$ref` within the same schema document.
///
/// If the given schema has a `$ref` that begins with `#/`, follow the path
/// through the root schema. Otherwise return the schema unchanged.
pub fn resolve_ref<'a>(schema: &'a Schema, root: &'a Schema) -> &'a Schema {
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        // Navigate the root using serde_json::Value for flexibility
        let Ok(root_value) = serde_json::to_value(root) else {
            return schema;
        };
        let mut current = &root_value;
        for segment in path.split('/') {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            match current.get(&decoded) {
                Some(next) => current = next,
                None => return schema,
            }
        }
        // Try to deserialize the resolved value back into a Schema.
        // This is expensive, so we use a different approach for the explain crate.
        // For now, just return the original schema — the explain crate has its own
        // resolve_ref that works with SchemaValue trees directly.
        let _ = current;
        return schema;
    }
    schema
}

/// Walk a JSON Pointer path through a schema, resolving `$ref` at each step.
///
/// Segments are decoded per RFC 6901 (`~1` → `/`, `~0` → `~`).
/// Returns the sub-`SchemaValue` at the given pointer, or an error.
///
/// # Errors
///
/// Returns an error if a segment in the pointer cannot be resolved.
pub fn navigate_pointer<'a>(
    schema: &'a SchemaValue,
    root: &'a SchemaValue,
    pointer: &str,
) -> Result<&'a SchemaValue, String> {
    let path = pointer.strip_prefix('/').unwrap_or(pointer);
    if path.is_empty() {
        return Ok(schema);
    }

    let mut current = resolve_schema_value_ref(schema, root);
    let mut segments = path.split('/').peekable();

    while let Some(segment) = segments.next() {
        let decoded = segment.replace("~1", "/").replace("~0", "~");
        current = resolve_schema_value_ref(current, root);

        let Some(schema) = current.as_schema() else {
            return Err(format!(
                "cannot resolve segment '{decoded}' in pointer '{pointer}'"
            ));
        };

        // Map-bearing keywords: consume this segment AND the next one.
        if is_map_keyword(&decoded) {
            let key_segment = segments
                .next()
                .ok_or_else(|| format!("expected key after '{decoded}' in pointer '{pointer}'"))?;
            let key = key_segment.replace("~1", "/").replace("~0", "~");
            if let Some(entry) = schema.get_map_entry(&decoded, &key) {
                current = entry;
                continue;
            }
            return Err(format!(
                "cannot resolve segment '{key}' in '{decoded}' in pointer '{pointer}'"
            ));
        }

        // Array-bearing keywords: consume this segment, then the next as an index.
        if is_array_keyword(&decoded) {
            let idx_segment = segments.next().ok_or_else(|| {
                format!("expected index after '{decoded}' in pointer '{pointer}'")
            })?;
            let idx: usize = idx_segment.parse().map_err(|_| {
                format!("expected numeric index after '{decoded}', got '{idx_segment}'")
            })?;
            if let Some(entry) = schema.get_array_entry(&decoded, idx) {
                current = entry;
                continue;
            }
            return Err(format!(
                "index {idx} out of bounds in '{decoded}' in pointer '{pointer}'"
            ));
        }

        // Single-value keywords (items, not, if, then, else, etc.)
        if let Some(sv) = schema.get_keyword(&decoded) {
            current = sv;
            continue;
        }

        // Fall back: try as a key in the schema's maps (for when the
        // pointer navigates directly into a map without naming the keyword).
        if let Some(sv) = schema.get_map_entry_by_pointer_segment(&decoded) {
            current = sv;
            continue;
        }

        // Try as array index (for arrays embedded in composition keywords)
        if let Ok(idx) = decoded.parse::<usize>() {
            let found = ["allOf", "anyOf", "oneOf", "prefixItems"]
                .iter()
                .find_map(|kw| schema.get_array_entry(kw, idx));
            if let Some(entry) = found {
                current = entry;
                continue;
            }
        }

        return Err(format!(
            "cannot resolve segment '{decoded}' in pointer '{pointer}'"
        ));
    }

    Ok(resolve_schema_value_ref(current, root))
}

/// Whether a JSON pointer segment names a map-bearing keyword.
fn is_map_keyword(segment: &str) -> bool {
    matches!(
        segment,
        "properties" | "patternProperties" | "$defs" | "dependentSchemas"
    )
}

/// Whether a JSON pointer segment names an array-bearing keyword.
fn is_array_keyword(segment: &str) -> bool {
    matches!(segment, "allOf" | "anyOf" | "oneOf" | "prefixItems")
}

/// Resolve `$ref` on a `SchemaValue`, returning the referenced `SchemaValue`.
fn resolve_schema_value_ref<'a>(sv: &'a SchemaValue, root: &'a SchemaValue) -> &'a SchemaValue {
    let Some(schema) = sv.as_schema() else {
        return sv;
    };
    if let Some(ref ref_str) = schema.ref_
        && let Some(path) = ref_str.strip_prefix("#/")
    {
        let mut current = root;
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            let decoded = segment.replace("~1", "/").replace("~0", "~");
            let Some(inner) = current.as_schema() else {
                return sv;
            };

            // Map-bearing keywords: consume the next segment as a key
            if is_map_keyword(&decoded) {
                let Some(key_segment) = segments.next() else {
                    return sv;
                };
                let key = key_segment.replace("~1", "/").replace("~0", "~");
                match inner.get_map_entry(&decoded, &key) {
                    Some(n) => current = n,
                    None => return sv,
                }
                continue;
            }

            // Array-bearing keywords: consume the next segment as an index
            if is_array_keyword(&decoded) {
                let Some(idx_segment) = segments.next() else {
                    return sv;
                };
                let Ok(idx) = idx_segment.parse::<usize>() else {
                    return sv;
                };
                match inner.get_array_entry(&decoded, idx) {
                    Some(n) => current = n,
                    None => return sv,
                }
                continue;
            }

            // Single-value keywords
            if let Some(n) = inner.get_keyword(&decoded) {
                current = n;
                continue;
            }

            // Fall back to map entry lookup
            if let Some(n) = inner.get_map_entry_by_pointer_segment(&decoded) {
                current = n;
                continue;
            }

            return sv;
        }
        return current;
    }
    sv
}

impl Schema {
    /// Look up a child by a JSON pointer segment name.
    /// This handles both map keywords (where the segment is a key within the map)
    /// and direct keywords.
    fn get_map_entry_by_pointer_segment(&self, segment: &str) -> Option<&SchemaValue> {
        // Try all map-bearing keyword fields.
        // For pointer navigation, when we're inside a "properties" object,
        // the segment is the property name.
        self.properties
            .get(segment)
            .or_else(|| self.pattern_properties.get(segment))
            .or_else(|| self.defs.as_ref().and_then(|m| m.get(segment)))
            .or_else(|| self.dependent_schemas.get(segment))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_simple_schema() {
        let json = json!({
            "type": "object",
            "title": "Test",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let schema: Schema = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(schema.title.as_deref(), Some("Test"));
        assert!(!schema.properties.is_empty());

        let back = serde_json::to_value(&schema).unwrap();
        assert_eq!(back["type"], "object");
        assert_eq!(back["title"], "Test");
    }

    #[test]
    fn bool_schema_value() {
        let json = json!(true);
        let sv: SchemaValue = serde_json::from_value(json).unwrap();
        assert!(matches!(sv, SchemaValue::Bool(true)));
        assert!(sv.as_schema().is_none());
    }

    #[test]
    fn schema_value_object() {
        let json = json!({"type": "string"});
        let sv: SchemaValue = serde_json::from_value(json).unwrap();
        let s = sv.as_schema().unwrap();
        assert!(matches!(
            s.type_,
            Some(TypeValue::Single(SimpleType::String))
        ));
    }

    #[test]
    fn type_value_single() {
        let json = json!("string");
        let tv: TypeValue = serde_json::from_value(json).unwrap();
        assert!(matches!(tv, TypeValue::Single(SimpleType::String)));
    }

    #[test]
    fn type_value_union() {
        let json = json!(["string", "null"]);
        let tv: TypeValue = serde_json::from_value(json).unwrap();
        assert!(matches!(tv, TypeValue::Union(ref v) if v.len() == 2));
    }

    #[test]
    fn simple_type_display() {
        assert_eq!(SimpleType::Array.to_string(), "array");
        assert_eq!(SimpleType::Boolean.to_string(), "boolean");
        assert_eq!(SimpleType::Integer.to_string(), "integer");
        assert_eq!(SimpleType::Null.to_string(), "null");
        assert_eq!(SimpleType::Number.to_string(), "number");
        assert_eq!(SimpleType::Object.to_string(), "object");
        assert_eq!(SimpleType::String.to_string(), "string");
    }

    #[test]
    fn simple_type_round_trip() {
        for ty in [
            SimpleType::Array,
            SimpleType::Boolean,
            SimpleType::Integer,
            SimpleType::Null,
            SimpleType::Number,
            SimpleType::Object,
            SimpleType::String,
        ] {
            let json = serde_json::to_value(ty).unwrap();
            let back: SimpleType = serde_json::from_value(json).unwrap();
            assert_eq!(ty, back);
        }
    }

    #[test]
    fn description_prefers_markdown() {
        let schema = Schema {
            description: Some("plain".into()),
            markdown_description: Some("**rich**".into()),
            ..Default::default()
        };
        assert_eq!(schema.description(), Some("**rich**"));
    }

    #[test]
    fn description_falls_back() {
        let schema = Schema {
            description: Some("plain".into()),
            ..Default::default()
        };
        assert_eq!(schema.description(), Some("plain"));
    }

    #[test]
    fn type_str_simple() {
        let schema = Schema {
            type_: Some(TypeValue::Single(SimpleType::String)),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string"));
    }

    #[test]
    fn type_str_union() {
        let schema = Schema {
            type_: Some(TypeValue::Union(vec![SimpleType::String, SimpleType::Null])),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string | null"));
    }

    #[test]
    fn type_str_array_with_items() {
        let items = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single(SimpleType::String)),
            ..Default::default()
        }));
        let schema = Schema {
            type_: Some(TypeValue::Single(SimpleType::Array)),
            items: Some(Box::new(items)),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("string[]"));
    }

    #[test]
    fn type_str_ref() {
        let schema = Schema {
            ref_: Some("#/$defs/Foo".into()),
            ..Default::default()
        };
        assert_eq!(schema.type_str().as_deref(), Some("Foo"));
    }

    #[test]
    fn is_deprecated_default_false() {
        let schema = Schema::default();
        assert!(!schema.is_deprecated());
    }

    #[test]
    fn is_deprecated_true() {
        let schema = Schema {
            deprecated: true,
            ..Default::default()
        };
        assert!(schema.is_deprecated());
    }

    #[test]
    fn required_set_empty() {
        let schema = Schema::default();
        assert!(schema.required_set().is_empty());
    }

    #[test]
    fn required_set_values() {
        let schema = Schema {
            required: Some(vec!["a".into(), "b".into()]),
            ..Default::default()
        };
        assert_eq!(schema.required_set(), &["a", "b"]);
    }

    #[test]
    fn extra_fields_preserved() {
        let json = json!({
            "type": "object",
            "x-custom": "value",
            "x-another": 42
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert_eq!(schema.extra.get("x-custom").unwrap(), "value");
        assert_eq!(schema.extra.get("x-another").unwrap(), 42);
    }

    #[test]
    fn x_taplo_deserialization() {
        let json = json!({
            "type": "object",
            "x-taplo": {
                "hidden": true,
                "docs": {
                    "main": "Main docs"
                }
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        let taplo = schema.x_taplo.unwrap();
        assert_eq!(taplo.hidden, Some(true));
        assert_eq!(taplo.docs.unwrap().main.as_deref(), Some("Main docs"));
    }

    #[test]
    fn x_intellij_deserialization() {
        let json = json!({
            "type": "string",
            "enum": ["system", "local"],
            "x-intellij-html-description": "<b>bold</b> description",
            "x-intellij-language-injection": "Shell Script",
            "x-intellij-enum-metadata": {
                "system": { "description": "Use system nginx" },
                "local": { "description": "Use local nginx process" }
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert_eq!(
            schema.x_intellij.html_description.as_deref(),
            Some("<b>bold</b> description")
        );
        assert_eq!(
            schema.x_intellij.language_injection.as_deref(),
            Some("Shell Script")
        );
        let meta = schema.x_intellij.enum_metadata.unwrap();
        assert_eq!(meta.len(), 2);
        assert_eq!(
            meta["system"].description.as_deref(),
            Some("Use system nginx")
        );
    }

    #[test]
    fn x_intellij_fixture_huskyrc() {
        let content = include_str!("../tests/fixtures/huskyrc.json");
        let value: Value = serde_json::from_str(content).expect("parse huskyrc.json");
        let mut migrated = value;
        jsonschema_migrate::migrate_to_2020_12(&mut migrated);
        let schema: Schema = serde_json::from_value(migrated).expect("deserialize huskyrc schema");

        // definitions/hook has x-intellij-language-injection
        let hook = schema.defs.as_ref().expect("defs present")["hook"]
            .as_schema()
            .expect("hook is a schema");
        assert_eq!(
            hook.x_intellij.language_injection.as_deref(),
            Some("Shell Script")
        );

        // hooks/applypatch-msg has x-intellij-html-description
        let hooks = &schema.properties["hooks"]
            .as_schema()
            .expect("hooks is a schema");
        let applypatch = &hooks.properties["applypatch-msg"]
            .as_schema()
            .expect("applypatch-msg is a schema");
        assert!(
            applypatch
                .x_intellij
                .html_description
                .as_ref()
                .expect("html_description present")
                .starts_with("<p>This hook is invoked by")
        );

        // Neither should leak into extra
        assert!(!hook.extra.contains_key("x-intellij-language-injection"));
        assert!(!applypatch.extra.contains_key("x-intellij-html-description"));
    }

    #[test]
    fn x_intellij_fixture_monade() {
        let content = include_str!("../tests/fixtures/monade-stack-config.json");
        let value: Value = serde_json::from_str(content).expect("parse monade-stack-config.json");
        let mut migrated = value;
        jsonschema_migrate::migrate_to_2020_12(&mut migrated);
        let schema: Schema = serde_json::from_value(migrated).expect("deserialize monade schema");

        // properties/nginx has x-intellij-enum-metadata
        let nginx = &schema.properties["nginx"]
            .as_schema()
            .expect("nginx is a schema");
        let meta = nginx
            .x_intellij
            .enum_metadata
            .as_ref()
            .expect("enum_metadata present");
        assert_eq!(meta.len(), 2);
        assert_eq!(
            meta["system"].description.as_deref(),
            Some("Use system nginx")
        );
        assert_eq!(
            meta["local"].description.as_deref(),
            Some("Use local nginx process")
        );
        assert!(!nginx.extra.contains_key("x-intellij-enum-metadata"));
    }

    #[test]
    fn x_intellij_not_in_extra() {
        let json = json!({
            "type": "string",
            "x-intellij-html-description": "hello",
            "x-custom": "other"
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert!(schema.x_intellij.html_description.is_some());
        // x-intellij should NOT leak into extra
        assert!(!schema.extra.contains_key("x-intellij-html-description"));
        // but other x-* should still be in extra
        assert!(schema.extra.contains_key("x-custom"));
    }

    #[test]
    fn x_lintel_deserialization() {
        let json = json!({
            "type": "object",
            "x-lintel": {
                "source": "https://example.com/schema.json",
                "sourceSha256": "abc123"
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        let lintel = schema.x_lintel.unwrap();
        assert_eq!(
            lintel.source.as_deref(),
            Some("https://example.com/schema.json")
        );
        assert_eq!(lintel.source_sha256.as_deref(), Some("abc123"));
    }

    #[test]
    fn navigate_pointer_empty() {
        let sv = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single(SimpleType::Object)),
            ..Default::default()
        }));
        let result = navigate_pointer(&sv, &sv, "").unwrap();
        assert!(result.as_schema().is_some());
    }

    #[test]
    fn navigate_pointer_properties() {
        let name_schema = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single(SimpleType::String)),
            ..Default::default()
        }));
        let mut props = IndexMap::new();
        props.insert("name".into(), name_schema);
        let root = SchemaValue::Schema(Box::new(Schema {
            properties: props,
            ..Default::default()
        }));
        let result = navigate_pointer(&root, &root, "/properties/name").unwrap();
        let s = result.as_schema().unwrap();
        assert!(matches!(
            s.type_,
            Some(TypeValue::Single(SimpleType::String))
        ));
    }

    #[test]
    fn navigate_pointer_resolves_ref() {
        let item_schema = SchemaValue::Schema(Box::new(Schema {
            type_: Some(TypeValue::Single(SimpleType::Object)),
            description: Some("An item".into()),
            ..Default::default()
        }));
        let ref_schema = SchemaValue::Schema(Box::new(Schema {
            ref_: Some("#/$defs/Item".into()),
            ..Default::default()
        }));
        let mut defs = BTreeMap::new();
        defs.insert("Item".into(), item_schema);
        let mut props = IndexMap::new();
        props.insert("item".into(), ref_schema);
        let root = SchemaValue::Schema(Box::new(Schema {
            properties: props,
            defs: Some(defs),
            ..Default::default()
        }));
        let result = navigate_pointer(&root, &root, "/properties/item").unwrap();
        let s = result.as_schema().unwrap();
        assert_eq!(s.description.as_deref(), Some("An item"));
    }

    #[test]
    fn navigate_pointer_bad_segment_errors() {
        let sv = SchemaValue::Schema(Box::default());
        let err = navigate_pointer(&sv, &sv, "/nonexistent").unwrap_err();
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn parse_cargo_fixture() {
        let content = include_str!("../../jsonschema-migrate/tests/fixtures/cargo.json");
        let value: Value = serde_json::from_str(content).expect("parse cargo.json");
        let mut migrated = value;
        jsonschema_migrate::migrate_to_2020_12(&mut migrated);
        let schema: Schema = serde_json::from_value(migrated).expect("deserialize cargo schema");
        assert!(schema.title.is_some() || schema.type_.is_some());
        // Verify x-taplo is parsed if present
        if schema.x_taplo.is_some() {
            // Just verify it parsed without error
        }
    }

    #[test]
    fn numeric_fields_round_trip() {
        let json = json!({
            "type": "number",
            "minimum": 0,
            "maximum": 100.5,
            "exclusiveMinimum": -1,
            "exclusiveMaximum": 101,
            "multipleOf": 0.5
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert_eq!(schema.minimum.as_ref().unwrap().to_string(), "0");
        assert_eq!(schema.maximum.as_ref().unwrap().to_string(), "100.5");
        assert_eq!(schema.exclusive_minimum.as_ref().unwrap().to_string(), "-1");
        assert_eq!(
            schema.exclusive_maximum.as_ref().unwrap().to_string(),
            "101"
        );
        assert_eq!(schema.multiple_of.as_ref().unwrap().to_string(), "0.5");

        let back = serde_json::to_value(&schema).unwrap();
        assert_eq!(back["minimum"], 0);
        assert_eq!(back["maximum"], 100.5);
    }

    #[test]
    fn schema_url_round_trip() {
        let json = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object"
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        assert_eq!(
            schema.schema.as_ref().unwrap().as_str(),
            "https://json-schema.org/draft/2020-12/schema"
        );

        let back = serde_json::to_value(&schema).unwrap();
        assert_eq!(
            back["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn vocabulary_round_trip() {
        let json = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$vocabulary": {
                "https://json-schema.org/draft/2020-12/vocab/core": true,
                "https://json-schema.org/draft/2020-12/vocab/applicator": true,
                "https://json-schema.org/draft/2020-12/vocab/validation": false
            }
        });
        let schema: Schema = serde_json::from_value(json).unwrap();
        let vocab = schema.vocabulary.as_ref().unwrap();
        assert_eq!(vocab.len(), 3);

        let core_url: Url = "https://json-schema.org/draft/2020-12/vocab/core"
            .parse()
            .unwrap();
        assert_eq!(vocab.get(&core_url), Some(&true));

        let validation_url: Url = "https://json-schema.org/draft/2020-12/vocab/validation"
            .parse()
            .unwrap();
        assert_eq!(vocab.get(&validation_url), Some(&false));

        let back = serde_json::to_value(&schema).unwrap();
        assert_eq!(
            back["$vocabulary"]["https://json-schema.org/draft/2020-12/vocab/core"],
            true
        );
    }
}
