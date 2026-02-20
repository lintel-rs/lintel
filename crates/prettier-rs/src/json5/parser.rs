/// JSON5 CST node types for comment-preserving formatting.
#[derive(Debug, Clone)]
pub enum Node {
    Null,
    Bool(bool),
    Number(String),
    String { value: String, quote: Quote },
    Array(Vec<ArrayElement>),
    Object(Vec<ObjectEntry>),
}

#[derive(Debug, Clone)]
pub struct ArrayElement {
    pub leading_comments: Vec<Comment>,
    pub value: Node,
    pub trailing_comment: Option<Comment>,
    pub has_trailing_comma: bool,
}

#[derive(Debug, Clone)]
pub struct ObjectEntry {
    pub leading_comments: Vec<Comment>,
    pub key: Key,
    pub value: Node,
    pub trailing_comment: Option<Comment>,
    pub has_trailing_comma: bool,
}

#[derive(Debug, Clone)]
pub enum Key {
    Identifier(String),
    String { value: String, quote: Quote },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    Single,
    Double,
}

#[derive(Debug, Clone)]
pub enum Comment {
    Line(String),
    Block(String),
}

/// Parse JSON5 source into a CST, preserving comments and formatting info.
///
/// # Errors
///
/// Returns an error string if the input is not valid JSON5.
pub fn parse(input: &str) -> Result<(Node, Vec<Comment>), String> {
    let mut parser = Parser::new(input);
    parser.skip_whitespace_and_comments();
    let node = parser.parse_value()?;
    let trailing = parser.pending_comments.drain(..).collect();
    Ok((node, trailing))
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    pending_comments: Vec<Comment>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            pending_comments: Vec::new(),
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    fn consume_char(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.advance(ch.len_utf8());
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
                self.advance(1);
            }

            if self.remaining().starts_with("//") {
                self.advance(2);
                let start = self.pos;
                while self.peek().is_some_and(|c| c != '\n') {
                    self.advance(1);
                }
                let text = self.input[start..self.pos].to_string();
                self.pending_comments.push(Comment::Line(text));
                if self.peek() == Some('\n') {
                    self.advance(1);
                }
            } else if self.remaining().starts_with("/*") {
                self.advance(2);
                let start = self.pos;
                while !self.remaining().starts_with("*/") {
                    if self.peek().is_none() {
                        break;
                    }
                    self.advance(1);
                }
                let text = self.input[start..self.pos].to_string();
                self.pending_comments.push(Comment::Block(text));
                if self.remaining().starts_with("*/") {
                    self.advance(2);
                }
            } else {
                break;
            }
        }
    }

    fn take_pending_comments(&mut self) -> Vec<Comment> {
        std::mem::take(&mut self.pending_comments)
    }

    fn parse_value(&mut self) -> Result<Node, String> {
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"' | '\'') => self.parse_string(),
            Some('n') if self.remaining().starts_with("null") => {
                self.advance(4);
                Ok(Node::Null)
            }
            Some('t') if self.remaining().starts_with("true") => {
                self.advance(4);
                Ok(Node::Bool(true))
            }
            Some('f') if self.remaining().starts_with("false") => {
                self.advance(5);
                Ok(Node::Bool(false))
            }
            Some('I') if self.remaining().starts_with("Infinity") => {
                self.advance(8);
                Ok(Node::Number("Infinity".to_string()))
            }
            Some('N') if self.remaining().starts_with("NaN") => {
                self.advance(3);
                Ok(Node::Number("NaN".to_string()))
            }
            Some(c) if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() => {
                self.parse_number()
            }
            Some(c) => Err(format!(
                "unexpected character '{c}' at position {}",
                self.pos
            )),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_string(&mut self) -> Result<Node, String> {
        let quote_char = self.consume_char().ok_or("expected string")?;
        let quote = if quote_char == '\'' {
            Quote::Single
        } else {
            Quote::Double
        };

        let mut value = String::new();
        loop {
            match self.consume_char() {
                Some(c) if c == quote_char => break,
                Some('\\') => {
                    match self.consume_char() {
                        Some('n') => value.push('\n'),
                        Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'),
                        Some('\\') => value.push('\\'),
                        Some('\'') => value.push('\''),
                        Some('"') => value.push('"'),
                        Some('/') => value.push('/'),
                        Some('b') => value.push('\u{08}'),
                        Some('f') => value.push('\u{0C}'),
                        Some('0') => value.push('\0'),
                        Some('u') => {
                            let hex: String = (0..4).filter_map(|_| self.consume_char()).collect();
                            if let Ok(cp) = u32::from_str_radix(&hex, 16)
                                && let Some(c) = char::from_u32(cp)
                            {
                                value.push(c);
                            }
                        }
                        Some('x') => {
                            let hex: String = (0..2).filter_map(|_| self.consume_char()).collect();
                            if let Ok(cp) = u32::from_str_radix(&hex, 16)
                                && let Some(c) = char::from_u32(cp)
                            {
                                value.push(c);
                            }
                        }
                        Some('\n') => {} // line continuation
                        Some(c) => {
                            value.push('\\');
                            value.push(c);
                        }
                        None => return Err("unexpected end of string".to_string()),
                    }
                }
                Some(c) => value.push(c),
                None => return Err("unterminated string".to_string()),
            }
        }

        Ok(Node::String { value, quote })
    }

    fn parse_number(&mut self) -> Result<Node, String> {
        let start = self.pos;

        // Optional sign
        if self.peek() == Some('-') || self.peek() == Some('+') {
            self.advance(1);
        }

        // Check for special values after sign
        if self.remaining().starts_with("Infinity") {
            self.advance(8);
            return Ok(Node::Number(self.input[start..self.pos].to_string()));
        }
        if self.remaining().starts_with("NaN") {
            self.advance(3);
            return Ok(Node::Number(self.input[start..self.pos].to_string()));
        }

        // Hex
        if self.remaining().starts_with("0x") || self.remaining().starts_with("0X") {
            self.advance(2);
            while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                self.advance(1);
            }
            return Ok(Node::Number(self.input[start..self.pos].to_string()));
        }

        // Integer part (may start with .)
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance(1);
        }

        // Decimal
        if self.peek() == Some('.') {
            self.advance(1);
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance(1);
            }
        }

        // Exponent
        if self.peek() == Some('e') || self.peek() == Some('E') {
            self.advance(1);
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance(1);
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance(1);
            }
        }

        let num_str = self.input[start..self.pos].to_string();
        if num_str.is_empty() || num_str == "-" || num_str == "+" {
            return Err(format!("invalid number at position {start}"));
        }

        Ok(Node::Number(num_str))
    }

    fn parse_array(&mut self) -> Result<Node, String> {
        self.advance(1); // skip '['
        let mut elements = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(']') {
                self.advance(1);
                break;
            }

            let leading = self.take_pending_comments();
            let value = self.parse_value()?;
            self.skip_whitespace_and_comments();
            let trailing = self.take_pending_comments();
            let trailing_comment = trailing.into_iter().next();

            let has_comma = self.peek() == Some(',');
            if has_comma {
                self.advance(1);
            }

            elements.push(ArrayElement {
                leading_comments: leading,
                value,
                trailing_comment,
                has_trailing_comma: has_comma,
            });

            if !has_comma {
                self.skip_whitespace_and_comments();
                if self.peek() == Some(']') {
                    self.advance(1);
                    break;
                }
            }
        }

        Ok(Node::Array(elements))
    }

    fn parse_object(&mut self) -> Result<Node, String> {
        self.advance(1); // skip '{'
        let mut entries = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                self.advance(1);
                break;
            }

            let leading = self.take_pending_comments();
            let key = self.parse_key()?;
            self.skip_whitespace_and_comments();

            // Expect ':'
            if self.peek() != Some(':') {
                return Err(format!("expected ':' at position {}", self.pos));
            }
            self.advance(1);
            self.skip_whitespace_and_comments();
            // Discard comments between : and value
            let _ = self.take_pending_comments();

            let value = self.parse_value()?;
            self.skip_whitespace_and_comments();
            let trailing = self.take_pending_comments();
            let trailing_comment = trailing.into_iter().next();

            let has_comma = self.peek() == Some(',');
            if has_comma {
                self.advance(1);
            }

            entries.push(ObjectEntry {
                leading_comments: leading,
                key,
                value,
                trailing_comment,
                has_trailing_comma: has_comma,
            });

            if !has_comma {
                self.skip_whitespace_and_comments();
                if self.peek() == Some('}') {
                    self.advance(1);
                    break;
                }
            }
        }

        Ok(Node::Object(entries))
    }

    fn parse_key(&mut self) -> Result<Key, String> {
        match self.peek() {
            Some('"' | '\'') => {
                if let Node::String { value, quote } = self.parse_string()? {
                    Ok(Key::String { value, quote })
                } else {
                    Err("expected string key".to_string())
                }
            }
            Some(c) if c == '_' || c == '$' || c.is_alphabetic() => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c == '_' || c == '$' || c.is_alphanumeric())
                {
                    self.advance(c.len_utf8());
                }
                Ok(Key::Identifier(self.input[start..self.pos].to_string()))
            }
            _ => Err(format!("expected property key at position {}", self.pos)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_object() {
        let (node, _) = parse(r#"{ "key": "value" }"#).expect("parse");
        match node {
            Node::Object(entries) => {
                assert_eq!(entries.len(), 1);
                match &entries[0].key {
                    Key::String { value, .. } => assert_eq!(value, "key"),
                    _ => panic!("expected string key"),
                }
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_unquoted_keys() {
        let (node, _) = parse(r#"{ key: "value" }"#).expect("parse");
        match node {
            Node::Object(entries) => match &entries[0].key {
                Key::Identifier(name) => assert_eq!(name, "key"),
                _ => panic!("expected identifier key"),
            },
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_trailing_commas() {
        let (node, _) = parse(r#"{ a: 1, b: 2, }"#).expect("parse");
        match node {
            Node::Object(entries) => assert_eq!(entries.len(), 2),
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_single_quoted_strings() {
        let (node, _) = parse("'hello'").expect("parse");
        match node {
            Node::String { value, quote } => {
                assert_eq!(value, "hello");
                assert_eq!(quote, Quote::Single);
            }
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn parse_comments() {
        let input = r#"{
            // line comment
            key: "value",
            /* block comment */
            other: 42
        }"#;
        let (node, _) = parse(input).expect("parse");
        match node {
            Node::Object(entries) => {
                assert_eq!(entries.len(), 2);
                assert!(!entries[0].leading_comments.is_empty());
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_hex_numbers() {
        let (node, _) = parse("0xFF").expect("parse");
        match node {
            Node::Number(s) => assert_eq!(s, "0xFF"),
            _ => panic!("expected number"),
        }
    }

    #[test]
    fn parse_array() {
        let (node, _) = parse("[1, 2, 3,]").expect("parse");
        match node {
            Node::Array(elements) => assert_eq!(elements.len(), 3),
            _ => panic!("expected array"),
        }
    }
}
