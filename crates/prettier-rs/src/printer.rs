use crate::options::{EndOfLine, PrettierOptions};

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
}

/// Print a document to a string using the Wadler-Lindig algorithm.
pub fn print(doc: &Doc, options: &PrettierOptions) -> String {
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
    // Stack of (indent_level, mode, doc)
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![(0, Mode::Break, doc)];
    let mut pos: usize = 0; // current column position

    while let Some((indent, mode, doc)) = stack.pop() {
        match doc {
            Doc::Text(s) => {
                output.push_str(s);
                pos += s.len();
            }
            Doc::Concat(docs) => {
                // Push in reverse order so first doc is processed first
                for d in docs.iter().rev() {
                    stack.push((indent, mode, d));
                }
            }
            Doc::Group(inner) => {
                // Try flat mode: measure if it fits
                if fits(
                    inner,
                    options.print_width.saturating_sub(pos),
                    indent,
                    &indent_str,
                ) {
                    stack.push((indent, Mode::Flat, inner));
                } else {
                    stack.push((indent, Mode::Break, inner));
                }
            }
            Doc::Indent(inner) => {
                stack.push((indent + 1, mode, inner));
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
                Mode::Flat => stack.push((indent, mode, flat)),
                Mode::Break => stack.push((indent, mode, broken)),
            },
        }
    }

    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// Check if a document fits within the remaining width when printed flat.
fn fits(doc: &Doc, remaining: usize, indent: usize, _indent_str: &str) -> bool {
    #[allow(clippy::cast_possible_wrap)]
    let mut rem = remaining as isize;
    let mut stack: Vec<(usize, &Doc)> = vec![(indent, doc)];

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
            Doc::Hardline => {
                // hardline means it definitely breaks
                return true;
            }
            Doc::Softline => {
                // empty in flat mode
            }
            Doc::IfBreak { flat, .. } => {
                stack.push((ind, flat));
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
        let result = print(&doc, &PrettierOptions::default());
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
        let result = print(&doc, &PrettierOptions::default());
        assert_eq!(result, "[1, 2]");
    }

    #[test]
    fn group_breaks_when_too_wide() {
        let opts = PrettierOptions {
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
        let result = print(&doc, &PrettierOptions::default());
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn crlf_line_endings() {
        let opts = PrettierOptions {
            end_of_line: EndOfLine::Crlf,
            ..Default::default()
        };
        let doc = Doc::concat(vec![Doc::text("a"), Doc::Hardline, Doc::text("b")]);
        let result = print(&doc, &opts);
        assert_eq!(result, "a\r\nb");
    }

    #[test]
    fn tabs_indentation() {
        let opts = PrettierOptions {
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
}
