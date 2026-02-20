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
                    break;
                }
            }
        }

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

        let preamble = self.collect_preamble_before_line(doc_start_line);
        let root = self.build_node()?;
        let content_end_line = self.last_event_end_line();

        let explicit_end = if let Some((Event::DocumentEnd, span)) = self.peek() {
            let span = *span;
            self.advance();
            self.check_explicit_doc_end(&span)
        } else {
            false
        };

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
            Some(self.extract_block_scalar_source(&span, style))
        } else {
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
                        if line_num == start_line {
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
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                if src.starts_with('#') {
                    continue;
                }
                if src.starts_with('!') || src.starts_with('&') {
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

            let content_end = self.last_content_end_line();
            let trailing_comment = self.find_trailing_comment(content_end).map(|c| c.text);

            items.push(SequenceItem {
                value,
                leading_comments,
                trailing_comment,
                blank_line_before,
            });

            prev_end_line = content_end;
        }

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
        let mut first_key_col: Option<usize> = None;

        // Collect middle comments
        let mut middle_comments = vec![];
        if anchor.is_some() || tag_str.is_some() {
            let content_line = span.start.line();
            let mut props_line = content_line;
            for l in (1..content_line).rev() {
                let src = self.source_lines[l - 1].trim_start();
                if src.starts_with('#') {
                    continue;
                }
                if src.starts_with('!') || src.starts_with('&') {
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
            let blank_line_before = if entries.is_empty() {
                false
            } else if let Some(last_comment) = leading_comments.last() {
                self.has_blank_line_between(last_comment.line, key_start_line)
            } else {
                self.has_blank_line_immediately_before(key_start_line)
            };

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

            let key_end_line = self.last_event_end_line();
            let value_start_line = self.peek().map_or(key_end_line, |(_, s)| s.start.line());
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
            // inject the comment into the value's middle_comments
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
                None
            } else {
                key_trailing_comment
            };

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

            prev_end_line = content_end;
        }

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
        let last = &self.events[self.pos - 1];
        match &last.0 {
            Event::MappingEnd | Event::SequenceEnd => {
                let prev = &self.events[self.pos - 2];
                prev.1.end.line()
            }
            _ => last.1.end.line(),
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

    fn extract_block_scalar_source(&self, span: &Span, style: ScalarStyle) -> String {
        let start_line = span.start.line();
        let indicator_char = match style {
            ScalarStyle::Literal => '|',
            ScalarStyle::Folded => '>',
            _ => return String::new(),
        };

        let mut indicator_line_idx = None;
        let mut indicator_char_pos = 0;

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

        let indicator_line = self.source_lines[indicator_line_idx];
        let indicator_pos = indicator_char_pos;

        let header_region = &indicator_line[indicator_pos..];
        let header = header_region.trim_end();

        let body_start_line = indicator_line_idx + 1;
        let indicator_line_indent = indicator_line.len() - indicator_line.trim_start().len();
        let has_keep = header.contains('+');

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

    fn extract_quoted_source(&self, span: &Span, style: ScalarStyle) -> Option<String> {
        let quote_char: u8 = match style {
            ScalarStyle::DoubleQuoted => b'"',
            ScalarStyle::SingleQuoted => b'\'',
            _ => return None,
        };

        let start_byte = self.to_byte(span.start.index());

        let mut open_pos = None;
        if start_byte == 0 {
            if !self.source.is_empty() && self.source.as_bytes()[0] == quote_char {
                open_pos = Some(0);
            }
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
            if open_pos.is_none()
                && start_byte < self.source.len()
                && self.source.as_bytes()[start_byte] == quote_char
            {
                open_pos = Some(start_byte);
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

    fn check_explicit_doc_end(&self, span: &Span) -> bool {
        let line = span.start.line();
        if line == 0 || line > self.source_lines.len() {
            return false;
        }
        let content = self.source_lines[line - 1].trim();
        content == "..."
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

    fn collect_preamble_before_line(&mut self, line: usize) -> Vec<String> {
        let mut result = Vec::new();
        for i in 0..self.source_lines.len() {
            let line_num = i + 1;
            if line_num >= line {
                break;
            }
            let trimmed = self.source_lines[i].trim();
            if trimmed.starts_with('%') {
                result.push(trimmed.to_string());
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
