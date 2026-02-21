use saphyr_parser::{ScalarStyle, Span};

/// A comment with its source column, used to compute correct indentation.
#[derive(Debug, Clone)]
pub(crate) struct Comment {
    pub text: String,            // the comment text (starting with `#`)
    pub col: usize,              // 0-indexed source column of `#`
    pub line: usize,             // 1-indexed source line number
    pub blank_line_before: bool, // whether there's a blank line before this comment in the source
}

pub(crate) struct YamlStream {
    pub documents: Vec<YamlDoc>,
    pub trailing_comments: Vec<Comment>,
}

pub(crate) struct YamlDoc {
    pub explicit_start: bool,
    pub explicit_end: bool,
    /// Preamble lines before `---`: directives (%YAML, %TAG) and comments, in source order.
    pub preamble: Vec<String>,
    pub root: Option<Node>,
    pub end_comments: Vec<Comment>, // comments after root, before next doc or end
    /// Inline comment on the `---` document start marker (e.g. `--- # comment`)
    pub start_comment: Option<String>,
    /// Inline comment on the `...` document end marker (e.g. `... # Suffix`)
    pub end_marker_comment: Option<String>,
    /// Trailing comment on the root node's last line (e.g. `!!int 1 - 3 # comment`)
    pub root_trailing_comment: Option<String>,
    /// Raw body source for prettier-ignore documents
    pub raw_body_source: Option<String>,
    /// Comments between `---` and the root body (e.g. for scalar roots)
    pub body_leading_comments: Vec<Comment>,
}

pub(crate) enum Node {
    Scalar(ScalarNode),
    Mapping(MappingNode),
    Sequence(SequenceNode),
    Alias(AliasNode),
}

#[allow(dead_code)]
pub(crate) struct ScalarNode {
    pub value: String,
    pub style: ScalarStyle,
    pub anchor: Option<String>,
    pub tag: Option<String>,
    /// For block scalars: the raw source text (indicator + body)
    pub block_source: Option<String>,
    /// For quoted scalars: the raw source text between delimiters (preserves original escapes)
    pub quoted_source: Option<String>,
    /// Whether this is an implicit null (empty value after `key:`)
    pub is_implicit_null: bool,
    pub span: Span,
    /// For plain scalars that span multiple source lines: the trimmed content per source line.
    /// Used by preserve mode to reconstruct original line structure.
    pub plain_source_lines: Option<Vec<String>>,
    /// Comments between tag/anchor and the scalar content.
    pub middle_comments: Vec<Comment>,
}

#[allow(dead_code)]
pub(crate) struct MappingNode {
    pub entries: Vec<MappingEntry>,
    pub flow: bool,
    pub anchor: Option<String>,
    pub tag: Option<String>,
    /// For flow mappings, store the raw source if we want to preserve it
    pub flow_source: Option<String>,
    /// Whether this mapping had explicit `{ ... }` braces in the source.
    /// Detected by checking `span.start != span.end` for `MappingStart` events.
    pub has_explicit_braces: bool,
    pub middle_comments: Vec<Comment>, // comments between tag/anchor and first entry
    pub trailing_comments: Vec<Comment>, // comments after last entry before MappingEnd
}

pub(crate) struct MappingEntry {
    pub key: Node,
    pub value: Node,
    pub leading_comments: Vec<Comment>,
    pub key_trailing_comment: Option<String>, // trailing comment on key line (e.g. "hr: # comment")
    pub between_comments: Vec<Comment>,       // standalone comments between key and value
    pub blank_line_before_value: bool,        // blank line between last between_comment and value
    /// Trailing comment on the `:` line of an explicit key (e.g. `? key\n: # comment\n  value`).
    /// Unlike `between_comments`, this does NOT force explicit key format.
    pub colon_comment: Option<Comment>,
    pub trailing_comment: Option<String>,
    pub blank_line_before: bool,
    pub is_explicit_key: bool,
    /// Inline comment on the `?` indicator line when key is on a separate line.
    /// e.g. `? # comment\n  key` → `question_mark_comment` = Some("# comment")
    pub question_mark_comment: Option<String>,
    /// Raw source text for prettier-ignore entries (de-indented)
    pub raw_source: Option<String>,
}

#[allow(dead_code)]
pub(crate) struct SequenceNode {
    pub items: Vec<SequenceItem>,
    pub flow: bool,
    pub anchor: Option<String>,
    pub tag: Option<String>,
    pub flow_source: Option<String>,
    pub middle_comments: Vec<Comment>,
    pub trailing_comments: Vec<Comment>, // comments after last item before SequenceEnd
    /// Inline comment on the closing `]` of a flow sequence (e.g. `] # comment`)
    pub closing_comment: Option<String>,
}

pub(crate) struct SequenceItem {
    pub value: Node,
    pub leading_comments: Vec<Comment>,
    pub trailing_comment: Option<String>,
    pub blank_line_before: bool,
    /// Whether this item has a `# prettier-ignore` leading comment
    pub prettier_ignore: bool,
    /// Inline comment on the `- ` indicator line when value is on the next line.
    /// e.g. `- #comment\n    value` → `indicator_comment` = Some("#comment")
    pub indicator_comment: Option<String>,
}

pub(crate) struct AliasNode {
    pub name: String,
}
