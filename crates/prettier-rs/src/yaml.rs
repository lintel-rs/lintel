use std::borrow::Cow;
use std::fmt::Write;

use anyhow::{Context, Result};
use saphyr_parser::{Event, Parser, ScalarStyle, Span, Tag};

use crate::options::PrettierOptions;

/// Format YAML content with prettier-compatible output.
///
/// # Errors
///
/// Returns an error if the content is not valid YAML.
pub fn format_yaml(content: &str, options: &PrettierOptions) -> Result<String> {
    let events = collect_events(content)?;
    let comments = extract_comments(content);
    let mut builder = AstBuilder::new(content, &events, &comments);
    let stream = builder.build_stream()?;
    let output = format_stream(&stream, options);
    Ok(output)
}

// ─── Event collection ──────────────────────────────────────────────────────────

fn collect_events(content: &str) -> Result<Vec<(Event<'_>, Span)>> {
    let parser = Parser::new_from_str(content);
    let mut events = Vec::new();
    for result in parser {
        let (event, span) = result.context("YAML parse error")?;
        events.push((event, span));
    }
    Ok(events)
}

// ─── Comment extraction ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SourceComment {
    line: usize,      // 1-indexed line number
    col: usize,       // 0-indexed column of the `#`
    text: String,     // including the `#`
    whole_line: bool, // true if the comment is the only content on this line
}

/// Extract all comments from source text.
///
/// This is a simple heuristic: find `#` characters that are not inside quoted strings.
/// We track whether we're inside a single-quoted or double-quoted string.
fn extract_comments(content: &str) -> Vec<SourceComment> {
    let mut comments = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(comment) = find_comment_in_line(line) {
            let whole_line = line[..comment.0].trim().is_empty();
            comments.push(SourceComment {
                line: line_idx + 1,
                col: comment.0,
                text: comment.1.to_string(),
                whole_line,
            });
        }
    }
    comments
}

/// Find a comment in a line, returning (column, text) if found.
/// Handles skipping `#` inside quoted strings.
fn find_comment_in_line(line: &str) -> Option<(usize, &str)> {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_char = '\0';
    let bytes = line.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        let ch = b as char;
        match ch {
            '\'' if !in_double && prev_char != '\\' => in_single = !in_single,
            '"' if !in_single && prev_char != '\\' => in_double = !in_double,
            '#' if !in_single && !in_double => {
                // A comment `#` must be preceded by a space or be at the start of the line
                if i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t' {
                    return Some((i, &line[i..]));
                }
            }
            _ => {}
        }
        prev_char = ch;
    }
    None
}

// ─── AST types ─────────────────────────────────────────────────────────────────

/// A comment with its source column, used to compute correct indentation.
#[derive(Debug, Clone)]
struct Comment {
    text: String,            // the comment text (starting with `#`)
    col: usize,              // 0-indexed source column of `#`
    line: usize,             // 1-indexed source line number
    blank_line_before: bool, // whether there's a blank line before this comment in the source
}

struct YamlStream {
    documents: Vec<YamlDoc>,
    trailing_comments: Vec<Comment>,
}

struct YamlDoc {
    explicit_start: bool,
    explicit_end: bool,
    /// Preamble lines before `---`: directives (%YAML, %TAG) and comments, in source order.
    preamble: Vec<String>,
    root: Option<Node>,
    end_comments: Vec<Comment>, // comments after root, before next doc or end
}

enum Node {
    Scalar(ScalarNode),
    Mapping(MappingNode),
    Sequence(SequenceNode),
    Alias(AliasNode),
}

#[allow(dead_code)]
struct ScalarNode {
    value: String,
    style: ScalarStyle,
    anchor: Option<String>,
    tag: Option<String>,
    /// For block scalars: the raw source text (indicator + body)
    block_source: Option<String>,
    /// For quoted scalars: the raw source text between delimiters (preserves original escapes)
    quoted_source: Option<String>,
    /// Whether this is an implicit null (empty value after `key:`)
    is_implicit_null: bool,
    span: Span,
    /// For plain scalars that span multiple source lines: the trimmed content per source line.
    /// Used by preserve mode to reconstruct original line structure.
    plain_source_lines: Option<Vec<String>>,
    /// Comments between tag/anchor and the scalar content.
    middle_comments: Vec<Comment>,
}

#[allow(dead_code)]
struct MappingNode {
    entries: Vec<MappingEntry>,
    flow: bool,
    anchor: Option<String>,
    tag: Option<String>,
    /// For flow mappings, store the raw source if we want to preserve it
    flow_source: Option<String>,
    middle_comments: Vec<Comment>, // comments between tag/anchor and first entry
    trailing_comments: Vec<Comment>, // comments after last entry before MappingEnd
}

struct MappingEntry {
    key: Node,
    value: Node,
    leading_comments: Vec<Comment>,
    key_trailing_comment: Option<String>, // trailing comment on key line (e.g. "hr: # comment")
    between_comments: Vec<Comment>,       // standalone comments between key and value
    trailing_comment: Option<String>,
    blank_line_before: bool,
    is_explicit_key: bool,
}

#[allow(dead_code)]
struct SequenceNode {
    items: Vec<SequenceItem>,
    flow: bool,
    anchor: Option<String>,
    tag: Option<String>,
    flow_source: Option<String>,
    middle_comments: Vec<Comment>,
    trailing_comments: Vec<Comment>, // comments after last item before SequenceEnd
}

struct SequenceItem {
    value: Node,
    leading_comments: Vec<Comment>,
    trailing_comment: Option<String>,
    blank_line_before: bool,
}

struct AliasNode {
    name: String,
}

// ─── AST builder ───────────────────────────────────────────────────────────────

struct AstBuilder<'a> {
    source: &'a str,
    source_lines: Vec<&'a str>,
    events: &'a [(Event<'a>, Span)],
    comments: &'a [SourceComment],
    pos: usize,
    /// Track lines that have been "consumed" by comment attachment
    used_comment_lines: Vec<bool>,
    /// Mapping from char index to byte index for safe slicing
    char_to_byte: Vec<usize>,
    /// Whether we are currently inside a flow collection (nested depth counter)
    in_flow_context: usize,
}

impl<'a> AstBuilder<'a> {
    fn new(
        source: &'a str,
        events: &'a [(Event<'a>, Span)],
        comments: &'a [SourceComment],
    ) -> Self {
        let source_lines: Vec<&str> = source.lines().collect();
        let used = vec![false; comments.len()];
        // Build char-to-byte index mapping
        let mut char_to_byte: Vec<usize> = source.char_indices().map(|(b, _)| b).collect();
        char_to_byte.push(source.len()); // sentinel for end
        AstBuilder {
            source,
            source_lines,
            events,
            comments,
            pos: 0,
            used_comment_lines: used,
            char_to_byte,
            in_flow_context: 0,
        }
    }

    /// Convert a char index from saphyr Marker to a byte index for string slicing.
    fn to_byte(&self, char_idx: usize) -> usize {
        if char_idx >= self.char_to_byte.len() {
            self.source.len()
        } else {
            self.char_to_byte[char_idx]
        }
    }

    fn peek(&self) -> Option<&(Event<'a>, Span)> {
        self.events.get(self.pos)
    }

    fn advance(&mut self) -> &(Event<'a>, Span) {
        let item = &self.events[self.pos];
        self.pos += 1;
        item
    }

    fn build_stream(&mut self) -> Result<YamlStream> {
        // Skip StreamStart
        self.advance(); // StreamStart

        let mut documents = Vec::new();
        while let Some((event, _)) = self.peek() {
            match event {
                Event::StreamEnd => {
                    self.advance();
                    break;
                }
                Event::DocumentStart(_) => {
                    documents.push(self.build_document()?);
                }
                _ => {
                    // Unexpected event, try to parse as implicit document
                    break;
                }
            }
        }

        // Collect any remaining unused whole-line comments as trailing
        let last_end = self.last_event_end_line();
        let trailing_comments = self.collect_remaining_comments(last_end);

        Ok(YamlStream {
            documents,
            trailing_comments,
        })
    }

    fn build_document(&mut self) -> Result<YamlDoc> {
        let (event, doc_span) = self.advance();
        let explicit_start = matches!(event, Event::DocumentStart(true));
        let doc_start_line = doc_span.start.line();

        // Collect preamble (directives + comments before ---) in source order
        let preamble = self.collect_preamble_before_line(doc_start_line);

        let root = self.build_node()?;

        // Save end line of content before consuming DocumentEnd
        let content_end_line = self.last_event_end_line();

        let explicit_end = if let Some((Event::DocumentEnd, span)) = self.peek() {
            let span = *span;
            self.advance();
            // Check source to see if "..." actually appears
            self.check_explicit_doc_end(&span)
        } else {
            false
        };

        // Collect end comments (between last node and document end marker).
        // Only do this when the document has an explicit `...` end marker,
        // otherwise the comments belong to the next document's preamble.
        let end_comments = if explicit_end {
            let doc_end_line = self.last_event_end_line();
            self.collect_comments_between_lines(content_end_line, doc_end_line)
        } else {
            Vec::new()
        };

        Ok(YamlDoc {
            explicit_start,
            explicit_end,
            preamble,
            root,
            end_comments,
        })
    }

    fn build_node(&mut self) -> Result<Option<Node>> {
        let Some((event, _span)) = self.peek() else {
            return Ok(None);
        };

        match event {
            Event::Scalar(_, _, _, _) => {
                let node = self.build_scalar()?;
                Ok(Some(node))
            }
            Event::SequenceStart(_, _) => {
                let node = self.build_sequence()?;
                Ok(Some(node))
            }
            Event::MappingStart(_, _) => {
                let node = self.build_mapping()?;
                Ok(Some(node))
            }
            Event::Alias(_) => {
                let node = self.build_alias()?;
                Ok(Some(node))
            }
            _ => Ok(None),
        }
    }

    #[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
    fn build_scalar(&mut self) -> Result<Node> {
        let (event, span) = self.advance();
        let span = *span;

        // Clone everything from the event to release the borrow on self
        let (value, style, anchor_id, has_tag) = if let Event::Scalar(v, s, a, t) = event {
            (v.to_string(), *s, *a, t.is_some())
        } else {
            unreachable!()
        };

        let anchor = if anchor_id > 0 {
            self.extract_anchor_before(&span)
        } else {
            None
        };

        let tag_str = if has_tag {
            // Prefer raw source tag to preserve original shorthand form
            self.extract_tag_before(&span)
        } else {
            None
        };

        let is_implicit_null = self.is_implicit_null(&span, &value, style);

        let block_source = if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded) {
            Some(self.extract_block_scalar_source(&span, style))
        } else {
            None
        };

        // For quoted scalars, extract the raw source between delimiters to preserve
        // original escape sequences (e.g. \u263A, \x0d, \/ stay as-is)
        let quoted_source =
            if matches!(style, ScalarStyle::DoubleQuoted | ScalarStyle::SingleQuoted) {
                self.extract_quoted_source(&span, style)
            } else {
                None
            };

        // For plain scalars spanning multiple lines, capture source line structure
        let plain_source_lines =
            if matches!(style, ScalarStyle::Plain) && span.end.line() > span.start.line() {
                let start_line = span.start.line(); // 1-indexed
                let end_line = span.end.line(); // 1-indexed
                let start_col = span.start.col(); // 0-indexed (char-based)
                let mut lines = Vec::new();
                for line_num in start_line..=end_line {
                    let idx = line_num.saturating_sub(1);
                    if idx < self.source_lines.len() {
                        let raw = self.source_lines[idx];
                        if line_num == start_line {
                            // First line: skip to the scalar start column
                            let content: String = raw.chars().skip(start_col).collect();
                            let trimmed = content.trim();
                            if trimmed.is_empty() {
                                lines.push(String::new());
                            } else {
                                lines.push(trimmed.to_string());
                            }
                        } else {
                            let trimmed = raw.trim();
                            if trimmed.is_empty() {
                                lines.push(String::new()); // blank line (paragraph break)
                            } else {
                                lines.push(trimmed.to_string());
                            }
                        }
                    }
                }
                Some(lines)
            } else {
                None
            };

        // Capture middle comments (between tag/anchor and scalar content)
        let middle_comments = if (tag_str.is_some() || anchor.is_some()) && !is_implicit_null {
            let content_line = span.start.line();
            // Find the actual tag/anchor line by looking at the source position
            let start_byte = self.to_byte(span.start.index());
            let mut props_line = content_line;
            // Search backwards to find the tag or anchor in the source
            if start_byte > 0 {
                let search_start = start_byte.saturating_sub(300);
                let region = &self.source[search_start..start_byte];
                // Find the last tag or anchor marker in the region
                for (i, b) in region.bytes().enumerate().rev() {
                    if b == b'!' || b == b'&' {
                        // Count newlines from search_start to this position to get the line
                        let pos = search_start + i;
                        let line = self.source[..pos].matches('\n').count() + 1;
                        props_line = line;
                        break;
                    }
                    // Don't search too far back (stop at blank lines)
                    if b == b'\n' && i > 0 && region.as_bytes()[i - 1] == b'\n' {
                        break;
                    }
                }
            }
            // The scalar content starts after the tag/anchor line
            if content_line > props_line {
                let mut comments = vec![];
                // First, capture any trailing comment on the props line itself
                if let Some(tc) = self.find_trailing_comment(props_line) {
                    comments.push(tc);
                }
                // Then collect standalone comments between props and content
                comments.extend(self.collect_comments_between_lines(props_line, content_line));
                comments
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(Node::Scalar(ScalarNode {
            value,
            style,
            anchor,
            tag: tag_str,
            block_source,
            quoted_source,
            is_implicit_null,
            span,
            plain_source_lines,
            middle_comments,
        }))
    }

    #[allow(clippy::too_many_lines)]
    fn build_sequence(&mut self) -> Result<Node> {
        let (event, span) = self.advance();
        let span = *span;

        let (anchor_id, has_tag) = if let Event::SequenceStart(a, t) = event {
            (*a, t.is_some())
        } else {
            unreachable!()
        };

        let anchor = if anchor_id > 0 {
            self.extract_anchor_before(&span)
        } else {
            None
        };
        let tag_str = if has_tag {
            self.extract_tag_before(&span)
        } else {
            None
        };

        let flow = self.is_flow_sequence_at(&span) || self.in_flow_context > 0;

        // For flow sequences, extract the full source text
        let flow_source = if flow && self.is_flow_sequence_at(&span) {
            self.extract_flow_source(&span)
        } else {
            None
        };

        if flow {
            self.in_flow_context += 1;
        }

        let mut items = Vec::new();
        let mut prev_end_line = span.start.line();

        // Collect middle comments (comments between tag/anchor and first entry)
        let mut middle_comments = vec![];
        if anchor.is_some() || tag_str.is_some() {
            let content_line = span.start.line();
            // Scan backwards from content_line to find the tag/anchor line
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                if src.starts_with('#') {
                    continue; // comment line, keep scanning
                }
                if src.starts_with('!') || src.starts_with('&') {
                    props_line = l;
                    break;
                }
                break; // not a comment or tag/anchor line
            }

            // First check for a trailing comment on the props line
            if let Some(comment) = self.find_trailing_comment(props_line) {
                middle_comments.push(comment);
            }
            // Then collect any standalone comment lines between props_line and content
            if props_line < content_line {
                let standalone = self.collect_comments_between_lines(props_line, content_line);
                middle_comments.extend(standalone);
            }
            // If no comments found yet, check the content line itself
            if middle_comments.is_empty()
                && let Some(comment) = self.find_trailing_comment(content_line)
            {
                middle_comments.push(comment);
            }
        }

        loop {
            let Some((event, _)) = self.peek() else {
                break;
            };
            if matches!(event, Event::SequenceEnd) {
                break;
            }

            let item_start_line = self.peek().map_or(0, |(_, s)| s.start.line());
            let leading_comments =
                self.collect_comments_between_lines(prev_end_line, item_start_line);
            // If there are leading comments, blank_line_before means "blank line between
            // the last leading comment and the item key" (not between prev entry and first comment).
            // The blank lines before/between comments are tracked on each Comment.
            let blank_line_before = if items.is_empty() {
                false
            } else if let Some(last_comment) = leading_comments.last() {
                self.has_blank_line_between(last_comment.line, item_start_line)
            } else {
                self.has_blank_line_immediately_before(item_start_line)
            };

            let value = self.build_node()?.unwrap_or(Node::Scalar(ScalarNode {
                value: String::new(),
                style: ScalarStyle::Plain,
                anchor: None,
                tag: None,
                block_source: None,
                quoted_source: None,
                is_implicit_null: true,
                span: Span::empty(span.start),
                plain_source_lines: None,
                middle_comments: vec![],
            }));

            // Use content end line for trailing comment to avoid picking up
            // comments from the next entry's line (MappingEnd/SequenceEnd events
            // can have end lines past the actual content)
            let content_end = self.last_content_end_line();
            let trailing_comment = self.find_trailing_comment(content_end).map(|c| c.text);

            items.push(SequenceItem {
                value,
                leading_comments,
                trailing_comment,
                blank_line_before,
            });

            // Use content_end (not event end) so comments between the child's
            // actual content and its End event are available for the next item's
            // leading_comments (especially when a child mapping filters out
            // comments at a shallower indent).
            prev_end_line = content_end;
        }

        // Collect trailing comments (between last item and SequenceEnd)
        let seq_end_line = self.peek().map_or(prev_end_line, |(_, s)| s.start.line());
        let trailing_comments =
            self.collect_comments_between_lines(prev_end_line, seq_end_line + 1);

        if let Some((Event::SequenceEnd, _)) = self.peek() {
            self.advance();
        }

        if flow {
            self.in_flow_context -= 1;
        }

        Ok(Node::Sequence(SequenceNode {
            items,
            flow,
            anchor,
            tag: tag_str,
            flow_source,
            middle_comments,
            trailing_comments,
        }))
    }

    #[allow(clippy::too_many_lines)]
    fn build_mapping(&mut self) -> Result<Node> {
        let (event, span) = self.advance();
        let span = *span;

        let (anchor_id, has_tag) = if let Event::MappingStart(a, t) = event {
            (*a, t.is_some())
        } else {
            unreachable!()
        };

        let anchor = if anchor_id > 0 {
            self.extract_anchor_before(&span)
        } else {
            None
        };
        let tag_str = if has_tag {
            self.extract_tag_before(&span)
        } else {
            None
        };

        let flow = self.is_flow_mapping_at(&span) || self.in_flow_context > 0;

        let flow_source = if flow && self.is_flow_mapping_at(&span) {
            self.extract_flow_source(&span)
        } else {
            None
        };

        if flow {
            self.in_flow_context += 1;
        }

        let mut entries = Vec::new();
        let mut prev_end_line = span.start.line();
        // Track the column of the first key for depth-filtered trailing comment collection
        let mut first_key_col: Option<usize> = None;

        // Collect middle comments (comments between tag/anchor and first entry)
        let mut middle_comments = vec![];
        if anchor.is_some() || tag_str.is_some() {
            let content_line = span.start.line();
            // Scan backwards from content_line to find the tag/anchor line
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                if src.starts_with('#') {
                    continue; // comment line, keep scanning
                }
                if src.starts_with('!') || src.starts_with('&') {
                    props_line = l;
                    break;
                }
                break; // not a comment or tag/anchor line
            }

            // First check for a trailing comment on the props line
            if let Some(comment) = self.find_trailing_comment(props_line) {
                middle_comments.push(comment);
            }
            // Then collect any standalone comment lines between props_line and content
            if props_line < content_line {
                let standalone = self.collect_comments_between_lines(props_line, content_line);
                middle_comments.extend(standalone);
            }
            // If no comments found yet, check the content line itself
            if middle_comments.is_empty()
                && let Some(comment) = self.find_trailing_comment(content_line)
            {
                middle_comments.push(comment);
            }
        }

        loop {
            let Some((event, _)) = self.peek() else {
                break;
            };
            if matches!(event, Event::MappingEnd) {
                break;
            }

            let key_start_line = self.peek().map_or(0, |(_, s)| s.start.line());
            let key_start_col = self.peek().map_or(0, |(_, s)| s.start.col());
            if first_key_col.is_none() {
                first_key_col = Some(key_start_col);
            }
            let leading_comments =
                self.collect_comments_between_lines(prev_end_line, key_start_line);
            // If there are leading comments, blank_line_before means "blank line between
            // the last leading comment and the entry key" (not between prev entry and first comment).
            // The blank lines before/between comments are tracked on each Comment.
            let blank_line_before = if entries.is_empty() {
                false
            } else if let Some(last_comment) = leading_comments.last() {
                self.has_blank_line_between(last_comment.line, key_start_line)
            } else {
                self.has_blank_line_immediately_before(key_start_line)
            };

            // Check for explicit key (?) in source
            let is_explicit_key = self.check_explicit_key(key_start_line, key_start_col);

            let key = self.build_node()?.unwrap_or(Node::Scalar(ScalarNode {
                value: String::new(),
                style: ScalarStyle::Plain,
                anchor: None,
                tag: None,
                block_source: None,
                quoted_source: None,
                is_implicit_null: true,
                span: Span::empty(span.start),
                plain_source_lines: None,
                middle_comments: vec![],
            }));

            // Collect comments between key and value
            let key_end_line = self.last_event_end_line();
            let value_start_line = self.peek().map_or(key_end_line, |(_, s)| s.start.line());
            // Check for trailing comment on key line (e.g. "hr: # comment\n  - items")
            let key_trailing_comment = if value_start_line > key_end_line {
                self.find_trailing_comment(key_end_line).map(|c| c.text)
            } else {
                None
            };
            let between_comments =
                self.collect_comments_between_lines(key_end_line, value_start_line);

            let mut value = self.build_node()?.unwrap_or(Node::Scalar(ScalarNode {
                value: String::new(),
                style: ScalarStyle::Plain,
                anchor: None,
                tag: None,
                block_source: None,
                quoted_source: None,
                is_implicit_null: true,
                span: Span::empty(span.start),
                plain_source_lines: None,
                middle_comments: vec![],
            }));

            // If the key has a trailing comment and the value has props (anchor/tag),
            // inject the comment into the value's middle_comments so it appears
            // after the props on the same line (e.g. "key: &anchor # comment")
            let key_trailing_comment = if key_trailing_comment.is_some() && has_node_props(&value) {
                let comment_obj = Comment {
                    text: key_trailing_comment.clone().unwrap_or_default(),
                    col: 0,
                    line: 0,
                    blank_line_before: false,
                };
                match &mut value {
                    Node::Mapping(m) => {
                        m.middle_comments.insert(0, comment_obj.clone());
                    }
                    Node::Sequence(s) => {
                        s.middle_comments.insert(0, comment_obj);
                    }
                    _ => {}
                }
                None // consumed - don't store on the entry
            } else {
                key_trailing_comment
            };

            // Use content end line for trailing comment to avoid picking up
            // comments from the next entry's line (MappingEnd/SequenceEnd events
            // can have end lines past the actual content)
            let content_end = self.last_content_end_line();
            let trailing_comment = self.find_trailing_comment(content_end).map(|c| c.text);

            entries.push(MappingEntry {
                key,
                value,
                leading_comments,
                key_trailing_comment,
                between_comments,
                trailing_comment,
                blank_line_before,
                is_explicit_key,
            });

            // Use content_end (not event end) so comments between the child's
            // actual content and its End event are available for the next entry's
            // leading_comments.
            prev_end_line = content_end;
        }

        // Collect trailing comments (between last entry and MappingEnd)
        // Use depth-filtered collection for block mappings so comments at a
        // shallower indent (belonging to parent scope) are left for the parent.
        let mapping_end_line = self.peek().map_or(prev_end_line, |(_, s)| s.start.line());
        let trailing_comments = if flow {
            self.collect_comments_between_lines(prev_end_line, mapping_end_line + 1)
        } else if let Some(min_col) = first_key_col {
            self.collect_comments_between_lines_at_depth(
                prev_end_line,
                mapping_end_line + 1,
                min_col,
            )
        } else {
            self.collect_comments_between_lines(prev_end_line, mapping_end_line + 1)
        };

        if let Some((Event::MappingEnd, _)) = self.peek() {
            self.advance();
        }

        if flow {
            self.in_flow_context -= 1;
        }

        Ok(Node::Mapping(MappingNode {
            entries,
            flow,
            anchor,
            tag: tag_str,
            flow_source,
            middle_comments,
            trailing_comments,
        }))
    }

    fn build_alias(&mut self) -> Result<Node> {
        let (event, span) = self.advance();
        let span = *span;

        if let Event::Alias(_id) = event {
            // Extract alias name from source
            let name = self.extract_alias_name(&span);
            Ok(Node::Alias(AliasNode { name }))
        } else {
            unreachable!()
        }
    }

    // ─── Source extraction helpers ──────────────────────────────────────────

    fn last_event_end_line(&self) -> usize {
        if self.pos > 0 {
            self.events[self.pos - 1].1.end.line()
        } else {
            1
        }
    }

    /// Get the end line of the last content event, avoiding the issue where
    /// MappingEnd/SequenceEnd events may have end lines past the actual content.
    /// Falls back to `last_event_end_line` for non-End events.
    fn last_content_end_line(&self) -> usize {
        if self.pos < 2 {
            return self.last_event_end_line();
        }
        let last = &self.events[self.pos - 1];
        match &last.0 {
            Event::MappingEnd | Event::SequenceEnd => {
                // Use the start line of the End event, or the end line of the
                // event before it (the last content event)
                let prev = &self.events[self.pos - 2];
                prev.1.end.line()
            }
            _ => last.1.end.line(),
        }
    }

    /// Check if a mapping or sequence at this span is a flow collection.
    /// For mappings, check for `{`; for sequences, check for `[`.
    fn is_flow_mapping_at(&self, span: &Span) -> bool {
        let byte_idx = self.to_byte(span.start.index());
        if byte_idx < self.source.len() {
            self.source.as_bytes()[byte_idx] == b'{'
        } else {
            false
        }
    }

    fn is_flow_sequence_at(&self, span: &Span) -> bool {
        let byte_idx = self.to_byte(span.start.index());
        if byte_idx < self.source.len() {
            self.source.as_bytes()[byte_idx] == b'['
        } else {
            false
        }
    }

    /// Extract the raw tag string from source before the span (e.g. "!!str", "!foo", "!e!foo").
    /// This preserves the original shorthand form before %TAG directive resolution.
    fn extract_tag_before(&self, span: &Span) -> Option<String> {
        let start = self.to_byte(span.start.index());
        let search_start = start.saturating_sub(300);
        let region = &self.source[search_start..start];

        // Find the last '!' in the region that starts a tag
        // Tags: !suffix, !!suffix, !handle!suffix, !<verbatim>
        // Search backwards for the start of the tag
        let bytes = region.as_bytes();
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            if bytes[i] == b'!' {
                // Check if this '!' is inside a comment line (after '#')
                // Scan backwards from i to the start of the line
                let mut on_comment_line = false;
                let mut j = i;
                while j > 0 {
                    j -= 1;
                    if bytes[j] == b'\n' {
                        break;
                    }
                    if bytes[j] == b'#' {
                        on_comment_line = true;
                        break;
                    }
                }
                if on_comment_line {
                    continue;
                }

                // Found a '!'. Determine the tag form.
                let tag_start = search_start + i;
                let rest = &self.source[tag_start..start];

                // Tags end at whitespace or flow indicators
                let tag_end_offset = rest
                    .find(|c: char| c.is_whitespace() || c == '{' || c == '[')
                    .unwrap_or(rest.len());
                let tag_text = rest[..tag_end_offset].trim_end();
                if !tag_text.is_empty() && tag_text.starts_with('!') {
                    // Check that the char before is whitespace, start of line, or context char
                    if i == 0
                        || bytes[i - 1].is_ascii_whitespace()
                        || bytes[i - 1] == b'-'
                        || bytes[i - 1] == b':'
                        || bytes[i - 1] == b','
                    {
                        return Some(tag_text.to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_anchor_before(&self, span: &Span) -> Option<String> {
        // Look backwards from span start to find &anchor_name
        let start = self.to_byte(span.start.index());
        let search_start = start.saturating_sub(200);
        let region = &self.source[search_start..start];

        // Find the last & in this region
        if let Some(amp_pos) = region.rfind('&') {
            let after_amp = &region[amp_pos + 1..];
            let name: String = after_amp
                .chars()
                .take_while(|c| is_anchor_char(*c))
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }

    fn extract_alias_name(&self, span: &Span) -> String {
        let start = self.to_byte(span.start.index());
        let end = self.to_byte(span.end.index());
        let region = &self.source[start..end];

        // Should start with *
        if let Some(rest) = region.strip_prefix('*') {
            rest.chars().take_while(|c| is_anchor_char(*c)).collect()
        } else {
            // Fallback: search for * in the region
            if let Some(star_pos) = region.find('*') {
                region[star_pos + 1..]
                    .chars()
                    .take_while(|c| is_anchor_char(*c))
                    .collect()
            } else {
                String::from("unknown")
            }
        }
    }

    fn extract_flow_source(&self, span: &Span) -> Option<String> {
        // Find the matching close bracket/brace
        let start = self.to_byte(span.start.index());
        let open_char = self.source.as_bytes().get(start).copied()? as char;
        let close_char = match open_char {
            '{' => '}',
            '[' => ']',
            _ => return None,
        };

        let mut depth = 0;
        let mut in_single = false;
        let mut in_double = false;
        let mut end = start;

        for (i, ch) in self.source[start..].char_indices() {
            match ch {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                c if c == open_char && !in_single && !in_double => depth += 1,
                c if c == close_char && !in_single && !in_double => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        Some(self.source[start..end].to_string())
    }

    fn extract_block_scalar_source(&self, span: &Span, style: ScalarStyle) -> String {
        // The span covers the scalar value. We need to find the indicator line.
        let start_line = span.start.line(); // 1-indexed
        let indicator_char = match style {
            ScalarStyle::Literal => '|',
            ScalarStyle::Folded => '>',
            _ => return String::new(),
        };

        // The indicator line is on or before the span start line.
        // Search backwards from the span start line.
        let mut indicator_line_idx = None;
        let mut indicator_char_pos = 0;

        for i in 0..4 {
            let check_idx = start_line.saturating_sub(1).saturating_sub(i);
            if check_idx < self.source_lines.len() {
                let line = self.source_lines[check_idx];
                // Find the block scalar indicator char on this line.
                // We search from the right, but must verify the char is actually an
                // indicator (not content that happens to contain | or >).
                // The indicator char must be preceded by whitespace, ':', '-', or be
                // the first non-space character on the line.
                let bytes = line.as_bytes();
                let mut found = false;
                let mut search_from = bytes.len();
                while search_from > 0 {
                    // Find the last occurrence of indicator_char before search_from
                    let region = &line[..search_from];
                    let Some(pos) = region.rfind(indicator_char) else {
                        break;
                    };
                    search_from = pos; // next iteration searches before this pos

                    // Check what follows: valid block scalar header chars
                    let after = line[pos + 1..].trim();
                    let valid_after = after.is_empty()
                        || after.starts_with('+')
                        || after.starts_with('-')
                        || after.starts_with(|c: char| c.is_ascii_digit())
                        || after.starts_with('#');
                    if !valid_after {
                        continue;
                    }

                    // Check what precedes: must be whitespace, ':', '-', or start of line
                    let valid_before = if pos == 0 {
                        true
                    } else {
                        let prev = bytes[pos - 1];
                        prev == b' ' || prev == b'\t' || prev == b':' || prev == b'-'
                    };
                    // Also accept if this is the first non-space char on the line
                    let first_non_space = line.find(|c: char| !c.is_whitespace());
                    let is_first_content = first_non_space == Some(pos);

                    if valid_before || is_first_content {
                        indicator_line_idx = Some(check_idx);
                        indicator_char_pos = pos;
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }

        let Some(indicator_line_idx) = indicator_line_idx else {
            return String::new();
        };

        // Use the indicator position we already found during search
        let indicator_line = self.source_lines[indicator_line_idx];
        let indicator_pos = indicator_char_pos;

        // Extract the indicator header (e.g., |, |+, |-, |2, >-, etc.)
        // Preserve any trailing comment (e.g., "> # hello" or "| # comment")
        let header_region = &indicator_line[indicator_pos..];
        let header = header_region.trim_end();

        // Determine the body extent by scanning forward from the indicator.
        // Body lines must be indented more than the indicator line, or be empty.
        // This is more reliable than using span.end which can point into the next token.
        let body_start_line = indicator_line_idx + 1;
        let indicator_line_indent = indicator_line.len() - indicator_line.trim_start().len();
        let has_keep = header.contains('+');

        // Track the last content (non-empty, properly-indented) line and the last
        // candidate line (including trailing empties). For keep chomping, include
        // trailing empties. For clip/strip, stop at the last content line so that
        // trailing blank lines remain available for blank_line_before detection.
        let mut last_content_line: Option<usize> = None;
        let mut last_candidate_line: Option<usize> = None;

        for i in body_start_line..self.source_lines.len() {
            let line = self.source_lines[i];
            if line.trim().is_empty() {
                last_candidate_line = Some(i);
            } else {
                let line_indent = line.len() - line.trim_start().len();
                if line_indent > indicator_line_indent {
                    last_content_line = Some(i);
                    last_candidate_line = Some(i);
                } else {
                    break;
                }
            }
        }

        let body_end_line = if has_keep {
            last_candidate_line
        } else {
            last_content_line
        };

        let mut body_lines = Vec::new();
        if let Some(end) = body_end_line {
            for i in body_start_line..=end.min(self.source_lines.len().saturating_sub(1)) {
                body_lines.push(self.source_lines[i]);
            }
        }

        format!("{}\n{}", header, body_lines.join("\n"))
    }

    /// Extract the raw source text of a quoted scalar (content between delimiters).
    /// This preserves original escape sequences like \u263A, \x0d, \/.
    fn extract_quoted_source(&self, span: &Span, style: ScalarStyle) -> Option<String> {
        let quote_char: u8 = match style {
            ScalarStyle::DoubleQuoted => b'"',
            ScalarStyle::SingleQuoted => b'\'',
            _ => return None,
        };

        // Use span start byte to find the opening quote
        let start_byte = self.to_byte(span.start.index());

        // Search backwards from span start for the opening quote
        let mut open_pos = None;
        if start_byte == 0 {
            // Span starts at byte 0; check if byte 0 is the opening quote
            if !self.source.is_empty() && self.source.as_bytes()[0] == quote_char {
                open_pos = Some(0);
            }
        } else {
            for i in (0..start_byte).rev() {
                if self.source.as_bytes()[i] == quote_char {
                    open_pos = Some(i);
                    break;
                }
                // Don't search too far back (max 5 chars for whitespace/newline before content)
                if start_byte - i > 5 {
                    break;
                }
            }
            // Also check at the span start itself (for empty strings like "" or '')
            if open_pos.is_none()
                && start_byte < self.source.len()
                && self.source.as_bytes()[start_byte] == quote_char
            {
                open_pos = Some(start_byte);
            }
        }

        let open_pos = open_pos?;

        // Search forward from after the opening quote for the closing quote.
        // For double-quoted: skip \X escape sequences.
        // For single-quoted: '' is an escape, lone ' is the end.
        let content_start = open_pos + 1;
        let bytes = self.source.as_bytes();
        let mut i = content_start;
        while i < bytes.len() {
            if style == ScalarStyle::DoubleQuoted {
                if bytes[i] == b'\\' {
                    i += 2; // skip escape sequence
                } else if bytes[i] == b'"' {
                    // Found closing quote
                    return Some(self.source[content_start..i].to_string());
                } else {
                    i += 1;
                }
            } else {
                // Single-quoted
                if bytes[i] == b'\'' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 2; // escaped ''
                    } else {
                        // Closing quote
                        return Some(self.source[content_start..i].to_string());
                    }
                } else {
                    i += 1;
                }
            }
        }

        None
    }

    fn is_implicit_null(&self, span: &Span, value: &str, style: ScalarStyle) -> bool {
        if style != ScalarStyle::Plain {
            return false;
        }
        // Check if the scalar is "~" or "" and the source at span position doesn't contain "~"
        if value == "~" || value.is_empty() {
            // If span is zero-length, it's definitely implicit
            if span.is_empty() {
                return true;
            }
            // Check the source - if it's just whitespace/newline at this position
            let start = self.to_byte(span.start.index());
            let end = self.to_byte(span.end.index());
            if start >= self.source.len() {
                return true;
            }
            let text = self.source[start..end].trim();
            // "~" in source means explicit null; empty or non-tilde means implicit
            // For flow collections, saphyr may set span to ", " or "}" etc.
            !text.contains('~')
        } else {
            false
        }
    }

    fn check_explicit_doc_end(&self, span: &Span) -> bool {
        // Check if "..." appears at the span position in source
        let line = span.start.line();
        if line == 0 || line > self.source_lines.len() {
            return false;
        }
        let content = self.source_lines[line - 1].trim();
        content == "..."
    }

    fn check_explicit_key(&self, line: usize, col: usize) -> bool {
        // line is 1-indexed, col is 0-indexed
        if line == 0 || line > self.source_lines.len() {
            return false;
        }
        let src_line = self.source_lines[line - 1];
        // First check: line starts with `? ` (block context)
        let content = src_line.trim_start();
        if content.starts_with("? ") || content == "?" {
            return true;
        }
        // Second check: `? ` appears before the key column (flow context)
        // Look backwards from col for `?` preceded by whitespace or flow indicator
        if col >= 2 {
            let before = &src_line[..col];
            let trimmed = before.trim_end();
            if trimmed.ends_with('?') {
                return true;
            }
        }
        false
    }

    /// Check if a line (1-indexed) is preceded by a blank line in source.
    /// Skips over comment-only lines when scanning backwards.
    /// Check if the line immediately before the given 1-indexed line is blank.
    /// Does NOT skip over comment lines — only checks the single previous line.
    fn has_blank_line_immediately_before(&self, line: usize) -> bool {
        if line < 2 {
            return false;
        }
        let idx = line - 2; // 0-indexed previous line
        if idx >= self.source_lines.len() {
            return false;
        }
        self.source_lines[idx].trim().is_empty()
    }

    #[allow(dead_code)]
    fn line_preceded_by_blank(&self, line: usize) -> bool {
        let mut check = line.saturating_sub(1); // 1-indexed, the line before
        while check >= 1 {
            let idx = check - 1;
            if idx >= self.source_lines.len() {
                break;
            }
            let src = self.source_lines[idx];
            if src.trim().is_empty() {
                return true;
            }
            if src.trim_start().starts_with('#') {
                // Comment line - keep looking backwards
                check -= 1;
                continue;
            }
            break;
        }
        false
    }

    /// Check if there's a blank line in the source between two 1-indexed line numbers.
    /// Skips comment lines when looking for blank lines.
    fn has_blank_line_between(&self, start_line: usize, end_line: usize) -> bool {
        if end_line <= start_line + 1 {
            return false;
        }
        for line in (start_line + 1)..end_line {
            let idx = line - 1;
            if idx >= self.source_lines.len() {
                break;
            }
            let src = self.source_lines[idx];
            if src.trim().is_empty() {
                return true;
            }
        }
        false
    }

    fn collect_comments_between_lines(
        &mut self,
        start_line: usize,
        end_line: usize,
    ) -> Vec<Comment> {
        let mut result = Vec::new();
        let mut prev_item_line = start_line;
        for (i, comment) in self.comments.iter().enumerate() {
            if !self.used_comment_lines[i]
                && comment.whole_line
                && comment.line > start_line
                && comment.line < end_line
            {
                let blank_before = self.has_blank_line_between(prev_item_line, comment.line);
                result.push(Comment {
                    text: comment.text.clone(),
                    col: comment.col,
                    line: comment.line,
                    blank_line_before: blank_before,
                });
                self.used_comment_lines[i] = true;
                prev_item_line = comment.line;
            }
        }
        result
    }

    /// Like `collect_comments_between_lines` but only collects comments at col >= `min_col`.
    /// Comments at shallower indentation are left for the parent scope.
    /// Once a comment at col < `min_col` is found, all subsequent comments are also skipped
    /// (they belong to the parent scope).
    fn collect_comments_between_lines_at_depth(
        &mut self,
        start_line: usize,
        end_line: usize,
        min_col: usize,
    ) -> Vec<Comment> {
        let mut result = Vec::new();
        let mut prev_item_line = start_line;
        let mut broke_out = false;
        for (i, comment) in self.comments.iter().enumerate() {
            if !self.used_comment_lines[i]
                && comment.whole_line
                && comment.line > start_line
                && comment.line < end_line
            {
                if broke_out || comment.col < min_col {
                    // Once we see a comment at a shallower indent, all subsequent
                    // comments (even deeper ones) belong to the parent scope.
                    broke_out = true;
                    continue;
                }
                let blank_before = self.has_blank_line_between(prev_item_line, comment.line);
                result.push(Comment {
                    text: comment.text.clone(),
                    col: comment.col,
                    line: comment.line,
                    blank_line_before: blank_before,
                });
                self.used_comment_lines[i] = true;
                prev_item_line = comment.line;
            }
        }
        result
    }

    /// Collect all remaining unused comments, tracking blank lines relative to `after_line`.
    fn collect_remaining_comments(&mut self, after_line: usize) -> Vec<Comment> {
        let mut result = Vec::new();
        let mut prev_item_line = after_line;
        for (i, comment) in self.comments.iter().enumerate() {
            if !self.used_comment_lines[i] && comment.whole_line {
                let blank_before =
                    prev_item_line > 0 && self.has_blank_line_between(prev_item_line, comment.line);
                result.push(Comment {
                    text: comment.text.clone(),
                    col: comment.col,
                    line: comment.line,
                    blank_line_before: blank_before,
                });
                self.used_comment_lines[i] = true;
                prev_item_line = comment.line;
            }
        }
        result
    }

    /// Collect preamble (directives and comments) before a given line, in source order.
    fn collect_preamble_before_line(&mut self, line: usize) -> Vec<String> {
        let mut result = Vec::new();
        for i in 0..self.source_lines.len() {
            let line_num = i + 1; // 1-indexed
            if line_num >= line {
                break;
            }
            let trimmed = self.source_lines[i].trim();
            if trimmed.starts_with('%') {
                result.push(trimmed.to_string());
            } else if trimmed.starts_with('#') {
                // Check if this comment is in our comments list and mark it used
                for (ci, comment) in self.comments.iter().enumerate() {
                    if !self.used_comment_lines[ci] && comment.line == line_num {
                        result.push(comment.text.clone());
                        self.used_comment_lines[ci] = true;
                        break;
                    }
                }
            }
        }
        result
    }

    fn find_trailing_comment(&mut self, line: usize) -> Option<Comment> {
        for (i, comment) in self.comments.iter().enumerate() {
            if !self.used_comment_lines[i] && !comment.whole_line && comment.line == line {
                self.used_comment_lines[i] = true;
                return Some(Comment {
                    text: comment.text.clone(),
                    col: comment.col,
                    line: comment.line,
                    blank_line_before: false,
                });
            }
        }
        None
    }
}

/// Check if a character is valid in a YAML anchor/alias name.
/// YAML spec: any character except flow indicators ([]{},) and whitespace.
fn is_anchor_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '[' | ']' | '{' | '}' | ',')
}

#[allow(dead_code, clippy::ptr_arg)]
fn format_tag(tag: &Cow<'_, Tag>) -> String {
    if tag.handle.is_empty() && tag.suffix == "!" {
        // Non-specific tag: just "!"
        "!".to_string()
    } else if tag.handle.is_empty() {
        // Verbatim tag: !<suffix> (saphyr strips the angle brackets)
        format!("!<{}>", tag.suffix)
    } else if tag.handle == "!" {
        format!("!{}", tag.suffix)
    } else if tag.handle == "!!" || tag.handle == "tag:yaml.org,2002:" {
        format!("!!{}", tag.suffix)
    } else {
        format!("{}!{}", tag.handle, tag.suffix)
    }
}

// ─── Formatting ────────────────────────────────────────────────────────────────

fn format_stream(stream: &YamlStream, options: &PrettierOptions) -> String {
    let mut output = String::new();

    for (i, doc) in stream.documents.iter().enumerate() {
        // Blank line between documents only when the next document has a preamble
        // (comments) and the prior document didn't end with `...` separator.
        let prev_had_end_marker = i > 0 && stream.documents[i - 1].explicit_end;
        if i > 0
            && !doc.preamble.is_empty()
            && !prev_had_end_marker
            && !output.is_empty()
            && !output.ends_with("\n\n")
        {
            output.push('\n');
        }

        // Write preamble (directives and comments before ---)
        for line in &doc.preamble {
            output.push_str(line);
            output.push('\n');
        }

        // Write document start marker
        if doc.explicit_start {
            output.push_str("---");
            output.push('\n');
        }

        // Write root node
        if let Some(root) = &doc.root {
            format_node(root, &mut output, 0, options, true, false);
        }

        // Ensure content ends with newline before next doc
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        // Write document end marker
        if doc.explicit_end {
            output.push_str("...\n");
        }

        // Write end comments
        for comment in &doc.end_comments {
            output.push_str(&comment.text);
            output.push('\n');
        }

        let _ = i; // suppress unused warning
    }

    // Write trailing stream comments
    for comment in &stream.trailing_comments {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        // Preserve blank line before stream-level comments
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, 0, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }

    // Ensure output ends with newline (don't strip extra newlines - may be from |+ block scalars)
    if !output.ends_with('\n') && !output.is_empty() {
        output.push('\n');
    }

    output
}

fn format_node(
    node: &Node,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    is_top: bool,
    inline: bool,
) {
    match node {
        Node::Scalar(s) => format_scalar(s, output, depth, options, 0),
        Node::Mapping(m) => {
            if m.flow {
                format_flow_mapping(m, output, depth, options);
            } else {
                format_block_mapping(m, output, depth, options, is_top, inline);
            }
        }
        Node::Sequence(s) => {
            if s.flow {
                format_flow_sequence(s, output, depth, options);
            } else {
                format_block_sequence(s, output, depth, options, is_top, inline);
            }
        }
        Node::Alias(a) => {
            output.push('*');
            output.push_str(&a.name);
        }
    }
}

fn format_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    first_line_prefix: usize,
) {
    // Write tag
    if let Some(tag) = &s.tag {
        output.push_str(tag);
        output.push(' ');
    }

    // Write anchor
    if let Some(anchor) = &s.anchor {
        output.push('&');
        output.push_str(anchor);
        if !s.is_implicit_null {
            output.push(' ');
        }
    }

    if s.is_implicit_null {
        // Implicit null: don't write anything
        return;
    }

    // Write middle comments (between tag/anchor and content)
    if !s.middle_comments.is_empty() {
        let indent = indent_str(depth, options);
        if s.middle_comments.len() == 1 {
            // Single comment: on same line as tag/anchor, then newline
            // Remove trailing space from tag/anchor
            if output.ends_with(' ') {
                output.pop();
            }
            output.push(' ');
            output.push_str(&s.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple comments: tag/anchor on own line, then each comment
            // Remove trailing space from tag/anchor
            if output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
            for comment in &s.middle_comments {
                output.push_str(&indent);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
        // Value starts on new line with indent
        output.push_str(&indent);
    }

    match s.style {
        ScalarStyle::Plain => {
            if s.value == "~" {
                // Tilde null - keep as-is
                output.push('~');
            } else {
                format_plain_scalar(s, output, depth, options, first_line_prefix);
            }
        }
        ScalarStyle::SingleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                depth,
                options,
                true,
            );
        }
        ScalarStyle::DoubleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                depth,
                options,
                false,
            );
        }
        ScalarStyle::Literal | ScalarStyle::Folded => {
            format_block_scalar(s, output, depth, options);
        }
    }
}

/// Format a plain scalar value with proseWrap awareness.
///
/// `first_line_prefix` is the number of characters already consumed on the current line
/// (e.g., `key: ` for a mapping value).
fn format_plain_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    first_line_prefix: usize,
) {
    use crate::options::ProseWrap;

    let indent = indent_str(depth, options);

    match options.prose_wrap {
        ProseWrap::Always => {
            format_plain_wrap(
                &s.value,
                output,
                &indent,
                options.print_width,
                first_line_prefix,
            );
        }
        ProseWrap::Never => {
            format_plain_never(&s.value, output, &indent);
        }
        ProseWrap::Preserve => {
            if let Some(ref source_lines) = s.plain_source_lines {
                format_plain_preserve(source_lines, output, &indent);
            } else {
                // Single-line plain scalar: output as-is
                output.push_str(&s.value);
            }
        }
    }
}

/// `ProseWrap::Always` — Re-wrap at `print_width`.
/// Paragraph breaks (\n in value) are preserved as blank lines.
fn format_plain_wrap(
    value: &str,
    output: &mut String,
    indent: &str,
    print_width: usize,
    first_line_prefix: usize,
) {
    // Each \n in the value represents a paragraph break (blank line).
    // Split by \n; consecutive \n\n produces empty parts for extra blank lines.
    let parts: Vec<&str> = value.split('\n').collect();
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            // End previous content line + paragraph break (blank line)
            output.push_str("\n\n");
            // Additional blank lines for consecutive \n\n
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        // Word-wrap this paragraph
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let mut line_len = if first_content {
            first_line_prefix
        } else {
            indent.len()
        };
        let mut first_word = true;

        for word in &words {
            if first_word {
                output.push_str(word);
                line_len += word.len();
                first_word = false;
            } else if line_len + 1 + word.len() > print_width {
                output.push('\n');
                output.push_str(indent);
                output.push_str(word);
                line_len = indent.len() + word.len();
            } else {
                output.push(' ');
                output.push_str(word);
                line_len += 1 + word.len();
            }
        }

        first_content = false;
    }
}

/// `ProseWrap::Never` — Join words in each paragraph on one line.
/// Paragraph breaks (\n) still produce blank lines.
fn format_plain_never(value: &str, output: &mut String, indent: &str) {
    let parts: Vec<&str> = value.split('\n').collect();
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            output.push_str("\n\n");
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        // Join all words on one line (no wrapping)
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        output.push_str(&words.join(" "));

        first_content = false;
    }
}

/// `ProseWrap::Preserve` — Use original source line structure.
/// Each source line becomes a separate output line with proper indentation.
/// Blank source lines become blank output lines.
fn format_plain_preserve(source_lines: &[String], output: &mut String, indent: &str) {
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for line in source_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            output.push('\n');
            // Blank lines for paragraph breaks
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        output.push_str(trimmed);
        first_content = false;
    }
}

/// Normalize block scalar header indicator order.
/// YAML spec says digit (indentation indicator) comes before chomping indicator.
/// e.g. `|-2` → `|2-`, `>-1` → `>1-`
fn normalize_block_header(header: &str) -> String {
    let mut chars = header.chars();
    let indicator = chars.next().unwrap_or('|'); // | or >
    let rest: String = chars.collect();

    let mut digit = None;
    let mut chomp = None;
    for c in rest.chars() {
        if c.is_ascii_digit() {
            digit = Some(c);
        } else if c == '+' || c == '-' {
            chomp = Some(c);
        }
    }

    let mut result = String::new();
    result.push(indicator);
    if let Some(d) = digit {
        result.push(d);
    }
    if let Some(c) = chomp {
        result.push(c);
    }
    result
}

/// Format a block scalar (literal `|` or folded `>`).
///
/// Re-indents the body from the raw source to use the correct indentation
/// based on depth and `tab_width`. Uses raw source to preserve trailing blank
/// lines (saphyr doesn't always include them in the parsed value).
#[allow(clippy::too_many_lines)]
fn format_block_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let Some(block_src) = &s.block_source else {
        // Fallback: reconstruct from value
        let indicator = if s.style == ScalarStyle::Literal {
            '|'
        } else {
            '>'
        };
        output.push(indicator);
        output.push('\n');
        let body_indent = " ".repeat(depth.max(1) * options.tab_width);
        for line in s.value.lines() {
            output.push_str(&body_indent);
            output.push_str(line);
            output.push('\n');
        }
        return;
    };

    // Use split('\n') instead of lines() to preserve trailing empty lines
    // (lines() strips the final newline as a terminator, losing trailing blank lines)
    let mut src_lines = block_src.split('\n');

    // First line is the header (|, |+, |-, >+, etc.), possibly with trailing comment
    let header_full = src_lines.next().unwrap_or("|");
    // Split indicator part from trailing comment (e.g. "> # hello" -> ">", "# hello")
    let (header, header_comment) = if let Some(comment_pos) = header_full.find(" #") {
        (
            &header_full[..comment_pos],
            Some(header_full[comment_pos..].trim()),
        )
    } else {
        (header_full, None)
    };
    // Normalize indicator order: digit before chomping (e.g. |-2 -> |2-)
    let normalized_header = normalize_block_header(header);
    output.push_str(&normalized_header);
    if let Some(comment) = header_comment {
        output.push(' ');
        output.push_str(comment);
    }
    output.push('\n');

    // Collect body lines from raw source
    let mut body_lines: Vec<&str> = src_lines.collect();

    // For keep chomping (|+, >+), preserve all trailing blank lines.
    // For clip/strip, remove trailing empty lines from the body (they come from
    // source text but should not appear in formatted output).
    let is_keep = header.contains('+');
    if !is_keep {
        while body_lines.last() == Some(&"") {
            body_lines.pop();
        }
    }

    // Check if header has an explicit indent indicator (a digit)
    // e.g. |2, |1+, >2-, etc. - the digit after the indicator char
    let has_explicit_indent = header
        .chars()
        .skip(1) // skip | or >
        .any(|c| c.is_ascii_digit());

    if has_explicit_indent {
        // With explicit indent, preserve body lines as-is from source
        for line in &body_lines {
            output.push_str(line);
            output.push('\n');
        }
    } else {
        // Re-indent: detect base indent from source and normalize to target
        let base_indent = body_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        let target_indent = " ".repeat(depth.max(1) * options.tab_width);

        // For folded (>) scalars with proseWrap:always/never, re-wrap content
        if s.style == ScalarStyle::Folded {
            use crate::options::ProseWrap;
            if matches!(options.prose_wrap, ProseWrap::Always | ProseWrap::Never) {
                let mut i = 0;
                while i < body_lines.len() {
                    let line = body_lines[i];
                    let trimmed = line.trim();

                    if trimmed.is_empty() {
                        // Blank line: preserve (paragraph break or keep-chomping trailing)
                        if !line.is_empty() {
                            let line_indent = line.len();
                            let extra = line_indent.saturating_sub(base_indent);
                            if extra > 0 {
                                output.push_str(&target_indent);
                                output.push_str(&" ".repeat(extra));
                            }
                        }
                        output.push('\n');
                        i += 1;
                    } else {
                        let line_indent = line.len() - trimmed.len();
                        let extra = line_indent.saturating_sub(base_indent);

                        if extra > 0 {
                            // More-indented line: preserve with re-indent
                            output.push_str(&target_indent);
                            output.push_str(&" ".repeat(extra));
                            output.push_str(trimmed);
                            output.push('\n');
                            i += 1;
                        } else {
                            // Regular content: collect consecutive regular lines, fold into paragraph
                            let mut words = Vec::new();
                            while i < body_lines.len() {
                                let l = body_lines[i];
                                let t = l.trim();
                                if t.is_empty() {
                                    break;
                                }
                                let li = l.len() - t.len();
                                let ex = li.saturating_sub(base_indent);
                                if ex > 0 {
                                    break;
                                }
                                words.extend(t.split_whitespace());
                                i += 1;
                            }

                            // Output the folded paragraph
                            if matches!(options.prose_wrap, ProseWrap::Always) {
                                let mut line_len = target_indent.len();
                                output.push_str(&target_indent);
                                let mut first_word = true;
                                for word in &words {
                                    if first_word {
                                        output.push_str(word);
                                        line_len += word.len();
                                        first_word = false;
                                    } else if line_len + 1 + word.len() > options.print_width {
                                        output.push('\n');
                                        output.push_str(&target_indent);
                                        output.push_str(word);
                                        line_len = target_indent.len() + word.len();
                                    } else {
                                        output.push(' ');
                                        output.push_str(word);
                                        line_len += 1 + word.len();
                                    }
                                }
                                output.push('\n');
                            } else {
                                // Never: all words on one line
                                output.push_str(&target_indent);
                                let joined: Vec<&str> = words.into_iter().collect();
                                output.push_str(&joined.join(" "));
                                output.push('\n');
                            }
                        }
                    }
                }
                return;
            }
        }

        for line in &body_lines {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                // Line is empty or whitespace-only.
                // Preserve relative indentation beyond the base for
                // whitespace-only lines (important for |+ keep chomping).
                if !line.is_empty() {
                    let line_indent = line.len();
                    let extra = line_indent.saturating_sub(base_indent);
                    if extra > 0 {
                        output.push_str(&target_indent);
                        // Preserve original extra whitespace chars (may include tabs)
                        output.push_str(&line[base_indent..line_indent]);
                    }
                }
                output.push('\n');
            } else {
                output.push_str(&target_indent);
                // Preserve relative indentation beyond the base, keeping original
                // whitespace characters (tabs, spaces) intact
                let line_indent = line.len() - trimmed.len();
                if line_indent > base_indent {
                    output.push_str(&line[base_indent..line_indent]);
                }
                output.push_str(trimmed);
                output.push('\n');
            }
        }
    }
}

/// Format a quoted scalar, choosing between single and double quotes based on prettier rules.
/// `raw_source` is the raw content between quote delimiters from the original source,
/// used to preserve original escape sequences (e.g. \u263A, \x0d, \/).
fn format_quoted_scalar(
    value: &str,
    raw_source: Option<&str>,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    was_single_quoted: bool,
) {
    let contains_single = value.contains('\'');
    let contains_double = value.contains('"');
    let contains_newline = value.contains('\n');

    // Check if we have a multi-line raw source (originally multi-line in source)
    let raw_is_multiline = raw_source.is_some_and(|r| r.contains('\n'));

    // Determine which quote style to use
    let use_single = if contains_newline && !raw_is_multiline {
        // Single-line escaped form: must use double quotes for \n escapes
        false
    } else if contains_newline && raw_is_multiline {
        // Multi-line form: will be output as multi-line, double quotes required
        false
    } else if options.single_quote {
        // Prefer single quotes, but use double when string contains only single quotes
        // (no double quotes), since double-quoting is more efficient in that case
        if contains_single && !contains_double {
            false // Use double quotes (e.g., "'" instead of '''')
        } else {
            true // Use single quotes: no single quotes, or both present
        }
    } else if contains_single && contains_double {
        // Both styles require escaping: prefer single quotes (fewer escape chars needed)
        // In YAML, single-quote escaping ('') is simpler than double-quote escaping (\")
        let single_escape_count = value.chars().filter(|&c| c == '\'').count();
        let double_escape_count = value.chars().filter(|&c| c == '"' || c == '\\').count();
        single_escape_count <= double_escape_count
    } else {
        // Prefer double quotes, but use single quotes when it avoids escaping
        (contains_double || value.contains('\\')) && !contains_single
    };

    // For multi-line double-quoted strings, output in multi-line form
    if contains_newline
        && raw_is_multiline
        && !use_single
        && let Some(raw) = raw_source
    {
        format_multiline_double_quoted(raw, output, depth, options);
        return;
    }

    // Only use raw_source for single-line strings
    let raw_single_line = raw_source.filter(|r| !r.contains('\n'));

    if use_single {
        // If the output style matches the original style and we have raw source, use it
        if was_single_quoted && let Some(raw) = raw_single_line {
            output.push('\'');
            output.push_str(raw);
            output.push('\'');
            return;
        }
        output.push('\'');
        output.push_str(&value.replace('\'', "''"));
        output.push('\'');
    } else {
        // Double-quoted output
        if !was_single_quoted {
            // Originally double-quoted -> use raw source to preserve escapes
            if let Some(raw) = raw_single_line {
                output.push('"');
                output.push_str(raw);
                output.push('"');
                return;
            }
        }
        output.push('"');
        output.push_str(&escape_double_quoted(value));
        output.push('"');
    }
}

/// Format a multi-line double-quoted string, preserving the multi-line form
/// with proper re-indentation of continuation lines.
fn format_multiline_double_quoted(
    raw_source: &str,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let indent = if depth == 0 {
        String::new()
    } else {
        indent_str(depth, options)
    };
    let lines: Vec<&str> = raw_source.split('\n').collect();

    output.push('"');
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            // First line: output directly after the opening quote
            output.push_str(line);
        } else {
            output.push('\n');
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Blank line: keep empty (represents \n in the value)
            } else {
                // Continuation line: re-indent to target depth
                output.push_str(&indent);
                output.push_str(trimmed);
            }
        }
    }
    output.push('"');
}

fn escape_double_quoted(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\u{08}' => result.push_str("\\b"),
            '\u{07}' => result.push_str("\\a"),
            '\u{0B}' => result.push_str("\\v"),
            '\u{0C}' => result.push_str("\\f"),
            '\u{1B}' => result.push_str("\\e"),
            c if c.is_control() => {
                let _ = write!(result, "\\x{:02X}", c as u32);
            }
            c => result.push(c),
        }
    }
    result
}

fn indent_str(depth: usize, options: &PrettierOptions) -> String {
    // YAML does not support tab indentation, so always use spaces
    // (useTabs is ignored for YAML, matching prettier's behavior)
    " ".repeat(depth * options.tab_width)
}

/// Compute the correct indent for a comment based on both structural depth and
/// the comment's original source column. Prettier normalizes comment indentation
/// to the nearest structural level that is >= the comment's source depth.
fn comment_indent(comment: &Comment, min_depth: usize, options: &PrettierOptions) -> String {
    let tw = options.tab_width;
    // Compute the depth implied by the comment's source column
    let source_depth = if tw > 0 { comment.col.div_ceil(tw) } else { 0 };
    let depth = min_depth.max(source_depth);
    indent_str(depth, options)
}

/// Like `comment_indent` but caps the depth to prevent comments from rendering
/// deeper than the structural context allows.
fn comment_indent_capped(
    comment: &Comment,
    min_depth: usize,
    max_depth: usize,
    options: &PrettierOptions,
) -> String {
    let tw = options.tab_width;
    let source_depth = if tw > 0 { comment.col.div_ceil(tw) } else { 0 };
    let depth = min_depth.max(source_depth).min(max_depth);
    indent_str(depth, options)
}

#[allow(clippy::too_many_lines)]
fn format_block_mapping(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    is_top: bool,
    inline: bool,
) {
    if mapping.entries.is_empty() {
        output.push_str("{}");
        return;
    }

    // Check if this mapping is a !!set (preserve ? key format for null values)
    let is_set = mapping.tag.as_deref().is_some_and(|t| t.contains("set"));

    // Write tag and anchor
    let has_props = mapping.tag.is_some() || mapping.anchor.is_some();
    if has_props {
        if let Some(tag) = &mapping.tag {
            output.push_str(tag);
        }
        if let Some(anchor) = &mapping.anchor {
            if mapping.tag.is_some() {
                output.push(' ');
            }
            output.push('&');
            output.push_str(anchor);
        }
        // Middle comments
        if mapping.middle_comments.len() == 1 {
            // Single comment: on same line as props
            output.push(' ');
            output.push_str(&mapping.middle_comments[0].text);
            output.push('\n');
        } else if mapping.middle_comments.is_empty() {
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            output.push('\n');
            for comment in &mapping.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    } else if !is_top && !inline {
        output.push('\n');
    }

    let indent = indent_str(depth, options);
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &entry.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent(comment, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the entry key
        if entry.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if !is_top || i > 0 || has_props {
            output.push_str(&indent);
        }

        if entry.is_explicit_key
            && is_null_value(&entry.value)
            && !has_node_props(&entry.value)
            && is_set
        {
            // Set-style explicit key with null value: ? key
            output.push_str("? ");
            format_node(&entry.key, output, depth + 1, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if entry.is_explicit_key
            && (is_collection(&entry.key) || is_block_scalar_value(&entry.key))
        {
            // Keep explicit key format for complex keys (mappings, sequences, block scalars)
            format_explicit_key_entry(entry, output, depth, options);
        } else {
            // Write key
            format_node(&entry.key, output, depth, options, false, true);
            output.push(':');

            // Write value
            if is_block_scalar_value(&entry.value) {
                // Block scalar: inline after colon, body on next lines
                output.push(' ');
                format_node(&entry.value, output, depth + 1, options, false, true);
                // Block scalar already outputs trailing newline
            } else if is_simple_value(&entry.value) {
                output.push(' ');
                // For plain scalars: pass the first-line prefix length for wrapping
                if let Node::Scalar(ref scalar) = entry.value {
                    if scalar.style == ScalarStyle::Plain
                        && scalar.value != "~"
                        && !scalar.value.is_empty()
                    {
                        let line_start = output.rfind('\n').map_or(0, |i| i + 1);
                        let first_line_prefix = output.len() - line_start;

                        // Check if we need to break value to the next line.
                        // "always" mode: break single-paragraph values that exceed width
                        // "never" mode: break multi-paragraph values whose first
                        //   paragraph exceeds width (since never-mode can't word-wrap)
                        let should_break = {
                            use crate::options::ProseWrap;
                            let has_para_break = scalar.value.contains('\n');
                            match options.prose_wrap {
                                ProseWrap::Always if !has_para_break => {
                                    let len = scalar.value.trim().len();
                                    let can_break = scalar.value.contains(' ');
                                    first_line_prefix + len > options.print_width && can_break
                                }
                                ProseWrap::Never if has_para_break => {
                                    let first_para = scalar.value.split('\n').next().unwrap_or("");
                                    let len = first_para.trim().len();
                                    first_line_prefix + len > options.print_width
                                }
                                _ => false,
                            }
                        };
                        if should_break {
                            // Break: remove the trailing space, add newline + indent
                            output.pop(); // remove the ' ' we just pushed
                            output.push('\n');
                            let val_indent = indent_str(depth + 1, options);
                            output.push_str(&val_indent);
                            format_scalar(scalar, output, depth + 1, options, val_indent.len());
                            if let Some(comment) = &entry.trailing_comment {
                                output.push(' ');
                                output.push_str(comment);
                            }
                            output.push('\n');
                            continue;
                        }
                        format_scalar(scalar, output, depth + 1, options, first_line_prefix);
                    } else {
                        format_scalar(scalar, output, depth + 1, options, 0);
                    }
                } else {
                    format_node(&entry.value, output, depth, options, false, true);
                }
                // Trailing comment
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            } else if is_null_value(&entry.value) {
                // Null value - but may still have anchor/tag
                if has_node_props(&entry.value) {
                    output.push(' ');
                    format_node(&entry.value, output, depth, options, false, true);
                }
                // Trailing comment for null value
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            } else {
                // Complex value on next line
                // Add space after colon if value has props (anchor/tag) so we get
                // `key: &anchor\n` instead of `key:&anchor\n`
                if has_node_props(&entry.value) {
                    output.push(' ');
                }
                // Key trailing comment: if the value has no props, output inline.
                // If value has props, it was injected into the value's middle_comments
                // during AST construction so it appears after the props.
                if !has_node_props(&entry.value)
                    && let Some(comment) = &entry.key_trailing_comment
                {
                    output.push(' ');
                    output.push_str(comment);
                }
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                // Standalone between comments (between key and value, on their own lines)
                if !entry.between_comments.is_empty() {
                    for comment in &entry.between_comments {
                        output.push('\n');
                        let ci = comment_indent(comment, depth + 1, options);
                        output.push_str(&ci);
                        output.push_str(&comment.text);
                    }
                }
                format_node(&entry.value, output, depth + 1, options, false, false);
            }
        }
    }

    // Write trailing comments (comments after last entry in the mapping)
    // Cap indent at depth+1 so comments "breaking out" of child scopes don't
    // get indented deeper than one level beyond the mapping's own entries.
    for comment in &mapping.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent_capped(comment, depth, depth + 1, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

fn format_explicit_key_entry(
    entry: &MappingEntry,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    // ? key\n: value
    output.push_str("? ");
    format_node(&entry.key, output, depth + 1, options, false, true);
    output.push('\n');

    let indent = indent_str(depth, options);
    output.push_str(&indent);
    output.push(':');

    if is_simple_value(&entry.value) {
        output.push(' ');
        format_node(&entry.value, output, depth, options, false, true);
        output.push('\n');
    } else if is_null_value(&entry.value) {
        output.push('\n');
    } else {
        output.push('\n');
        format_node(&entry.value, output, depth + 1, options, false, false);
    }
}

#[allow(clippy::too_many_lines)]
fn format_block_sequence(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
    is_top: bool,
    _inline: bool,
) {
    if seq.items.is_empty() {
        output.push_str("[]");
        return;
    }

    // Write tag and anchor
    let has_props = seq.tag.is_some() || seq.anchor.is_some();
    if has_props {
        if let Some(tag) = &seq.tag {
            output.push_str(tag);
        }
        if let Some(anchor) = &seq.anchor {
            if seq.tag.is_some() {
                output.push(' ');
            }
            output.push('&');
            output.push_str(anchor);
        }
        // Middle comments
        if seq.middle_comments.len() == 1 {
            // Single comment: on same line as props
            output.push(' ');
            output.push_str(&seq.middle_comments[0].text);
            output.push('\n');
        } else if seq.middle_comments.is_empty() {
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            output.push('\n');
            let indent = indent_str(depth, options);
            for comment in &seq.middle_comments {
                output.push_str(&indent);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    } else if !is_top {
        output.push('\n');
    }

    let indent = indent_str(depth, options);
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent(comment, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the item
        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if !is_top || i > 0 || has_props {
            output.push_str(&indent);
        }

        match &item.value {
            Node::Mapping(m) if !m.flow && !m.entries.is_empty() => {
                // Inline first entry after dash, with optional anchor/tag
                output.push_str("- ");
                if let Some(anchor) = &m.anchor {
                    output.push('&');
                    output.push_str(anchor);
                    output.push('\n');
                    let entry_indent = indent_str(depth + 1, options);
                    output.push_str(&entry_indent);
                }
                let first = &m.entries[0];
                format_node(&first.key, output, depth + 1, options, false, true);
                output.push(':');

                if is_block_scalar_value(&first.value) {
                    output.push(' ');
                    format_node(&first.value, output, depth + 2, options, false, true);
                } else if is_simple_value(&first.value) {
                    output.push(' ');
                    format_node(&first.value, output, depth + 1, options, false, true);
                    if let Some(comment) = &first.trailing_comment {
                        output.push(' ');
                        output.push_str(comment);
                    }
                    output.push('\n');
                } else if is_null_value(&first.value) {
                    if let Some(comment) = &first.trailing_comment {
                        output.push(' ');
                        output.push_str(comment);
                    }
                    output.push('\n');
                } else {
                    if has_node_props(&first.value) {
                        output.push(' ');
                    }
                    if let Some(comment) = &first.trailing_comment {
                        output.push(' ');
                        output.push_str(comment);
                    }
                    format_node(&first.value, output, depth + 2, options, false, false);
                }

                // Remaining entries at deeper indent
                let entry_indent = indent_str(depth + 1, options);
                for entry in m.entries.iter().skip(1) {
                    for comment in &entry.leading_comments {
                        if comment.blank_line_before && !output.ends_with("\n\n") {
                            output.push('\n');
                        }
                        output.push_str(&entry_indent);
                        output.push_str(&comment.text);
                        output.push('\n');
                    }
                    if entry.blank_line_before && !output.ends_with("\n\n") {
                        output.push('\n');
                    }
                    output.push_str(&entry_indent);
                    format_node(&entry.key, output, depth + 1, options, false, true);
                    output.push(':');

                    if is_block_scalar_value(&entry.value) {
                        output.push(' ');
                        format_node(&entry.value, output, depth + 2, options, false, true);
                    } else if is_simple_value(&entry.value) {
                        output.push(' ');
                        format_node(&entry.value, output, depth + 1, options, false, true);
                        if let Some(comment) = &entry.trailing_comment {
                            output.push(' ');
                            output.push_str(comment);
                        }
                        output.push('\n');
                    } else if is_null_value(&entry.value) {
                        if let Some(comment) = &entry.trailing_comment {
                            output.push(' ');
                            output.push_str(comment);
                        }
                        output.push('\n');
                    } else {
                        if has_node_props(&entry.value) {
                            output.push(' ');
                        }
                        if let Some(comment) = &entry.trailing_comment {
                            output.push(' ');
                            output.push_str(comment);
                        }
                        format_node(&entry.value, output, depth + 2, options, false, false);
                    }
                }

                // Write trailing comments of the mapping (e.g., comments after last entry)
                for comment in &m.trailing_comments {
                    if comment.blank_line_before && !output.ends_with("\n\n") {
                        output.push('\n');
                    }
                    let ci = comment_indent(comment, depth + 1, options);
                    output.push_str(&ci);
                    output.push_str(&comment.text);
                    output.push('\n');
                }

                if let Some(comment) = &item.trailing_comment {
                    output.push_str(&indent);
                    output.push_str(comment);
                    output.push('\n');
                }
            }
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                // Nested sequence: `- - item`
                output.push_str("- ");
                format_nested_sequence_inline(s, output, depth + 1, options);
            }
            _ => {
                if is_null_value(&item.value) {
                    output.push('-');
                } else if is_block_scalar_value(&item.value) {
                    output.push_str("- ");
                    format_node(&item.value, output, depth + 1, options, false, true);
                    // Block scalar already outputs trailing newline
                    if let Some(comment) = &item.trailing_comment {
                        output.push_str(&indent);
                        output.push_str(comment);
                        output.push('\n');
                    }
                    continue;
                } else {
                    output.push_str("- ");
                    format_node(&item.value, output, depth + 1, options, false, true);
                }
                if let Some(comment) = &item.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            }
        }
    }

    // Write trailing comments (comments after last item in the sequence)
    for comment in &seq.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, depth, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

/// Format a nested sequence inline: `- item1\n  - item2` etc.
fn format_nested_sequence_inline(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let indent = indent_str(depth, options);
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            // Cap comment indent at the sequence item level (depth)
            let ci = comment_indent_capped(comment, depth, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and this item
        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if i > 0 {
            output.push_str(&indent);
        }
        match &item.value {
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                output.push_str("- ");
                format_nested_sequence_inline(s, output, depth + 1, options);
            }
            _ => {
                output.push_str("- ");
                if is_null_value(&item.value) {
                    output.pop();
                }
                format_node(&item.value, output, depth + 1, options, false, true);
                if let Some(comment) = &item.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            }
        }
    }

    // Trailing comments of the nested sequence
    // Cap indent at the sequence's item level (depth)
    for comment in &seq.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent_capped(comment, depth, depth, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

fn format_flow_mapping(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let has_props = mapping.tag.is_some() || mapping.anchor.is_some();
    let has_middle_comments = !mapping.middle_comments.is_empty();

    // Write tag and anchor
    if let Some(tag) = &mapping.tag {
        output.push_str(tag);
        output.push(' ');
    }
    if let Some(anchor) = &mapping.anchor {
        output.push('&');
        output.push_str(anchor);
        output.push(' ');
    }

    // Middle comments go between props and content
    if has_middle_comments {
        if mapping.middle_comments.len() == 1 && has_props {
            // Single middle comment: on same line as props
            output.push_str(&mapping.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            if has_props {
                // Trim trailing space added after tag/anchor
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push('\n');
            }
            for comment in &mapping.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    }

    if mapping.entries.is_empty() {
        output.push_str("{}");
        return;
    }

    // If there are comments or trailing comments, force broken format
    let has_entry_comments = mapping.entries.iter().any(|e| {
        e.trailing_comment.is_some()
            || e.key_trailing_comment.is_some()
            || !e.leading_comments.is_empty()
            || !e.between_comments.is_empty()
    });

    if has_middle_comments || has_entry_comments || !mapping.trailing_comments.is_empty() {
        format_flow_mapping_broken(mapping, output, depth, options);
        return;
    }

    // Try flat format first
    let flat = format_flow_mapping_flat(mapping, depth, options);

    // If flat result contains newlines (nested broken collections), go to broken
    let current_col = depth * options.tab_width;
    if !flat.contains('\n') && current_col + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        // Break to multi-line
        format_flow_mapping_broken(mapping, output, depth, options);
    }
}

fn format_flow_mapping_flat(
    mapping: &MappingNode,
    depth: usize,
    options: &PrettierOptions,
) -> String {
    let mut parts = Vec::new();
    for entry in &mapping.entries {
        let mut part = String::new();
        // In flow mappings, prettier drops `?` for explicit-key entries
        // (unlike flow sequences, where `?` is preserved)

        // Handle null key with null value (`: ` entry)
        let key_is_null = is_null_value(&entry.key);
        if key_is_null && is_null_value(&entry.value) {
            if options.bracket_spacing {
                part.push(':');
            } else {
                part.push_str(": ");
            }
            parts.push(part);
            continue;
        }
        format_node(&entry.key, &mut part, depth, options, false, true);
        if is_null_value(&entry.value) {
            // Null value: just the key, no ": ~"
        } else {
            // Alias keys need space before colon (e.g. `*foo : bar`)
            if matches!(&entry.key, Node::Alias(_)) {
                part.push_str(" : ");
            } else {
                part.push_str(": ");
            }
            format_node(&entry.value, &mut part, depth, options, false, true);
        }
        parts.push(part);
    }

    if options.bracket_spacing {
        format!("{{ {} }}", parts.join(", "))
    } else {
        format!("{{{}}}", parts.join(", "))
    }
}

/// Check if a node would render as multi-line (contains newlines).
fn renders_multiline(node: &Node, depth: usize, options: &PrettierOptions) -> bool {
    let mut buf = String::new();
    format_node(node, &mut buf, depth, options, false, true);
    buf.contains('\n')
}

/// Check if a node is a collection (mapping or sequence).
fn is_collection(node: &Node) -> bool {
    matches!(node, Node::Mapping(_) | Node::Sequence(_))
}

fn format_flow_mapping_broken(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let inner_indent = indent_str(depth + 1, options);
    let outer_indent = indent_str(depth, options);

    output.push_str("{\n");
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &entry.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the entry key
        if entry.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        let key_is_complex = is_collection(&entry.key);
        let key_is_null = is_null_value(&entry.key);
        let value_is_null = is_null_value(&entry.value);
        let value_is_multiline =
            !value_is_null && renders_multiline(&entry.value, depth + 2, options);

        let has_between =
            !entry.between_comments.is_empty() || entry.key_trailing_comment.is_some();

        if key_is_null && value_is_null {
            // null key + null value = `: `
            output.push_str(&inner_indent);
            output.push_str(": ");
        } else if (key_is_complex || has_between) && !value_is_null {
            // Complex key or comments between key-value: use ? key \n [comments] \n : value
            output.push_str(&inner_indent);
            output.push_str("? ");
            format_node(&entry.key, output, depth + 2, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            for comment in &entry.between_comments {
                output.push('\n');
                output.push_str(&inner_indent);
                output.push_str(&comment.text);
            }
            output.push('\n');
            output.push_str(&inner_indent);
            output.push_str(": ");
            format_node(&entry.value, output, depth + 2, options, false, true);
        } else if key_is_complex {
            // Complex key with null value: just the key
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
        } else if value_is_multiline {
            // Simple key with multiline value: key:\n  value
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
            output.push_str(":\n");
            let value_indent = indent_str(depth + 2, options);
            output.push_str(&value_indent);
            format_node(&entry.value, output, depth + 2, options, false, true);
        } else {
            // Simple key with simple value (or null)
            // Note: In flow mappings, prettier drops `?` for explicit keys
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
            if !value_is_null {
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    output.push_str(" : ");
                } else {
                    output.push_str(": ");
                }
                format_node(&entry.value, output, depth + 1, options, false, true);
            }
        }
        // Always trailing comma (prettier style)
        output.push(',');
        if let Some(comment) = &entry.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    }
    output.push_str(&outer_indent);
    output.push('}');
}

fn format_flow_sequence(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let has_props = seq.tag.is_some() || seq.anchor.is_some();
    let has_middle_comments = !seq.middle_comments.is_empty();

    // Write tag and anchor
    if let Some(tag) = &seq.tag {
        output.push_str(tag);
        output.push(' ');
    }
    if let Some(anchor) = &seq.anchor {
        output.push('&');
        output.push_str(anchor);
        output.push(' ');
    }

    // Middle comments go between props and content
    if has_middle_comments {
        if seq.middle_comments.len() == 1 && has_props {
            // Single middle comment: on same line as props
            output.push_str(&seq.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            if has_props {
                // Trim trailing space added after tag/anchor
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push('\n');
            }
            for comment in &seq.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    }

    if seq.items.is_empty() {
        output.push_str("[]");
        return;
    }

    // If there are comments, force broken format
    let has_item_comments = seq
        .items
        .iter()
        .any(|item| item.trailing_comment.is_some() || !item.leading_comments.is_empty());
    // Also check for comments in implicit mapping entries
    let has_mapping_comments = seq.items.iter().any(|item| {
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let e = &m.entries[0];
            return e.trailing_comment.is_some()
                || e.key_trailing_comment.is_some()
                || !e.leading_comments.is_empty()
                || !e.between_comments.is_empty();
        }
        false
    });

    if has_middle_comments
        || has_item_comments
        || has_mapping_comments
        || !seq.trailing_comments.is_empty()
    {
        format_flow_sequence_broken(seq, output, depth, options);
        return;
    }

    // Try flat format
    let flat = format_flow_sequence_flat(seq, depth, options);

    // If flat result contains newlines (nested broken collections), go to broken
    let current_col = depth * options.tab_width;
    if !flat.contains('\n') && current_col + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        format_flow_sequence_broken(seq, output, depth, options);
    }
}

fn format_flow_sequence_flat(
    seq: &SequenceNode,
    depth: usize,
    options: &PrettierOptions,
) -> String {
    let mut parts = Vec::new();
    for item in &seq.items {
        let mut part = String::new();
        // Check if item is a single-entry flow mapping (key-value pair in sequence)
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let entry = &m.entries[0];
            let key_is_null = is_null_value(&entry.key);
            // If value is a collection and key is a simple scalar, wrap in {} for clarity
            let value_is_collection = matches!(&entry.value, Node::Mapping(_) | Node::Sequence(_));
            let key_is_simple = matches!(&entry.key, Node::Scalar(_)) && !key_is_null;
            if value_is_collection && key_is_simple && !entry.is_explicit_key {
                // Format as { key: value } (explicit mapping)
                format_node(&item.value, &mut part, depth, options, false, true);
                parts.push(part);
                continue;
            }
            if key_is_null && is_null_value(&entry.value) {
                // null key + null value = `: `
                part.push_str(": ");
                parts.push(part);
                continue;
            }
            if entry.is_explicit_key {
                part.push_str("? ");
            }
            format_node(&entry.key, &mut part, depth, options, false, true);
            if !is_null_value(&entry.value) {
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    part.push_str(" : ");
                } else {
                    part.push_str(": ");
                }
                format_node(&entry.value, &mut part, depth, options, false, true);
            }
            parts.push(part);
            continue;
        }
        format_node(&item.value, &mut part, depth, options, false, true);
        parts.push(part);
    }
    format!("[{}]", parts.join(", "))
}

fn format_flow_sequence_broken(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &PrettierOptions,
) {
    let inner_indent = indent_str(depth + 1, options);
    let outer_indent = indent_str(depth, options);

    output.push_str("[\n");
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the item
        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        // Check if item is a single-entry flow mapping (key-value pair in sequence)
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let entry = &m.entries[0];
            let key_is_complex = is_collection(&entry.key);
            let key_is_null = is_null_value(&entry.key);
            let value_is_null = is_null_value(&entry.value);
            let has_between =
                !entry.between_comments.is_empty() || entry.key_trailing_comment.is_some();
            let value_is_collection = matches!(&entry.value, Node::Mapping(_) | Node::Sequence(_));
            let key_is_simple = matches!(&entry.key, Node::Scalar(_)) && !key_is_null;

            // If value is a collection and key is simple, format as { key: value } for clarity
            if value_is_collection && key_is_simple && !entry.is_explicit_key {
                output.push_str(&inner_indent);
                format_node(&item.value, output, depth + 1, options, false, true);
                output.push(',');
                output.push('\n');
                continue;
            }

            if key_is_null && value_is_null {
                // null:null -> ": "
                output.push_str(&inner_indent);
                output.push_str(": ");
            } else if (key_is_complex || has_between) && !value_is_null {
                // ? key \n [comments] \n : value
                output.push_str(&inner_indent);
                output.push_str("? ");
                format_node(&entry.key, output, depth + 2, options, false, true);
                if let Some(comment) = &entry.key_trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                for comment in &entry.between_comments {
                    output.push('\n');
                    output.push_str(&inner_indent);
                    output.push_str(&comment.text);
                }
                output.push('\n');
                output.push_str(&inner_indent);
                output.push_str(": ");
                format_node(&entry.value, output, depth + 2, options, false, true);
            } else if key_is_complex || (entry.is_explicit_key && value_is_null) {
                // ? key (null value) — explicit key syntax for long keys
                output.push_str(&inner_indent);
                output.push_str("? ");
                format_node(&entry.key, output, depth + 2, options, false, true);
            } else {
                // simple key: value
                output.push_str(&inner_indent);
                format_node(&entry.key, output, depth + 1, options, false, true);
                if !value_is_null {
                    // Alias keys need space before colon
                    if matches!(&entry.key, Node::Alias(_)) {
                        output.push_str(" : ");
                    } else {
                        output.push_str(": ");
                    }
                    format_node(&entry.value, output, depth + 2, options, false, true);
                }
            }
            output.push(',');
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
            continue;
        }

        output.push_str(&inner_indent);
        format_node(&item.value, output, depth + 1, options, false, true);
        output.push(',');
        if let Some(comment) = &item.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    }
    output.push_str(&outer_indent);
    output.push(']');
}

fn is_simple_value(node: &Node) -> bool {
    match node {
        Node::Scalar(s) => !s.is_implicit_null,
        Node::Alias(_) => true,
        Node::Mapping(m) => m.flow,
        Node::Sequence(s) => s.flow,
    }
}

fn is_null_value(node: &Node) -> bool {
    match node {
        Node::Scalar(s) => s.is_implicit_null,
        _ => false,
    }
}

fn is_block_scalar_value(node: &Node) -> bool {
    matches!(
        node,
        Node::Scalar(s) if matches!(s.style, ScalarStyle::Literal | ScalarStyle::Folded)
    )
}

/// Check if a node has properties (anchor, tag) that would need a space separator.
fn has_node_props(node: &Node) -> bool {
    match node {
        Node::Mapping(m) => m.anchor.is_some() || m.tag.is_some(),
        Node::Sequence(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Scalar(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Alias(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_mapping() {
        let input = "a: 1\nb: 2\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "a: 1\nb: 2\n");
    }

    #[test]
    fn format_simple_sequence() {
        let input = "- 1\n- 2\n- 3\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "- 1\n- 2\n- 3\n");
    }

    #[test]
    fn format_null_values() {
        let input = "a:\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "a:\n");
    }

    #[test]
    fn format_nested_mapping() {
        let input = "key:\n  nested: value\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "key:\n  nested: value\n");
    }

    #[test]
    fn format_sequence_of_mappings() {
        let input = "- a: b\n  c: d\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "- a: b\n  c: d\n");
    }

    #[test]
    fn format_block_literal_clip() {
        let input = "|\n    123\n    456\n    789\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "|\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_keep() {
        let input = "|+\n    123\n    456\n    789\n\n\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "|+\n  123\n  456\n  789\n\n\n");
    }

    #[test]
    fn format_block_literal_in_mapping() {
        let input = "a: |\n  123\n  456\n  789\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        eprintln!("result: {:?}", result);
        assert_eq!(result, "a: |\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_multi_entry_map() {
        let input = "a: |\n  123\n  456\n  789\nb: |1\n    123\n   456\n  789\nd: |\n  123\n  456\n  789\n\nc: 0\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, input);
    }

    #[test]
    fn format_flow_seq_alias_key_flat() {
        let input = "[&123 foo, *123 : 456]\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        // Single-entry mapping in flow sequence stays flat
        // TODO: prettier removes space before colon after alias keys (*123: 456)
        assert_eq!(result, "[&123 foo, *123 : 456]\n");
    }
}
