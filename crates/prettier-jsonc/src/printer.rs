use crate::PrettierConfig;
use prettier_config::EndOfLine;

/// Document IR for the pretty-printing algorithm.
#[derive(Debug, Clone)]
pub enum Doc {
    /// Literal text (no newlines).
    Text(String),
    /// Concatenation of documents.
    Concat(Vec<Doc>),
    /// Try to print flat; if it exceeds `print_width`, break.
    Group(Box<Doc>),
    /// Increase indent level for the inner document.
    Indent(Box<Doc>),
    /// Space when flat, newline+indent when broken.
    Line,
    /// Always a newline.
    Hardline,
    /// Empty when flat, newline+indent when broken.
    Softline,
    /// Choose between flat and broken variants.
    IfBreak { flat: Box<Doc>, broken: Box<Doc> },
    /// Force the enclosing group to break (no-op in printing, causes `fits()` to return false).
    BreakParent,
    /// Fill: alternating [content, separator, content, separator, ..., content].
    /// Packs content items onto each line, breaking separators as needed.
    Fill(Vec<Doc>),
}

impl Doc {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    pub fn concat(docs: Vec<Doc>) -> Self {
        Self::Concat(docs)
    }

    pub fn group(doc: Doc) -> Self {
        Self::Group(Box::new(doc))
    }

    pub fn indent(doc: Doc) -> Self {
        Self::Indent(Box::new(doc))
    }

    pub fn if_break(flat: Doc, broken: Doc) -> Self {
        Self::IfBreak {
            flat: Box::new(flat),
            broken: Box::new(broken),
        }
    }

    pub fn fill(parts: Vec<Doc>) -> Self {
        Self::Fill(parts)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// Stack command for the printer.
enum Cmd<'a> {
    /// Print a doc with a given indent and mode.
    Print(usize, Mode, &'a Doc),
    /// Continue processing a fill from the given offset.
    FillParts(usize, &'a [Doc], usize),
}

/// Print a document to a string using the Wadler-Lindig algorithm.
#[allow(clippy::too_many_lines)]
pub fn print(doc: &Doc, options: &PrettierConfig) -> String {
    let eol = match options.end_of_line {
        EndOfLine::Lf | EndOfLine::Auto => "\n",
        EndOfLine::Crlf => "\r\n",
        EndOfLine::Cr => "\r",
    };

    let indent_str = if options.use_tabs {
        "\t".to_string()
    } else {
        " ".repeat(options.tab_width)
    };

    let mut output = String::new();
    let mut stack: Vec<Cmd> = vec![Cmd::Print(0, Mode::Break, doc)];
    let mut pos: usize = 0; // current column position

    while let Some(cmd) = stack.pop() {
        match cmd {
            Cmd::Print(indent, mode, doc) => match doc {
                Doc::Text(s) => {
                    output.push_str(s);
                    pos += s.len();
                }
                Doc::Concat(docs) => {
                    for d in docs.iter().rev() {
                        stack.push(Cmd::Print(indent, mode, d));
                    }
                }
                Doc::Group(inner) => {
                    if fits(inner, options.print_width.saturating_sub(pos), indent) {
                        stack.push(Cmd::Print(indent, Mode::Flat, inner));
                    } else {
                        stack.push(Cmd::Print(indent, Mode::Break, inner));
                    }
                }
                Doc::Indent(inner) => {
                    stack.push(Cmd::Print(indent + 1, mode, inner));
                }
                Doc::Line => match mode {
                    Mode::Flat => {
                        output.push(' ');
                        pos += 1;
                    }
                    Mode::Break => {
                        output.push_str(eol);
                        let indent_text = indent_str.repeat(indent);
                        output.push_str(&indent_text);
                        pos = indent_text.len();
                    }
                },
                Doc::Hardline => {
                    output.push_str(eol);
                    let indent_text = indent_str.repeat(indent);
                    output.push_str(&indent_text);
                    pos = indent_text.len();
                }
                Doc::Softline => match mode {
                    Mode::Flat => {
                        // empty
                    }
                    Mode::Break => {
                        output.push_str(eol);
                        let indent_text = indent_str.repeat(indent);
                        output.push_str(&indent_text);
                        pos = indent_text.len();
                    }
                },
                Doc::IfBreak { flat, broken } => match mode {
                    Mode::Flat => stack.push(Cmd::Print(indent, mode, flat)),
                    Mode::Break => stack.push(Cmd::Print(indent, mode, broken)),
                },
                Doc::BreakParent => {
                    // No-op in printing — it only affects fits() measurement
                }
                Doc::Fill(parts) => {
                    if !parts.is_empty() {
                        stack.push(Cmd::FillParts(indent, parts, 0));
                    }
                }
            },
            Cmd::FillParts(indent, parts, offset) => {
                let remaining = parts.len() - offset;
                if remaining == 0 {
                    continue;
                }

                let content = &parts[offset];
                let rem_width = options.print_width.saturating_sub(pos);
                let content_fits = fits(content, rem_width, indent);

                if remaining == 1 {
                    // Only content, no separator
                    let m = if content_fits {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    stack.push(Cmd::Print(indent, m, content));
                    continue;
                }

                let whitespace = &parts[offset + 1];

                if remaining == 2 {
                    // Content + separator, no next content
                    let m = if content_fits {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    stack.push(Cmd::Print(indent, m, whitespace));
                    stack.push(Cmd::Print(indent, m, content));
                    continue;
                }

                let next_content = &parts[offset + 2];

                // Check if content + whitespace + next_content fits flat
                let first_and_second_fits =
                    fits_multi(&[content, whitespace, next_content], rem_width, indent);

                // Push remaining fill (processed last — bottom of stack)
                stack.push(Cmd::FillParts(indent, parts, offset + 2));

                if first_and_second_fits {
                    // Both fit: content flat, whitespace flat
                    stack.push(Cmd::Print(indent, Mode::Flat, whitespace));
                    stack.push(Cmd::Print(indent, Mode::Flat, content));
                } else if content_fits {
                    // Only content fits: content flat, whitespace break
                    stack.push(Cmd::Print(indent, Mode::Break, whitespace));
                    stack.push(Cmd::Print(indent, Mode::Flat, content));
                } else {
                    // Neither fits: both break
                    stack.push(Cmd::Print(indent, Mode::Break, whitespace));
                    stack.push(Cmd::Print(indent, Mode::Break, content));
                }
            }
        }
    }

    output
}

/// Check if a document fits within the remaining width when printed flat.
fn fits(doc: &Doc, remaining: usize, indent: usize) -> bool {
    fits_with_stack(vec![(indent, doc)], remaining)
}

/// Check if multiple documents fit within the remaining width when printed flat.
fn fits_multi(docs: &[&Doc], remaining: usize, indent: usize) -> bool {
    fits_with_stack(docs.iter().rev().map(|d| (indent, *d)).collect(), remaining)
}

fn fits_with_stack(mut stack: Vec<(usize, &Doc)>, remaining: usize) -> bool {
    #[allow(clippy::cast_possible_wrap)]
    let mut rem = remaining as isize;

    while let Some((ind, doc)) = stack.pop() {
        if rem < 0 {
            return false;
        }
        match doc {
            Doc::Text(s) => {
                #[allow(clippy::cast_possible_wrap)]
                {
                    rem -= s.len() as isize;
                }
            }
            Doc::Concat(docs) => {
                for d in docs.iter().rev() {
                    stack.push((ind, d));
                }
            }
            Doc::Group(inner) | Doc::Indent(inner) => {
                stack.push((ind, inner));
            }
            Doc::Line => {
                rem -= 1; // space in flat mode
            }
            Doc::Hardline | Doc::BreakParent => {
                // Both force the enclosing group to break
                return false;
            }
            Doc::Softline => {
                // empty in flat mode
            }
            Doc::IfBreak { flat, .. } => {
                stack.push((ind, flat));
            }
            Doc::Fill(parts) => {
                // In fits measurement, treat fill as flat concatenation
                for d in parts.iter().rev() {
                    stack.push((ind, d));
                }
            }
        }
    }

    rem >= 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_text() {
        let doc = Doc::text("hello");
        let result = print(&doc, &PrettierConfig::default());
        assert_eq!(result, "hello");
    }

    #[test]
    fn group_fits_on_one_line() {
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(vec![
                Doc::Softline,
                Doc::text("1"),
                Doc::text(","),
                Doc::Line,
                Doc::text("2"),
            ])),
            Doc::Softline,
            Doc::text("]"),
        ]));
        let result = print(&doc, &PrettierConfig::default());
        assert_eq!(result, "[1, 2]");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        let opts = PrettierConfig {
            print_width: 10,
            ..Default::default()
        };
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(vec![
                Doc::Line,
                Doc::text("\"longvalue1\""),
                Doc::text(","),
                Doc::Line,
                Doc::text("\"longvalue2\""),
            ])),
            Doc::Line,
            Doc::text("]"),
        ]));
        let result = print(&doc, &opts);
        assert_eq!(result, "[\n  \"longvalue1\",\n  \"longvalue2\"\n]");
    }

    #[test]
    fn hardline_always_breaks() {
        let doc = Doc::concat(vec![Doc::text("a"), Doc::Hardline, Doc::text("b")]);
        let result = print(&doc, &PrettierConfig::default());
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn crlf_line_endings() {
        let opts = PrettierConfig {
            end_of_line: EndOfLine::Crlf,
            ..Default::default()
        };
        let doc = Doc::concat(vec![Doc::text("a"), Doc::Hardline, Doc::text("b")]);
        let result = print(&doc, &opts);
        assert_eq!(result, "a\r\nb");
    }

    #[test]
    fn tabs_indentation() {
        let opts = PrettierConfig {
            print_width: 10,
            use_tabs: true,
            ..Default::default()
        };
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(vec![
                Doc::Line,
                Doc::text("\"longvalue1\""),
                Doc::text(","),
                Doc::Line,
                Doc::text("\"longvalue2\""),
            ])),
            Doc::Line,
            Doc::text("]"),
        ]));
        let result = print(&doc, &opts);
        assert_eq!(result, "[\n\t\"longvalue1\",\n\t\"longvalue2\"\n]");
    }

    #[test]
    fn break_parent_forces_break() {
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("{"),
            Doc::indent(Doc::concat(vec![
                Doc::Line,
                Doc::text("a"),
                Doc::BreakParent,
            ])),
            Doc::Line,
            Doc::text("}"),
        ]));
        let result = print(&doc, &PrettierConfig::default());
        assert_eq!(result, "{\n  a\n}");
    }

    #[test]
    fn fill_packs_items_on_lines() {
        let opts = PrettierConfig {
            print_width: 20,
            ..Default::default()
        };
        // fill(["1,", line, "2,", line, "3,", line, "4,", line, "5"])
        let doc = Doc::fill(vec![
            Doc::text("1,"),
            Doc::Line,
            Doc::text("2,"),
            Doc::Line,
            Doc::text("3,"),
            Doc::Line,
            Doc::text("4,"),
            Doc::Line,
            Doc::text("5"),
        ]);
        let result = print(&doc, &opts);
        // All fit on one line: "1, 2, 3, 4, 5"
        assert_eq!(result, "1, 2, 3, 4, 5");
    }

    #[test]
    fn fill_breaks_when_needed() {
        let opts = PrettierConfig {
            print_width: 10,
            ..Default::default()
        };
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(vec![
                Doc::Softline,
                Doc::fill(vec![
                    Doc::text("1,"),
                    Doc::Line,
                    Doc::text("2,"),
                    Doc::Line,
                    Doc::text("3,"),
                    Doc::Line,
                    Doc::text("4,"),
                    Doc::Line,
                    Doc::text("5"),
                ]),
            ])),
            Doc::Softline,
            Doc::text("]"),
        ]));
        let result = print(&doc, &opts);
        // With print_width=10 and indent=2: "  1, 2," is 7 chars + "3," would be 11
        // So it should break across lines, packing as many as fit
        assert!(result.contains('\n'), "fill should break: {result}");
    }

    #[test]
    fn fill_with_hardline_separator() {
        let opts = PrettierConfig {
            print_width: 80,
            ..Default::default()
        };
        let doc = Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(vec![
                Doc::Softline,
                Doc::fill(vec![
                    Doc::text("1,"),
                    Doc::Line,
                    Doc::text("2,"),
                    Doc::Line,
                    Doc::text("3,"),
                    Doc::concat(vec![Doc::Hardline, Doc::Hardline]),
                    Doc::text("4,"),
                    Doc::Line,
                    Doc::text("5"),
                ]),
            ])),
            Doc::Softline,
            Doc::text("]"),
        ]));
        let result = print(&doc, &opts);
        // Hardline in fill forces the group to break.
        // Content before hardline packs flat, then blank line, then more packing.
        // The blank line may contain indent whitespace, so check for an empty/whitespace-only line.
        let has_blank = result.lines().any(|l| l.trim().is_empty());
        assert!(has_blank, "should have blank line: {result}");
        assert!(
            result.contains("1, 2, 3,"),
            "items before blank should pack: {result}"
        );
        assert!(
            result.contains("4, 5"),
            "items after blank should pack: {result}"
        );
    }
}
