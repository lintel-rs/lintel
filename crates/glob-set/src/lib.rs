#![doc = include_str!("../README.md")]
#![no_std]
extern crate alloc;

mod engine;
mod error;
mod glob;
mod literal;
mod map;
mod parse;
mod set;
mod strategy;
mod tinyset;

pub use crate::error::{Error, ErrorKind};
pub use crate::glob::{Candidate, Glob, GlobBuilder, GlobMatcher};
pub use crate::map::{GlobMap, GlobMapBuilder};
pub use crate::set::{GlobSet, GlobSetBuilder};
pub use crate::tinyset::{TinyGlobSet, TinyGlobSetBuilder};

use alloc::string::String;

/// Escape all special glob characters in the given string.
///
/// The returned string, when used as a glob pattern, will match the input
/// string literally.
pub fn escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '?' | '[' | ']' | '{' | '}' | '\\' | '!' | '^' | ',' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn escape_special_chars() {
        assert_eq!(escape("*.rs"), "\\*.rs");
        assert_eq!(escape("[foo]"), "\\[foo\\]");
        assert_eq!(escape("{a,b}"), "\\{a\\,b\\}");
        assert_eq!(escape("a\\b"), "a\\\\b");
        assert_eq!(escape("!negated"), "\\!negated");
    }

    #[test]
    fn escape_round_trip() {
        let original = "hello*world?[test]{a,b}";
        let escaped = escape(original);
        let glob = Glob::new(&escaped).unwrap();
        let matcher = glob.compile_matcher();
        assert!(matcher.is_match(original));
    }

    #[test]
    fn escape_no_special() {
        assert_eq!(escape("hello.txt"), "hello.txt");
        assert_eq!(escape("src/main.rs"), "src/main.rs");
    }
}
