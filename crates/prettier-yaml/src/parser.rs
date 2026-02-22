use anyhow::{Context, Result};
use saphyr_parser::{Event, Parser, ScalarStyle, Span};

use crate::ast::{
    AliasNode, Comment, MappingEntry, MappingNode, Node, ScalarNode, SequenceItem, SequenceNode,
    YamlDoc, YamlStream,
};
use crate::comments::SourceComment;
use crate::utilities::is_anchor_char;

pub(crate) fn collect_events(content: &str) -> Result<Vec<(Event<'_>, Span)>> {
    let parser = Parser::new_from_str(content);
    let mut events = Vec::new();
    for result in parser {
        let (event, span) = result.context("YAML parse error")?;
        events.push((event, span));
    }
    Ok(events)
}

pub(crate) struct AstBuilder<'a> {
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
    /// Body end line (0-indexed) from the last block scalar extraction.
    /// Used to override saphyr's span.end for comment collection between entries.
    last_block_scalar_body_end: Option<usize>,
    /// When set, the next root-level sequence/mapping should use this line as
    /// `prev_end_line` for the first child (to collect comments between `---`
    /// and the root node's first content line).
    doc_start_line_for_root: Option<usize>,
}

impl<'a> AstBuilder<'a> {
    pub fn new(
        source: &'a str,
        events: &'a [(Event<'a>, Span)],
        comments: &'a [SourceComment],
    ) -> Self {
        let source_lines: Vec<&str> = source.lines().collect();
        let used = vec![false; comments.len()];
        let mut char_to_byte: Vec<usize> = source.char_indices().map(|(b, _)| b).collect();
        char_to_byte.push(source.len());
        AstBuilder {
            source,
            source_lines,
            events,
            comments,
            pos: 0,
            used_comment_lines: used,
            char_to_byte,
            in_flow_context: 0,
            last_block_scalar_body_end: None,
            doc_start_line_for_root: None,
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

    pub fn build_stream(&mut self) -> Result<YamlStream> {
        // Skip StreamStart
        self.advance();

        let mut documents = Vec::new();
        let mut last_doc_end = 0;
        while let Some((event, _)) = self.peek() {
            match event {
                Event::StreamEnd => {
                    self.advance();
                    break;
                }
                Event::DocumentStart(_) => {
                    // Before building the next document, check if saphyr skipped
                    // an empty document between `...` markers. This happens when
                    // the source has `...\n# comment\n...\n` — saphyr merges the
                    // empty document, but prettier treats it as a separate doc.
                    if !documents.is_empty()
                        && let Some(synthetic) = self.check_skipped_empty_document(last_doc_end)
                    {
                        last_doc_end = synthetic
                            .end_comments
                            .last()
                            .map_or(last_doc_end, |c| c.line);
                        documents.push(synthetic);
                    }
                    documents.push(self.build_document(last_doc_end)?);
                    last_doc_end = self.last_content_end_line();
                }
                _ => {
                    break;
                }
            }
        }

        // Use last document content end (not StreamEnd) so blank line detection
        // between content and trailing comments works correctly.
        let trailing_comments = self.collect_remaining_comments(last_doc_end);

        Ok(YamlStream {
            documents,
            trailing_comments,
        })
    }

    fn build_document(&mut self, prev_doc_end: usize) -> Result<YamlDoc> {
        let (event, doc_span) = self.advance();
        let explicit_start = matches!(event, Event::DocumentStart(true));
        let doc_start_line = doc_span.start.line();

        // Only collect preamble (directives, comments before `---`) for
        // explicit document starts. Implicit documents don't have preamble.
        let preamble = if explicit_start {
            self.collect_preamble_between_lines(prev_doc_end, doc_start_line)
        } else {
            Vec::new()
        };

        // Check for trailing comment on the `---` line (e.g. `--- # comment`)
        let start_comment = if explicit_start {
            self.find_doc_marker_comment(doc_start_line, "---")
        } else {
            None
        };

        // Check for prettier-ignore in preamble
        let has_prettier_ignore = preamble.iter().any(|l| l.trim() == "# prettier-ignore");
        let root_start_line = if has_prettier_ignore {
            self.peek().map(|(_, s)| s.start.line())
        } else {
            None
        };

        // Tell the root sequence/mapping to collect comments starting from the
        // document start line instead of the collection's own span start. This
        // ensures comments between `---` (or stream start) and the first item
        // are captured.
        // For explicit documents: use the `---` line.
        // For implicit documents: use line 0 to capture any comments before
        // the first content line (e.g. `#6445\n\nobj:\n`).
        self.doc_start_line_for_root = Some(if explicit_start {
            doc_start_line
        } else {
            // Use prev doc end or stream start
            0
        });

        // Capture the root node's start line before building, so we can collect
        // comments between `---` and the root body for scalar roots.
        let root_node_start_line = self.peek().map(|(_, s)| s.start.line());

        let root = self.build_node()?;
        // For block scalar root nodes, saphyr's span extends past the body
        // into trailing blank/comment lines. Use the actual body end instead.
        let content_end_line = if let Some(body_end_0idx) = self.last_block_scalar_body_end.take() {
            body_end_0idx + 1 // convert 0-indexed to 1-indexed
        } else {
            self.last_content_end_line()
        };

        // Collect comments between the document start and the root body for
        // scalar/alias roots. For collection roots, `doc_start_line_for_root`
        // handles this inside build_sequence/build_mapping (comments are
        // already consumed), so body_leading_comments will be empty.
        // For implicit documents, use line 0 as the start to capture comments
        // before the first content (e.g. `# Private\n!foo "bar"`).
        let body_leading_comments = if let Some(root_line) = root_node_start_line {
            let from_line = if explicit_start { doc_start_line } else { 0 };
            self.collect_comments_between_lines(from_line, root_line)
        } else {
            Vec::new()
        };

        // Capture trailing comment on the root node's last line.
        // This handles cases like `!!int 1 - 3 # Interval` where the comment
        // is on the same line as a root scalar.
        let root_trailing_comment = self.find_trailing_comment(content_end_line).map(|c| c.text);

        // Capture raw body for prettier-ignore documents
        let raw_body_source = if let Some(start) = root_start_line {
            let end = self.last_content_end_line();
            let start_idx = start.saturating_sub(1);
            let end_idx = end.min(self.source_lines.len());
            if start_idx < end_idx {
                let lines = &self.source_lines[start_idx..end_idx];
                Some(lines.join("\n"))
            } else {
                None
            }
        } else {
            None
        };

        let explicit_end = if let Some((Event::DocumentEnd, span)) = self.peek() {
            let span = *span;
            self.advance();
            self.check_explicit_doc_end(&span)
        } else {
            false
        };

        // Check for trailing comment on the `...` line (e.g. `... # Suffix`)
        let end_marker_comment = if explicit_end {
            let end_line = self.last_event_end_line();
            self.find_doc_marker_comment(end_line, "...")
        } else {
            None
        };

        // Collect comments between root content end and document boundary.
        // For explicit `...` end: collect up to the end marker line.
        // For implicit end: collect up to the next event (next doc start or stream end).
        let doc_boundary_line = if explicit_end {
            self.last_event_end_line()
        } else {
            self.peek()
                .map_or(content_end_line, |(_, s)| s.start.line())
        };
        let end_comments = self.collect_comments_between_lines(content_end_line, doc_boundary_line);

        Ok(YamlDoc {
            explicit_start,
            explicit_end,
            preamble,
            root,
            end_comments,
            start_comment,
            end_marker_comment,
            root_trailing_comment,
            raw_body_source,
            body_leading_comments,
        })
    }

    fn build_node(&mut self) -> Result<Option<Node>> {
        let Some((event, _span)) = self.peek() else {
            return Ok(None);
        };

        match event {
            Event::Scalar(_, _, _, _) => {
                // Clear doc_start_line since scalars don't use it
                self.doc_start_line_for_root = None;
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
            self.extract_tag_before(&span)
        } else {
            None
        };

        let is_implicit_null = self.is_implicit_null(&span, &value, style);

        let block_source = if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded) {
            let (bs, indicator_line_idx, body_end_idx) =
                self.extract_block_scalar_source(&span, style);
            // Mark header comments as used so they aren't duplicated as
            // leading_comments of the next entry. The header line contains
            // the indicator (| or >) and may have a trailing comment.
            self.mark_block_scalar_header_comments(&span, style);
            // Mark body comments as used — block scalar body lines that start
            // with `#` are content, not standalone comments.
            if let (Some(ind_idx), Some(end_idx)) = (indicator_line_idx, body_end_idx) {
                let body_start = ind_idx + 1; // 0-indexed
                for (i, comment) in self.comments.iter().enumerate() {
                    // Comments are 1-indexed line numbers
                    let line_0 = comment.line.saturating_sub(1);
                    if line_0 >= body_start && line_0 <= end_idx {
                        self.used_comment_lines[i] = true;
                    }
                }
            }
            // Store the body end line so that comment collection after block
            // scalar values uses the correct boundary (not saphyr's span.end).
            self.last_block_scalar_body_end = body_end_idx;
            Some(bs)
        } else {
            self.last_block_scalar_body_end = None;
            None
        };

        let quoted_source =
            if matches!(style, ScalarStyle::DoubleQuoted | ScalarStyle::SingleQuoted) {
                self.extract_quoted_source(&span, style)
            } else {
                None
            };

        let plain_source_lines =
            if matches!(style, ScalarStyle::Plain) && span.end.line() > span.start.line() {
                let start_line = span.start.line();
                let end_line = span.end.line();
                let start_col = span.start.col();
                let mut lines = Vec::new();
                for line_num in start_line..=end_line {
                    let idx = line_num.saturating_sub(1);
                    if idx < self.source_lines.len() {
                        let raw = self.source_lines[idx];
                        // Strip trailing inline comment from plain scalar source
                        // (comments are already captured separately)
                        let without_comment = self
                            .comment_start_col_on_line(line_num)
                            .map_or(raw, |col| &raw[..col]);
                        if line_num == start_line {
                            let content: String = without_comment.chars().skip(start_col).collect();
                            let trimmed = content.trim();
                            if trimmed.is_empty() {
                                lines.push(String::new());
                            } else {
                                lines.push(trimmed.to_string());
                            }
                        } else {
                            let trimmed = without_comment.trim();
                            if trimmed.is_empty() {
                                lines.push(String::new());
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
            let start_byte = self.to_byte(span.start.index());
            let mut props_line = content_line;
            if start_byte > 0 {
                let search_start = start_byte.saturating_sub(300);
                let region = &self.source[search_start..start_byte];
                for (i, b) in region.bytes().enumerate().rev() {
                    if b == b'!' || b == b'&' {
                        let pos = search_start + i;
                        let line = self.source[..pos].matches('\n').count() + 1;
                        props_line = line;
                        break;
                    }
                    if b == b'\n' && i > 0 && region.as_bytes()[i - 1] == b'\n' {
                        break;
                    }
                }
            }
            if content_line > props_line {
                let mut comments = vec![];
                if let Some(tc) = self.find_trailing_comment(props_line) {
                    comments.push(tc);
                }
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

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
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

        let flow_source = if flow && self.is_flow_sequence_at(&span) {
            self.extract_flow_source(&span)
        } else {
            None
        };

        if flow {
            self.in_flow_context += 1;
        }

        let mut items = Vec::new();
        // If this is the document root, use the doc start line so that comments
        // between `---` and the first item are captured as leading comments.
        let mut prev_end_line = self
            .doc_start_line_for_root
            .take()
            .unwrap_or(span.start.line());

        // Collect middle comments (comments between tag/anchor and first entry)
        let mut middle_comments = vec![];
        if anchor.is_some() || tag_str.is_some() {
            let content_line = span.start.line();
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                // Skip past sequence item indicator `- ` or mapping key prefix if present
                let src = src.strip_prefix("- ").unwrap_or(src);
                if src.starts_with('#') {
                    continue;
                }
                // Check if this line contains a tag or anchor, either at the
                // start or anywhere in the line (e.g. `key: !!tag`)
                if src.starts_with('!') || src.starts_with('&') {
                    props_line = l;
                    break;
                }
                if let Some(tag) = &tag_str
                    && src.contains(tag.as_str())
                {
                    props_line = l;
                    break;
                }
                if let Some(anc) = &anchor
                    && src.contains(&format!("&{anc}"))
                {
                    props_line = l;
                    break;
                }
                break;
            }

            // For flow sequences, only capture a trailing comment on props_line
            // as a middle comment if the first item does NOT start on the same line.
            // When items exist on the tag line (e.g. `!!seq [ a, b, # comment]`),
            // the comment belongs to the last item, not the tag.
            let first_item_on_props_line = flow
                && self.peek().is_some_and(|(e, s)| {
                    !matches!(e, Event::SequenceEnd) && s.start.line() == props_line
                });
            if !first_item_on_props_line
                && let Some(comment) = self.find_trailing_comment(props_line)
            {
                middle_comments.push(comment);
            }
            if props_line < content_line {
                let standalone = self.collect_comments_between_lines(props_line, content_line);
                middle_comments.extend(standalone);
            }
            // Only capture trailing comment on content_line as a middle comment
            // when the tag/anchor is on the SAME line. When they're on different
            // lines, any trailing comment on content_line belongs to the first
            // item, not to the sequence's tag-to-content gap.
            if middle_comments.is_empty()
                && props_line == content_line
                && !first_item_on_props_line
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

            // For block scalars or values starting on the next line,
            // item_start_line may differ from the "- " prefix line. Find the
            // actual sequence item marker line by scanning source.
            let marker_line = if !flow && item_start_line > prev_end_line {
                let seq_col = span.start.col();
                // Search from prev_end_line (inclusive for first item, where the
                // SequenceStart line IS the marker line) through item_start_line.
                let search_start = if items.is_empty() {
                    prev_end_line
                } else {
                    prev_end_line + 1
                };
                (search_start..=item_start_line)
                    .find(|&line_1| {
                        let idx = line_1.saturating_sub(1);
                        if idx < self.source_lines.len() {
                            let src = self.source_lines[idx];
                            let line_indent = src.len() - src.trim_start().len();
                            line_indent == seq_col && src.trim_start().starts_with("- ")
                        } else {
                            false
                        }
                    })
                    .unwrap_or(item_start_line)
            } else {
                item_start_line
            };

            let leading_comments = self.collect_comments_between_lines(prev_end_line, marker_line);
            let blank_line_before = if items.is_empty() {
                false
            } else if let Some(last_comment) = leading_comments.last() {
                self.has_blank_line_between(last_comment.line, marker_line)
            } else {
                self.has_blank_line_immediately_before(marker_line)
            };

            // Capture inline comment on the `- ` indicator line when the value
            // starts on a later line AND the only content after `- ` is a comment.
            // e.g. `- #comment\n  value` → indicator_comment = "#comment"
            // If there's any non-comment content (tag, anchor, value, block scalar
            // indicator), the comment belongs to that content instead.
            let indicator_comment = if !flow && marker_line < item_start_line {
                let marker_idx = marker_line.saturating_sub(1);
                let is_comment_only = marker_idx < self.source_lines.len() && {
                    let after_dash = self.source_lines[marker_idx]
                        .trim_start()
                        .strip_prefix("- ")
                        .unwrap_or("");
                    let content = after_dash.trim_start();
                    // The only content after `- ` is a comment
                    content.starts_with('#')
                };
                if is_comment_only {
                    self.find_trailing_comment(marker_line).map(|c| c.text)
                } else {
                    None
                }
            } else {
                None
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

            // For block scalar values, use the actual body end line instead of
            // the span end (which may extend past trailing blank lines).
            let content_end = if let Some(body_end_0idx) = self.last_block_scalar_body_end.take() {
                body_end_0idx + 1 // convert 0-indexed to 1-indexed
            } else {
                self.last_content_end_line()
            };

            // In flow sequences, only attach a trailing comment to the item
            // if it's the last item on its source line. Otherwise, the comment
            // belongs to a later item (e.g. `[a, b, c, # comment]` — the
            // comment belongs to `c`, not `a` or `b`).
            let trailing_comment = if flow {
                let next_on_same_line = self
                    .peek()
                    .is_some_and(|(_, s)| s.start.line() == content_end);
                if next_on_same_line {
                    None
                } else {
                    self.find_trailing_comment(content_end).map(|c| c.text)
                }
            } else {
                self.find_trailing_comment(content_end).map(|c| c.text)
            };

            let prettier_ignore = leading_comments
                .iter()
                .any(|c| c.text.trim() == "# prettier-ignore");

            items.push(SequenceItem {
                value,
                leading_comments,
                trailing_comment,
                blank_line_before,
                prettier_ignore,
                indicator_comment,
            });

            prev_end_line = content_end;
        }

        let seq_end_line = self.peek().map_or(prev_end_line, |(_, s)| s.start.line());
        // Only collect trailing comments at or deeper than the sequence's indent
        // level. Comments at a shallower level belong to the parent scope.
        let seq_col = span.start.col();
        let trailing_comments =
            self.collect_comments_between_lines_at_depth(prev_end_line, seq_end_line + 1, seq_col);

        // Capture closing bracket comment for flow sequences (e.g. `] # comment`)
        // Only capture when this is a non-nested flow sequence (in_flow_context == 1).
        // For nested flow sequences, the comment typically belongs to the parent
        // context as a trailing comment on its item.
        let closing_comment = if flow {
            if let Some((Event::SequenceEnd, s)) = self.peek() {
                let end_line = s.start.line();
                self.advance();
                if self.in_flow_context == 1 {
                    self.find_trailing_comment(end_line).map(|c| c.text)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            if let Some((Event::SequenceEnd, _)) = self.peek() {
                self.advance();
            }
            None
        };

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
            closing_comment,
        }))
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
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

        // Explicit braces are detected by a non-zero-length MappingStart span.
        // Saphyr sets start!=end when it sees `{`, but start==end for implicit mappings.
        // This reliably avoids false positives when a mapping key starts with `{`.
        let has_explicit_braces =
            span.start.index() != span.end.index() && self.is_flow_mapping_at(&span);
        let flow = has_explicit_braces || self.in_flow_context > 0;

        let flow_source = if flow && has_explicit_braces {
            self.extract_flow_source(&span)
        } else {
            None
        };

        if flow {
            self.in_flow_context += 1;
        }

        let mut entries = Vec::new();
        // If this is the document root, use the doc start line so that comments
        // between `---` and the first entry are captured as leading comments.
        let mut prev_end_line = self
            .doc_start_line_for_root
            .take()
            .unwrap_or(span.start.line());
        let mut first_key_col: Option<usize> = None;

        // Collect middle comments
        let mut middle_comments = vec![];
        if anchor.is_some() || tag_str.is_some() {
            let content_line = span.start.line();
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                let src = src.strip_prefix("- ").unwrap_or(src);
                if src.starts_with('#') {
                    continue;
                }
                if src.starts_with('!') || src.starts_with('&') {
                    props_line = l;
                    break;
                }
                if let Some(tag) = &tag_str
                    && src.contains(tag.as_str())
                {
                    props_line = l;
                    break;
                }
                if let Some(anc) = &anchor
                    && src.contains(&format!("&{anc}"))
                {
                    props_line = l;
                    break;
                }
                break;
            }

            if let Some(comment) = self.find_trailing_comment(props_line) {
                middle_comments.push(comment);
            }
            if props_line < content_line {
                let standalone = self.collect_comments_between_lines(props_line, content_line);
                middle_comments.extend(standalone);
            }
            // Only capture trailing comment on content_line as a middle comment
            // when the tag/anchor is on the SAME line. When they're on different
            // lines, any trailing comment on content_line belongs to the first
            // entry, not to the mapping's tag-to-content gap.
            if middle_comments.is_empty()
                && props_line == content_line
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
            let blank_line_before = if entries.is_empty() {
                // For the first entry, only preserve blank lines between
                // leading comments and the key (not before the first comment).
                if let Some(last_comment) = leading_comments.last() {
                    self.has_blank_line_between(last_comment.line, key_start_line)
                } else {
                    false
                }
            } else if let Some(last_comment) = leading_comments.last() {
                self.has_blank_line_between(last_comment.line, key_start_line)
            } else {
                self.has_blank_line_immediately_before(key_start_line)
            };

            let mut has_prettier_ignore = leading_comments
                .iter()
                .any(|c| c.text.trim() == "# prettier-ignore");

            let is_explicit_key = self.check_explicit_key(key_start_line, key_start_col);

            // When explicit key detected and `?` is on a separate line from the key,
            // capture any trailing comment on the `?` line (e.g. `? # comment\n  key`).
            let question_mark_comment = if is_explicit_key {
                self.find_question_mark_comment(key_start_line)
            } else {
                None
            };

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

            let key_end_line = self.last_event_end_line();
            let value_start_line = self.peek().map_or(key_end_line, |(_, s)| s.start.line());

            // For explicit keys, find the colon (`:`) indicator line between
            // key_end and value_start. The `:` line should not be treated as key
            // trailing — its comment is a `colon_comment` instead.
            let explicit_colon_line = if is_explicit_key && value_start_line > key_end_line {
                (key_end_line..=value_start_line).find(|&line_1| {
                    let idx = line_1.saturating_sub(1);
                    idx < self.source_lines.len()
                        && self.source_lines[idx].trim_start().starts_with(':')
                })
            } else {
                None
            };

            let key_trailing_comment = match value_start_line.cmp(&key_end_line) {
                core::cmp::Ordering::Greater => {
                    // For explicit keys, skip the colon line when looking for
                    // key trailing comments.
                    if explicit_colon_line == Some(key_end_line) {
                        None
                    } else {
                        self.find_trailing_comment(key_end_line).map(|c| c.text)
                    }
                }
                core::cmp::Ordering::Equal => {
                    // Same line — collect trailing comment only when value is a
                    // flow collection with NO items on the bracket line.
                    // e.g. `key: [    # comment`  → capture as key trailing
                    // but  `key: [ item, # comment` → leave for flow item
                    let is_empty_flow_bracket = self.peek().is_some_and(|(event, span)| {
                        let is_flow = (matches!(event, Event::SequenceStart(..))
                            && self.is_flow_sequence_at(span))
                            || (matches!(event, Event::MappingStart(..))
                                && self.is_flow_mapping_at(span));
                        if !is_flow {
                            return false;
                        }
                        // Check source: is there non-whitespace between the
                        // opening bracket and the `#` comment on this line?
                        let line_idx = key_end_line.saturating_sub(1);
                        if line_idx >= self.source_lines.len() {
                            return false;
                        }
                        let line = self.source_lines[line_idx];
                        let bracket_byte = self.to_byte(span.start.index());
                        let line_start =
                            self.source[..bracket_byte].rfind('\n').map_or(0, |p| p + 1);
                        let bracket_col = bracket_byte - line_start;
                        // Text after the bracket up to a `#` comment
                        if bracket_col + 1 < line.len() {
                            let after_bracket = &line[bracket_col + 1..];
                            let before_comment = after_bracket
                                .find('#')
                                .map_or(after_bracket, |p| &after_bracket[..p]);
                            before_comment.trim().is_empty()
                        } else {
                            true
                        }
                    });
                    if is_empty_flow_bracket {
                        self.find_trailing_comment(key_end_line).map(|c| c.text)
                    } else {
                        None
                    }
                }
                core::cmp::Ordering::Less => None,
            };
            // Check if the value is an implicit null. Saphyr reports implicit
            // null positions at the NEXT token, which can be far away (e.g. a
            // sibling entry in the parent mapping). Don't collect between_comments
            // in that case, as they may belong to outer/sibling entries.
            let value_is_implicit_null = if let Some((Event::Scalar(v, style, ..), s)) = self.peek()
            {
                *style == ScalarStyle::Plain
                    && (v == "~" || v.is_empty())
                    && self.is_implicit_null(s, v, *style)
            } else {
                false
            };
            let between_comments = if value_is_implicit_null {
                vec![]
            } else {
                self.collect_comments_between_lines(key_end_line, value_start_line)
            };

            // Also check between_comments for prettier-ignore
            if !has_prettier_ignore {
                has_prettier_ignore = between_comments
                    .iter()
                    .any(|c| c.text.trim() == "# prettier-ignore");
            }

            // For explicit key entries, look for a trailing comment on the colon
            // (`:`) line. E.g. `? key\n: # comment\n  value`. This is stored as
            // `colon_comment` (NOT between_comments) so it doesn't force explicit
            // key format. The printer renders it on its own line between key and value.
            let colon_comment = if let Some(colon_line) = explicit_colon_line {
                self.find_trailing_comment(colon_line)
            } else {
                None
            };

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
            // inject the comment into the value's middle_comments.
            // Exception: block scalars (Literal/Folded) — the comment is a header comment
            // that's already part of block_source.
            let is_block_scalar = matches!(
                &value,
                Node::Scalar(s) if matches!(s.style, ScalarStyle::Literal | ScalarStyle::Folded)
            );
            let key_trailing_comment =
                if key_trailing_comment.is_some() && has_node_props(&value) && !is_block_scalar {
                    let comment_obj = Comment {
                        text: key_trailing_comment.clone().unwrap_or_default(),
                        col: 0,
                        line: 0,
                        blank_line_before: false,
                    };
                    match &mut value {
                        Node::Mapping(m) => {
                            m.middle_comments.insert(0, comment_obj);
                        }
                        Node::Sequence(s) => {
                            s.middle_comments.insert(0, comment_obj);
                        }
                        Node::Scalar(s) => {
                            s.middle_comments.insert(0, comment_obj);
                        }
                        Node::Alias(_) => {}
                    }
                    None
                } else {
                    key_trailing_comment
                };

            // Also move between_comments to the value's middle_comments when the
            // value has props (anchor/tag). These are comments between the props
            // and the first entry of the value collection (e.g. `key: &anchor\n\n
            // # comment\n  subkey: val`). Prettier renders them inline with the
            // anchor/tag as middle_comments.
            let between_comments =
                if !between_comments.is_empty() && has_node_props(&value) && !is_block_scalar {
                    match &mut value {
                        Node::Mapping(m) => {
                            m.middle_comments.extend(between_comments);
                        }
                        Node::Sequence(s) => {
                            s.middle_comments.extend(between_comments);
                        }
                        Node::Scalar(s) => {
                            s.middle_comments.extend(between_comments);
                        }
                        Node::Alias(_) => {}
                    }
                    vec![]
                } else {
                    between_comments
                };

            // For block scalar values, use the actual body end line instead of
            // the span end (which may extend past trailing blank lines).
            let content_end = if let Some(body_end_0idx) = self.last_block_scalar_body_end.take() {
                body_end_0idx + 1 // convert 0-indexed to 1-indexed
            } else {
                self.last_content_end_line()
            };
            let trailing_comment = self.find_trailing_comment(content_end).map(|c| c.text);

            // Capture raw source for prettier-ignore entries (only when the
            // comment is in leading_comments, i.e. before the key). When
            // prettier-ignore is in between_comments, the VALUE is preserved
            // raw by the printer, not the whole entry.
            let leading_prettier_ignore = leading_comments
                .iter()
                .any(|c| c.text.trim() == "# prettier-ignore");
            let raw_source = if leading_prettier_ignore {
                let start_idx = key_start_line.saturating_sub(1);
                let end_idx = content_end.min(self.source_lines.len());
                if start_idx < end_idx {
                    let lines = &self.source_lines[start_idx..end_idx];
                    let min_indent = lines
                        .iter()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.len() - l.trim_start().len())
                        .min()
                        .unwrap_or(0);
                    let stripped: Vec<&str> = lines
                        .iter()
                        .map(|l| {
                            if l.len() > min_indent {
                                &l[min_indent..]
                            } else {
                                l.trim()
                            }
                        })
                        .collect();
                    Some(stripped.join("\n"))
                } else {
                    None
                }
            } else {
                None
            };

            // Check if there's a blank line between the last between_comment
            // (or key) and the value. This preserves blank lines before values
            // in cases like `obj:\n  # comment\n\n  key: value`.
            let blank_line_before_value = if !between_comments.is_empty() {
                let last_comment_line = between_comments
                    .last()
                    .expect("between_comments non-empty")
                    .line;
                self.has_blank_line_between(last_comment_line, value_start_line)
            } else if key_trailing_comment.is_some() || colon_comment.is_some() {
                self.has_blank_line_between(key_end_line, value_start_line)
            } else {
                false
            };

            entries.push(MappingEntry {
                key,
                value,
                leading_comments,
                key_trailing_comment,
                between_comments,
                blank_line_before_value,
                colon_comment,
                trailing_comment,
                blank_line_before,
                is_explicit_key,
                question_mark_comment,
                raw_source,
            });

            prev_end_line = content_end;
        }

        let mapping_end_line = if flow {
            // For flow mappings, saphyr reports MappingEnd at the last value's
            // position, not the closing `}`. Compute from flow_source if available.
            if let Some(ref fs) = flow_source {
                span.start.line() + fs.chars().filter(|&c| c == '\n').count()
            } else {
                self.peek().map_or(prev_end_line, |(_, s)| s.start.line())
            }
        } else {
            self.peek().map_or(prev_end_line, |(_, s)| s.start.line())
        };
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
            has_explicit_braces,
            middle_comments,
            trailing_comments,
        }))
    }

    fn build_alias(&mut self) -> Result<Node> {
        let (event, span) = self.advance();
        let span = *span;

        if let Event::Alias(_id) = event {
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

    fn last_content_end_line(&self) -> usize {
        if self.pos < 2 {
            return self.last_event_end_line();
        }
        // Walk backwards through container end events (MappingEnd, SequenceEnd)
        // to find the actual last content event. Container end events in saphyr
        // report the position of the NEXT sibling, not the actual content end.
        //
        // Also skip zero-length null scalars, but ONLY when immediately preceded
        // by a container end in the walk. These are trailing null values (e.g. in
        // !!set mappings) whose positions come from the NEXT token.
        let mut idx = self.pos - 1;
        let mut after_container_end = false;
        loop {
            let event = &self.events[idx];
            match &event.0 {
                Event::MappingEnd | Event::SequenceEnd => {
                    after_container_end = true;
                    if idx == 0 {
                        return event.1.end.line();
                    }
                    idx -= 1;
                }
                Event::Scalar(v, ..)
                    if after_container_end
                        && (v == "~" || v.is_empty())
                        && event.1.start == event.1.end =>
                {
                    // Zero-length null scalar right after container end — skip it.
                    after_container_end = false;
                    if idx == 0 {
                        return event.1.end.line();
                    }
                    idx -= 1;
                }
                _ => {
                    let line = event.1.end.line();
                    let col = event.1.end.col();
                    let start_line = event.1.start.line();
                    // When the span ends at column 0, the content actually finished on the
                    // previous line. Adjust to avoid picking up comments on the next line.
                    let result = if col == 0 && line > 1 && start_line < line {
                        line - 1
                    } else {
                        line
                    };
                    return result;
                }
            }
        }
    }

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

    fn extract_tag_before(&self, span: &Span) -> Option<String> {
        let start = self.to_byte(span.start.index());
        let search_start = start.saturating_sub(300);
        let region = &self.source[search_start..start];

        let bytes = region.as_bytes();
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            if bytes[i] == b'!' {
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

                let tag_start = search_start + i;
                let rest = &self.source[tag_start..start];

                let tag_end_offset = rest
                    .find(|c: char| c.is_whitespace() || c == '{' || c == '[')
                    .unwrap_or(rest.len());
                let tag_text = rest[..tag_end_offset].trim_end();
                if !tag_text.is_empty()
                    && tag_text.starts_with('!')
                    && (i == 0
                        || bytes[i - 1].is_ascii_whitespace()
                        || bytes[i - 1] == b'-'
                        || bytes[i - 1] == b':'
                        || bytes[i - 1] == b',')
                {
                    return Some(tag_text.to_string());
                }
            }
        }
        None
    }

    fn extract_anchor_before(&self, span: &Span) -> Option<String> {
        let start = self.to_byte(span.start.index());
        let search_start = start.saturating_sub(200);
        let region = &self.source[search_start..start];

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

        if let Some(rest) = region.strip_prefix('*') {
            rest.chars().take_while(|c| is_anchor_char(*c)).collect()
        } else if let Some(star_pos) = region.find('*') {
            region[star_pos + 1..]
                .chars()
                .take_while(|c| is_anchor_char(*c))
                .collect()
        } else {
            String::from("unknown")
        }
    }

    fn extract_flow_source(&self, span: &Span) -> Option<String> {
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

    /// Returns (`source_text`, `indicator_line_0idx`, `body_end_line_0idx_inclusive`).
    #[allow(clippy::too_many_lines)]
    fn extract_block_scalar_source(
        &self,
        span: &Span,
        style: ScalarStyle,
    ) -> (String, Option<usize>, Option<usize>) {
        let start_line = span.start.line();
        let indicator_char = match style {
            ScalarStyle::Literal => '|',
            ScalarStyle::Folded => '>',
            _ => return (String::new(), None, None),
        };

        let mut indicator_line_idx = None;
        let mut indicator_char_pos = 0;

        let span_0idx = start_line.saturating_sub(1); // 0-indexed span start line
        let span_col = span.start.col();
        for i in 0..4 {
            let check_idx = start_line.saturating_sub(1).saturating_sub(i);
            if check_idx < self.source_lines.len() {
                let line = self.source_lines[check_idx];
                let bytes = line.as_bytes();
                let mut found = false;
                let mut search_from = bytes.len();
                while search_from > 0 {
                    let region = &line[..search_from];
                    let Some(pos) = region.rfind(indicator_char) else {
                        break;
                    };
                    search_from = pos;

                    let after = line[pos + 1..].trim();
                    let valid_after = after.is_empty()
                        || after.starts_with('+')
                        || after.starts_with('-')
                        || after.starts_with(|c: char| c.is_ascii_digit())
                        || after.starts_with('#');
                    if !valid_after {
                        continue;
                    }

                    let valid_before = if pos == 0 {
                        true
                    } else {
                        let prev = bytes[pos - 1];
                        prev == b' ' || prev == b'\t' || prev == b':' || prev == b'-'
                    };
                    let first_non_space = line.find(|c: char| !c.is_whitespace());
                    let is_first_content = first_non_space == Some(pos);

                    if valid_before || is_first_content {
                        // Validate: if the indicator is on the same line as
                        // span.start but span.start.col() is 0 and the
                        // indicator is not at position 0, this is likely a
                        // different block scalar (empty block scalar case
                        // where saphyr places span at the next mapping key).
                        if check_idx == span_0idx && span_col == 0 && pos > 0 {
                            // Skip — this indicator belongs to a different entry
                            break;
                        }
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
            return (String::new(), None, None);
        };

        let indicator_line = self.source_lines[indicator_line_idx];
        let indicator_pos = indicator_char_pos;

        let header_region = &indicator_line[indicator_pos..];
        let header = header_region.trim_end();

        let body_start_line = indicator_line_idx + 1;
        let indicator_line_indent = indicator_line.len() - indicator_line.trim_start().len();
        let has_keep = header.contains('+');

        // Parse explicit indent indicator from header (e.g. |2, >1+)
        let explicit_indent: Option<usize> = header
            .chars()
            .skip(1)
            .find(char::is_ascii_digit)
            .and_then(|c| c.to_digit(10).map(|d| d as usize));

        // Determine the content indent: explicit if given, else auto-detect
        // from the first non-empty body line.
        // Use saphyr's span.end to bound the body scan (prevents over-including
        // lines from subsequent entries/documents).
        let span_end_0idx = span.end.line().saturating_sub(1);
        let content_indent = if let Some(n) = explicit_indent {
            let computed = indicator_line_indent + n;
            // When the tag is on a separate line from the indicator (e.g.
            //   folded:\n   !foo\n  >1\n value), indicator_line_indent
            // may not match the parent's indent level. Auto-detect from
            // body lines and use the minimum of computed and detected.
            let mut detected = None;
            for i in body_start_line..self.source_lines.len() {
                if i > span_end_0idx {
                    break;
                }
                let line = self.source_lines[i];
                if !line.trim().is_empty() {
                    detected = Some(line.len() - line.trim_start().len());
                    break;
                }
            }
            detected.map_or(computed, |d| d.min(computed))
        } else {
            let has_body = span.start.index() < span.end.index();
            let mut detected = None;
            for i in body_start_line..self.source_lines.len() {
                if i > span_end_0idx {
                    break;
                }
                let line = self.source_lines[i];
                if !line.trim().is_empty() {
                    let indent = line.len() - line.trim_start().len();
                    if indent > indicator_line_indent {
                        detected = Some(indent);
                    } else if indent == indicator_line_indent && has_body {
                        // First body line has the same indent as the indicator
                        // line. Normally this means it's not body content, but
                        // if saphyr's span says the scalar is non-empty, it IS
                        // content (e.g., root-level block scalar with zero
                        // indent like `--- >\nline1\nline2`).
                        detected = Some(indent);
                    }
                    break;
                }
            }
            detected.unwrap_or(indicator_line_indent + 1)
        };

        let mut last_content_line: Option<usize> = None;
        let mut last_candidate_line: Option<usize> = None;

        for i in body_start_line..self.source_lines.len() {
            // Don't scan past saphyr's span end (prevents including lines
            // from subsequent documents/entries when content_indent is 0).
            if i > span_end_0idx {
                break;
            }
            let line = self.source_lines[i];
            if line.trim().is_empty() {
                last_candidate_line = Some(i);
            } else {
                let line_indent = line.len() - line.trim_start().len();
                if line_indent >= content_indent {
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

        (
            format!("{}\n{}", header, body_lines.join("\n")),
            Some(indicator_line_idx),
            body_end_line,
        )
    }

    fn extract_quoted_source(&self, span: &Span, style: ScalarStyle) -> Option<String> {
        let quote_char: u8 = match style {
            ScalarStyle::DoubleQuoted => b'"',
            ScalarStyle::SingleQuoted => b'\'',
            _ => return None,
        };

        let start_byte = self.to_byte(span.start.index());

        // Saphyr's span.start may point to:
        // 1. The opening quote itself (span includes delimiter)
        // 2. The first content character (span excludes delimiter)
        // Check start_byte first to avoid finding the closing quote of a previous string.
        let mut open_pos = None;
        if start_byte < self.source.len() && self.source.as_bytes()[start_byte] == quote_char {
            open_pos = Some(start_byte);
        } else if start_byte == 0 {
            // Already checked above
        } else {
            for i in (0..start_byte).rev() {
                if self.source.as_bytes()[i] == quote_char {
                    open_pos = Some(i);
                    break;
                }
                if start_byte - i > 5 {
                    break;
                }
            }
        }

        let open_pos = open_pos?;

        let content_start = open_pos + 1;
        let bytes = self.source.as_bytes();
        let mut i = content_start;
        while i < bytes.len() {
            if style == ScalarStyle::DoubleQuoted {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'"' {
                    return Some(self.source[content_start..i].to_string());
                } else {
                    i += 1;
                }
            } else if bytes[i] == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                } else {
                    return Some(self.source[content_start..i].to_string());
                }
            } else {
                i += 1;
            }
        }

        None
    }

    fn is_implicit_null(&self, span: &Span, value: &str, style: ScalarStyle) -> bool {
        if style != ScalarStyle::Plain {
            return false;
        }
        if value == "~" || value.is_empty() {
            if span.is_empty() {
                return true;
            }
            let start = self.to_byte(span.start.index());
            let end = self.to_byte(span.end.index());
            if start >= self.source.len() {
                return true;
            }
            let text = self.source[start..end].trim();
            !text.contains('~')
        } else {
            false
        }
    }

    /// Find a trailing comment on a document marker line (`---` or `...`).
    /// Returns the comment text (e.g. `# Suffix`) if found.
    fn find_doc_marker_comment(&mut self, line: usize, marker: &str) -> Option<String> {
        if line == 0 || line > self.source_lines.len() {
            return None;
        }
        let src = self.source_lines[line - 1].trim();
        if !src.starts_with(marker) {
            return None;
        }
        let after_marker = src[marker.len()..].trim_start();
        if after_marker.starts_with('#') {
            // Mark the comment as used to prevent duplication
            for (ci, comment) in self.comments.iter().enumerate() {
                if comment.line == line && !self.used_comment_lines[ci] {
                    self.used_comment_lines[ci] = true;
                    break;
                }
            }
            Some(after_marker.to_string())
        } else {
            None
        }
    }

    fn check_explicit_doc_end(&self, span: &Span) -> bool {
        let line = span.start.line();
        if line == 0 || line > self.source_lines.len() {
            return false;
        }
        let content = self.source_lines[line - 1].trim();
        content == "..." || content.starts_with("... ") || content.starts_with("...#")
    }

    /// Find a trailing comment on the `?` indicator line when the key is on a different line.
    /// Returns None if `?` and key are on the same line.
    fn find_question_mark_comment(&mut self, key_start_line: usize) -> Option<String> {
        // Search backward from key_start_line to find the `?` line
        for prev in (1..key_start_line).rev() {
            let prev_idx = prev - 1;
            if prev_idx >= self.source_lines.len() {
                break;
            }
            let prev_content = self.source_lines[prev_idx].trim();
            if prev_content.is_empty() || prev_content.starts_with('#') {
                continue;
            }
            // Found the `?` line
            if prev_content == "?"
                || prev_content.starts_with("? ")
                || prev_content.starts_with("?#")
            {
                // Get any trailing (inline) comment on this line
                return self.find_trailing_comment(prev).map(|c| c.text);
            }
            break;
        }
        None
    }

    fn check_explicit_key(&self, line: usize, col: usize) -> bool {
        if line == 0 || line > self.source_lines.len() {
            return false;
        }
        let src_line = self.source_lines[line - 1];
        let content = src_line.trim_start();
        if content.starts_with("? ") || content == "?" {
            return true;
        }
        if col >= 2 {
            let before = &src_line[..col];
            let trimmed = before.trim_end();
            if trimmed.ends_with('?') {
                return true;
            }
        }
        // Check previous lines for a standalone `?` (key on separate line from `?`)
        for prev in (1..line).rev() {
            let prev_idx = prev - 1;
            if prev_idx >= self.source_lines.len() {
                break;
            }
            let prev_content = self.source_lines[prev_idx].trim();
            if prev_content.is_empty() || prev_content.starts_with('#') {
                continue; // skip blank lines and comments
            }
            // Only check lines at same or shallower indent than the key.
            // Deeper-indented `?` lines belong to nested mappings.
            let prev_indent =
                self.source_lines[prev_idx].len() - self.source_lines[prev_idx].trim_start().len();
            if prev_indent > col {
                break;
            }
            if prev_content == "?"
                || prev_content.starts_with("? ")
                || prev_content.starts_with("?#")
            {
                return true;
            }
            break; // non-comment, non-blank, non-? line found — stop looking
        }
        false
    }

    fn has_blank_line_immediately_before(&self, line: usize) -> bool {
        if line < 2 {
            return false;
        }
        let idx = line - 2;
        if idx >= self.source_lines.len() {
            return false;
        }
        self.source_lines[idx].trim().is_empty()
    }

    #[allow(dead_code)]
    fn line_preceded_by_blank(&self, line: usize) -> bool {
        let mut check = line.saturating_sub(1);
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
                check -= 1;
                continue;
            }
            break;
        }
        false
    }

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

    fn collect_preamble_between_lines(
        &mut self,
        after_line: usize,
        before_line: usize,
    ) -> Vec<String> {
        let mut result = Vec::new();
        for i in 0..self.source_lines.len() {
            let line_num = i + 1;
            if line_num >= before_line {
                break;
            }
            if line_num <= after_line {
                continue;
            }
            let trimmed = self.source_lines[i].trim();
            if trimmed.starts_with('%') {
                // Normalize internal whitespace in directives
                // (e.g. `%FOO  bar` → `%FOO bar`)
                let normalized: String = {
                    let mut parts = trimmed.splitn(2, '#');
                    let directive_part = parts.next().unwrap_or("");
                    let comment_part = parts.next();
                    let words: Vec<&str> = directive_part.split_whitespace().collect();
                    let mut d = words.join(" ");
                    if let Some(comment) = comment_part {
                        d.push_str(" #");
                        d.push_str(comment);
                    }
                    d
                };
                result.push(normalized);
            } else if trimmed.starts_with('#') {
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

    /// Mark any inline comments on the block scalar header line as used,
    /// preventing them from being duplicated as leading comments of subsequent entries.
    fn mark_block_scalar_header_comments(&mut self, span: &Span, style: ScalarStyle) {
        let indicator_char = match style {
            ScalarStyle::Literal => '|',
            ScalarStyle::Folded => '>',
            _ => return,
        };
        // Find the indicator line by looking backwards from the span start.
        // The indicator line is the one with the block scalar header (| or >)
        // preceded by valid context (space, colon, dash, or at line start).
        let start_line = span.start.line();
        for offset in 0..4 {
            let line_num = start_line.saturating_sub(offset);
            if line_num == 0 {
                break;
            }
            let idx = line_num - 1;
            if idx >= self.source_lines.len() {
                continue;
            }
            let line = self.source_lines[idx];
            // Check if this line has a valid block scalar indicator
            let has_indicator = line.bytes().enumerate().any(|(pos, b)| {
                if b != indicator_char as u8 {
                    return false;
                }
                let valid_before =
                    pos == 0 || matches!(line.as_bytes()[pos - 1], b' ' | b'\t' | b':' | b'-');
                let first_non_space = line.find(|c: char| !c.is_whitespace());
                let is_first = first_non_space == Some(pos);
                if !valid_before && !is_first {
                    return false;
                }
                let after = line[pos + 1..].trim();
                after.is_empty()
                    || after.starts_with('+')
                    || after.starts_with('-')
                    || after.starts_with(|c: char| c.is_ascii_digit())
                    || after.starts_with('#')
            });
            if has_indicator {
                // Mark any non-whole-line comment on this line as used
                for (i, comment) in self.comments.iter().enumerate() {
                    if comment.line == line_num
                        && !comment.whole_line
                        && !self.used_comment_lines[i]
                    {
                        self.used_comment_lines[i] = true;
                    }
                }
                break;
            }
        }
    }

    /// Check if saphyr skipped an empty document between `...` markers.
    /// For input like `...\n# comment\n...\n`, saphyr produces only one `DocumentEnd`
    /// but prettier treats this as two documents. Returns a synthetic empty document
    /// if a `...` marker is found between the previous document end and the next document start.
    fn check_skipped_empty_document(&mut self, prev_doc_end_line: usize) -> Option<YamlDoc> {
        let next_doc_start_line = self.peek().map(|(_, s)| s.start.line())?;

        // Look for a `...` marker between the previous doc end and next doc start
        let mut doc_end_marker_line = None;
        for line_idx in prev_doc_end_line..next_doc_start_line {
            if line_idx < self.source_lines.len() {
                let trimmed = self.source_lines[line_idx].trim();
                if trimmed == "..." || trimmed.starts_with("... ") || trimmed.starts_with("...#") {
                    doc_end_marker_line = Some(line_idx + 1); // 1-indexed
                }
            }
        }

        let end_line = doc_end_marker_line?;

        // Collect comments between prev_doc_end and the `...` marker
        let preamble_comments: Vec<String> = self
            .collect_comments_between_lines(prev_doc_end_line, end_line)
            .into_iter()
            .map(|c| c.text)
            .collect();

        if preamble_comments.is_empty() {
            return None;
        }

        Some(YamlDoc {
            explicit_start: false,
            explicit_end: true,
            preamble: preamble_comments,
            root: None,
            end_comments: vec![],
            start_comment: None,
            end_marker_comment: None,
            body_leading_comments: vec![],
            root_trailing_comment: None,
            raw_body_source: None,
        })
    }

    /// Get the column where a non-whole-line comment starts on the given line, if any.
    fn comment_start_col_on_line(&self, line: usize) -> Option<usize> {
        for (i, comment) in self.comments.iter().enumerate() {
            if !self.used_comment_lines[i] && !comment.whole_line && comment.line == line {
                return Some(comment.col);
            }
        }
        None
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

/// Check if a node has properties (anchor, tag).
/// Duplicated here to avoid circular dependency with utilities module during AST construction.
fn has_node_props(node: &Node) -> bool {
    match node {
        Node::Mapping(m) => m.anchor.is_some() || m.tag.is_some(),
        Node::Sequence(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Scalar(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Alias(_) => false,
    }
}
