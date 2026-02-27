#![doc = include_str!("../README.md")]
#![no_std]
extern crate alloc;

mod syntax;
pub use syntax::{skip_braces, skip_char_class};

use alloc::vec::Vec;
use core::ops::Range;

#[allow(clippy::inline_always)]
#[inline(always)]
fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// Character index into the path string.
    path_index: usize,
    /// Character index into the glob string.
    glob_index: usize,

    /// When we hit a * or **, we store the state for backtracking.
    wildcard: Wildcard,
    globstar: Wildcard,

    /// The current index into the captures list.
    capture_index: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct Wildcard {
    /// Using u32 rather than usize for these results in 10% faster performance.
    glob: u32,
    path: u32,
    capture: u32,
}

#[derive(PartialEq)]
enum BraceState {
    Invalid,
    Comma,
    EndBrace,
}

struct BraceStack {
    stack: [State; 10],
    length: u32,
    longest_brace_match: u32,
}

type Capture = Range<usize>;

pub fn glob_match(glob: &str, path: &str) -> bool {
    Matcher::new(glob, path).run(None)
}

pub fn glob_match_with_captures(glob: &str, path: &str) -> Option<Vec<Capture>> {
    let mut captures = Vec::new();
    if Matcher::new(glob, path).run(Some(&mut captures)) {
        return Some(captures);
    }
    None
}

enum Step {
    Continue,
    Return(bool),
    Backtrack,
}

struct Matcher<'a> {
    glob: &'a [u8],
    path: &'a [u8],
    state: State,
    brace_stack: BraceStack,
}

#[allow(clippy::similar_names)]
impl<'a> Matcher<'a> {
    fn new(glob: &'a str, path: &'a str) -> Self {
        Matcher {
            glob: glob.as_bytes(),
            path: path.as_bytes(),
            state: State::default(),
            brace_stack: BraceStack::default(),
        }
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn unescape(&mut self, c: &mut u8) -> bool {
        if *c == b'\\' {
            self.state.glob_index += 1;
            if self.state.glob_index >= self.glob.len() {
                // Invalid pattern!
                return false;
            }
            *c = match self.glob[self.state.glob_index] {
                b'a' => b'\x61', // \a → literal 'a' (not BEL)
                b'b' => b'\x08',
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                c => c,
            }
        }
        true
    }

    // This algorithm is based on https://research.swtch.com/glob
    fn run(&mut self, mut captures: Option<&mut Vec<Capture>>) -> bool {
        // First, check if the pattern is negated with a leading '!' character.
        let mut negated = false;
        while self.state.glob_index < self.glob.len() && self.glob[self.state.glob_index] == b'!' {
            negated = !negated;
            self.state.glob_index += 1;
        }

        while self.state.glob_index < self.glob.len() || self.state.path_index < self.path.len() {
            if self.state.glob_index < self.glob.len() {
                match self.glob[self.state.glob_index] {
                    b'*' => match self.match_star(&mut captures) {
                        Step::Continue => continue,
                        Step::Return(v) => return v,
                        Step::Backtrack => {}
                    },
                    b'?' if self.state.path_index < self.path.len() => {
                        if !is_separator(self.path[self.state.path_index] as char) {
                            self.state.add_char_capture(&mut captures);
                            self.state.glob_index += 1;
                            self.state.path_index += 1;
                            continue;
                        }
                    }
                    b'[' if self.state.path_index < self.path.len() => {
                        match self.match_bracket(&mut captures) {
                            Step::Continue => continue,
                            Step::Return(v) => return v,
                            Step::Backtrack => {}
                        }
                    }
                    b'{' => {
                        if self.brace_stack.length as usize >= self.brace_stack.stack.len() {
                            return false;
                        }
                        self.state.end_capture(&mut captures);
                        self.state.begin_capture(
                            &mut captures,
                            self.state.path_index..self.state.path_index,
                        );
                        let snap = self.state;
                        self.state = self.brace_stack.push(&snap);
                        continue;
                    }
                    b'}' if self.brace_stack.length > 0 => {
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            self.brace_stack.longest_brace_match = self
                                .brace_stack
                                .longest_brace_match
                                .max(self.state.path_index as u32 + 1);
                        }
                        self.state.glob_index += 1;
                        let snap = self.state;
                        self.state = self.brace_stack.pop(&snap, &mut captures);
                        continue;
                    }
                    b',' if self.brace_stack.length > 0 => {
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            self.brace_stack.longest_brace_match = self
                                .brace_stack
                                .longest_brace_match
                                .max(self.state.path_index as u32 + 1);
                        }
                        self.state.path_index = self.brace_stack.last().path_index;
                        self.state.glob_index += 1;
                        self.state.wildcard = Wildcard::default();
                        self.state.globstar = Wildcard::default();
                        continue;
                    }
                    _ if self.state.path_index < self.path.len() => {
                        match self.match_literal(&mut captures) {
                            Step::Continue => continue,
                            Step::Return(v) => return v,
                            Step::Backtrack => {}
                        }
                    }
                    _ => {}
                }
            }

            match self.try_backtrack(&mut captures, negated) {
                Step::Continue => {}
                Step::Return(v) => return v,
                Step::Backtrack => unreachable!(),
            }
        }

        if self.brace_stack.length > 0
            && self.state.glob_index > 0
            && self.glob[self.state.glob_index - 1] == b'}'
        {
            #[allow(clippy::cast_possible_truncation)]
            {
                self.brace_stack.longest_brace_match = self.state.path_index as u32 + 1;
            }
            let snap = self.state;
            self.brace_stack.pop(&snap, &mut captures);
        }

        !negated
    }

    #[allow(clippy::cast_possible_truncation)]
    fn match_star(&mut self, captures: &mut Option<&mut Vec<Capture>>) -> Step {
        let is_globstar = self.state.glob_index + 1 < self.glob.len()
            && self.glob[self.state.glob_index + 1] == b'*';
        if is_globstar {
            self.skip_globstars();
        }

        // If we are on a different glob index than before, start a new capture.
        // Otherwise, extend the active one.
        if captures.as_ref().is_some_and(|c| {
            c.is_empty() || self.state.glob_index != self.state.wildcard.glob as usize
        }) {
            self.state.wildcard.capture = self.state.capture_index as u32;
            self.state
                .begin_capture(captures, self.state.path_index..self.state.path_index);
        } else {
            self.state.extend_capture(captures);
        }

        self.state.wildcard.glob = self.state.glob_index as u32;
        self.state.wildcard.path = self.state.path_index as u32 + 1;

        // ** allows path separators, whereas * does not.
        // However, ** must be a full path component, i.e. a/**/b not a**b.
        let mut in_globstar = false;
        if is_globstar {
            self.state.glob_index += 2;
            let is_end_invalid = self.state.glob_index != self.glob.len()
                && !(self.brace_stack.length > 0
                    && matches!(self.glob[self.state.glob_index], b'}' | b','));
            let preceded_by_sep = self.state.glob_index < 3
                || self.glob[self.state.glob_index - 3] == b'/'
                || (self.brace_stack.length > 0
                    && matches!(self.glob[self.state.glob_index - 3], b'{' | b','));
            if preceded_by_sep && (!is_end_invalid || self.glob[self.state.glob_index] == b'/') {
                if is_end_invalid {
                    self.state.end_capture(captures);
                    self.state.glob_index += 1;
                }
                self.skip_to_separator(is_end_invalid);
                in_globstar = true;
            }
        } else {
            self.state.glob_index += 1;
        }

        if self.state.path_index < self.path.len()
            && is_separator(self.path[self.state.path_index] as char)
        {
            if in_globstar {
                self.state.path_index += 1;
            } else if self.state.globstar.path > 0 && self.state.path_index < self.path.len() {
                self.state.wildcard = self.state.globstar;
            } else {
                self.state.wildcard.path = 0;
            }
        }

        // If the next char is a special brace separator,
        // skip to the end of the braces so we don't try to match it.
        if self.brace_stack.length > 0
            && self.state.glob_index < self.glob.len()
            && matches!(self.glob[self.state.glob_index], b',' | b'}')
            && self.skip_braces(captures, false) == BraceState::Invalid
        {
            return Step::Return(false);
        }

        Step::Continue
    }

    fn match_bracket(&mut self, captures: &mut Option<&mut Vec<Capture>>) -> Step {
        self.state.glob_index += 1;
        let c = self.path[self.state.path_index];

        // Check if the character class is negated.
        let mut negated = false;
        if self.state.glob_index < self.glob.len()
            && matches!(self.glob[self.state.glob_index], b'^' | b'!')
        {
            negated = true;
            self.state.glob_index += 1;
        }

        // Try each range.
        let mut first = true;
        let mut is_match = false;
        while self.state.glob_index < self.glob.len()
            && (first || self.glob[self.state.glob_index] != b']')
        {
            let mut low = self.glob[self.state.glob_index];
            if !self.unescape(&mut low) {
                return Step::Return(false);
            }
            self.state.glob_index += 1;

            // If there is a - and the following character is not ], read the range end character.
            let high = if self.state.glob_index + 1 < self.glob.len()
                && self.glob[self.state.glob_index] == b'-'
                && self.glob[self.state.glob_index + 1] != b']'
            {
                self.state.glob_index += 1;
                let mut high = self.glob[self.state.glob_index];
                if !self.unescape(&mut high) {
                    return Step::Return(false);
                }
                self.state.glob_index += 1;
                high
            } else {
                low
            };

            if low <= c && c <= high {
                is_match = true;
            }
            first = false;
        }
        if self.state.glob_index >= self.glob.len() {
            return Step::Return(false);
        }
        self.state.glob_index += 1;
        if is_match != negated {
            self.state.add_char_capture(captures);
            self.state.path_index += 1;
            return Step::Continue;
        }
        Step::Backtrack
    }

    #[allow(clippy::cast_possible_truncation)]
    fn match_literal(&mut self, captures: &mut Option<&mut Vec<Capture>>) -> Step {
        let mut c = self.glob[self.state.glob_index];
        if !self.unescape(&mut c) {
            return Step::Return(false);
        }

        let is_match = if c == b'/' {
            is_separator(self.path[self.state.path_index] as char)
        } else {
            self.path[self.state.path_index] == c
        };

        if is_match {
            self.state.end_capture(captures);

            if self.brace_stack.length > 0
                && self.state.glob_index > 0
                && self.glob[self.state.glob_index - 1] == b'}'
            {
                self.brace_stack.longest_brace_match = self.state.path_index as u32 + 1;
                let snap = self.state;
                self.state = self.brace_stack.pop(&snap, captures);
            }
            self.state.glob_index += 1;
            self.state.path_index += 1;

            if c == b'/' {
                self.state.wildcard = self.state.globstar;
            }
            return Step::Continue;
        }
        Step::Backtrack
    }

    fn try_backtrack(&mut self, captures: &mut Option<&mut Vec<Capture>>, negated: bool) -> Step {
        // If we didn't match, restore state to the previous star pattern.
        if self.state.wildcard.path > 0 && self.state.wildcard.path as usize <= self.path.len() {
            self.state.backtrack();
            return Step::Continue;
        }

        if self.brace_stack.length > 0 {
            match self.skip_braces(captures, true) {
                BraceState::Invalid => return Step::Return(false),
                BraceState::Comma => {
                    self.state.path_index = self.brace_stack.last().path_index;
                    return Step::Continue;
                }
                BraceState::EndBrace => {
                    if self.brace_stack.longest_brace_match > 0 {
                        let snap = self.state;
                        self.state = self.brace_stack.pop(&snap, captures);
                        return Step::Continue;
                    }
                    self.state = *self.brace_stack.last();
                    self.brace_stack.length -= 1;
                    if let Some(captures) = captures {
                        captures.truncate(self.state.capture_index);
                    }
                    if self.state.wildcard.path > 0
                        && self.state.wildcard.path as usize <= self.path.len()
                    {
                        self.state.backtrack();
                        return Step::Continue;
                    }
                }
            }
        }

        Step::Return(negated)
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn skip_globstars(&mut self) {
        let mut glob_index = self.state.glob_index + 2;
        while glob_index + 4 <= self.glob.len() && &self.glob[glob_index..glob_index + 4] == b"/**/"
        {
            glob_index += 3;
        }
        if glob_index + 3 == self.glob.len() && &self.glob[glob_index..] == b"/**" {
            glob_index += 3;
        }
        self.state.glob_index = glob_index - 2;
    }

    #[allow(clippy::inline_always, clippy::cast_possible_truncation)]
    #[inline(always)]
    fn skip_to_separator(&mut self, is_end_invalid: bool) {
        if self.state.path_index == self.path.len() {
            self.state.wildcard.path += 1;
            return;
        }

        let mut path_index = self.state.path_index + 1;
        while path_index < self.path.len() && !is_separator(self.path[path_index] as char) {
            path_index += 1;
        }

        if is_end_invalid && path_index == self.path.len() {
            path_index += 1;
        }

        self.state.wildcard.path = path_index as u32;
        self.state.globstar = self.state.wildcard;
    }

    fn skip_braces(
        &mut self,
        captures: &mut Option<&mut Vec<Capture>>,
        stop_on_comma: bool,
    ) -> BraceState {
        let mut braces = 1;
        let mut in_brackets = false;
        let mut capture_index = self.state.capture_index + 1;
        while self.state.glob_index < self.glob.len() && braces > 0 {
            match self.glob[self.state.glob_index] {
                b'{' if !in_brackets => braces += 1,
                b'}' if !in_brackets => braces -= 1,
                b',' if stop_on_comma && braces == 1 && !in_brackets => {
                    self.state.glob_index += 1;
                    return BraceState::Comma;
                }
                c @ (b'*' | b'?' | b'[') if !in_brackets => {
                    if c == b'[' {
                        in_brackets = true;
                    }
                    if let Some(captures) = captures {
                        if capture_index < captures.len() {
                            captures[capture_index] = self.state.path_index..self.state.path_index;
                        } else {
                            captures.push(self.state.path_index..self.state.path_index);
                        }
                        capture_index += 1;
                    }
                    if c == b'*'
                        && self.state.glob_index + 1 < self.glob.len()
                        && self.glob[self.state.glob_index + 1] == b'*'
                    {
                        self.skip_globstars();
                        self.state.glob_index += 1;
                    }
                }
                b']' => in_brackets = false,
                b'\\' => {
                    self.state.glob_index += 1;
                }
                _ => {}
            }
            self.state.glob_index += 1;
        }

        if braces != 0 {
            return BraceState::Invalid;
        }

        BraceState::EndBrace
    }
}

impl State {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn backtrack(&mut self) {
        self.glob_index = self.wildcard.glob as usize;
        self.path_index = self.wildcard.path as usize;
        self.capture_index = self.wildcard.capture as usize;
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn begin_capture(&self, captures: &mut Option<&mut Vec<Capture>>, capture: Capture) {
        if let Some(captures) = captures {
            if self.capture_index < captures.len() {
                captures[self.capture_index] = capture;
            } else {
                captures.push(capture);
            }
        }
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn extend_capture(&self, captures: &mut Option<&mut Vec<Capture>>) {
        if let Some(captures) = captures
            && self.capture_index < captures.len()
        {
            captures[self.capture_index].end = self.path_index;
        }
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn end_capture(&mut self, captures: &mut Option<&mut Vec<Capture>>) {
        if let Some(captures) = captures
            && self.capture_index < captures.len()
        {
            self.capture_index += 1;
        }
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn add_char_capture(&mut self, captures: &mut Option<&mut Vec<Capture>>) {
        self.end_capture(captures);
        self.begin_capture(captures, self.path_index..self.path_index + 1);
        self.capture_index += 1;
    }
}

impl Default for BraceStack {
    #[inline]
    fn default() -> Self {
        // Manual implementation is faster than the automatically derived one.
        BraceStack {
            stack: [State::default(); 10],
            length: 0,
            longest_brace_match: 0,
        }
    }
}

impl BraceStack {
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn push(&mut self, state: &State) -> State {
        // Push old state to the stack, and reset current state.
        self.stack[self.length as usize] = *state;
        self.length += 1;
        State {
            path_index: state.path_index,
            glob_index: state.glob_index + 1,
            capture_index: state.capture_index + 1,
            ..State::default()
        }
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn pop(&mut self, state: &State, captures: &mut Option<&mut Vec<Capture>>) -> State {
        self.length -= 1;
        let mut state = State {
            path_index: (self.longest_brace_match - 1) as usize,
            glob_index: state.glob_index,
            // But restore star state if needed later.
            wildcard: self.stack[self.length as usize].wildcard,
            globstar: self.stack[self.length as usize].globstar,
            capture_index: self.stack[self.length as usize].capture_index,
        };
        if self.length == 0 {
            self.longest_brace_match = 0;
        }
        state.extend_capture(captures);
        if let Some(captures) = captures {
            state.capture_index = captures.len();
        }

        state
    }

    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn last(&self) -> &State {
        &self.stack[self.length as usize - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn basic_wildcards() {
        assert!(glob_match("abc", "abc"));
        assert!(glob_match("*", "abc"));
        assert!(glob_match("*", ""));
        assert!(glob_match("**", ""));
        assert!(glob_match("*c", "abc"));
        assert!(!glob_match("*b", "abc"));
        assert!(glob_match("a*", "abc"));
        assert!(!glob_match("b*", "abc"));
        assert!(glob_match("a*", "a"));
        assert!(glob_match("*a", "a"));
        assert!(glob_match("a*b*c*d*e*", "axbxcxdxe"));
        assert!(glob_match("a*b*c*d*e*", "axbxcxdxexxx"));
        assert!(glob_match("a*b?c*x", "abxbbxdbxebxczzx"));
        assert!(!glob_match("a*b?c*x", "abxbbxdbxebxczzy"));
    }

    #[test]
    fn basic_paths() {
        assert!(glob_match("a/*/test", "a/foo/test"));
        assert!(!glob_match("a/*/test", "a/foo/bar/test"));
        assert!(glob_match("a/**/test", "a/foo/test"));
        assert!(glob_match("a/**/test", "a/foo/bar/test"));
        assert!(glob_match("a/**/b/c", "a/foo/bar/b/c"));
        assert!(glob_match("a\\*b", "a*b"));
        assert!(!glob_match("a\\*b", "axb"));
    }

    #[test]
    fn basic_char_classes() {
        assert!(glob_match("[abc]", "a"));
        assert!(glob_match("[abc]", "b"));
        assert!(glob_match("[abc]", "c"));
        assert!(!glob_match("[abc]", "d"));
        assert!(glob_match("x[abc]x", "xax"));
        assert!(glob_match("x[abc]x", "xbx"));
        assert!(glob_match("x[abc]x", "xcx"));
        assert!(!glob_match("x[abc]x", "xdx"));
        assert!(!glob_match("x[abc]x", "xay"));
        assert!(glob_match("[?]", "?"));
        assert!(!glob_match("[?]", "a"));
        assert!(glob_match("[*]", "*"));
        assert!(!glob_match("[*]", "a"));
    }

    #[test]
    fn basic_char_ranges() {
        assert!(glob_match("[a-cx]", "a"));
        assert!(glob_match("[a-cx]", "b"));
        assert!(glob_match("[a-cx]", "c"));
        assert!(!glob_match("[a-cx]", "d"));
        assert!(glob_match("[a-cx]", "x"));
    }

    #[test]
    fn basic_negated_classes() {
        assert!(!glob_match("[^abc]", "a"));
        assert!(!glob_match("[^abc]", "b"));
        assert!(!glob_match("[^abc]", "c"));
        assert!(glob_match("[^abc]", "d"));
        assert!(!glob_match("[!abc]", "a"));
        assert!(!glob_match("[!abc]", "b"));
        assert!(!glob_match("[!abc]", "c"));
        assert!(glob_match("[!abc]", "d"));
        assert!(glob_match("[\\!]", "!"));

        assert!(glob_match("a*b*[cy]*d*e*", "axbxcxdxexxx"));
        assert!(glob_match("a*b*[cy]*d*e*", "axbxyxdxexxx"));
        assert!(glob_match("a*b*[cy]*d*e*", "axbxxxyxdxexxx"));
    }

    #[test]
    fn basic_braces() {
        assert!(glob_match("test.{jpg,png}", "test.jpg"));
        assert!(glob_match("test.{jpg,png}", "test.png"));
        assert!(glob_match("test.{j*g,p*g}", "test.jpg"));
        assert!(glob_match("test.{j*g,p*g}", "test.jpxxxg"));
        assert!(glob_match("test.{j*g,p*g}", "test.jxg"));
        assert!(!glob_match("test.{j*g,p*g}", "test.jnt"));
        assert!(glob_match("test.{j*g,j*c}", "test.jnc"));
        assert!(glob_match("test.{jpg,p*g}", "test.png"));
        assert!(glob_match("test.{jpg,p*g}", "test.pxg"));
        assert!(!glob_match("test.{jpg,p*g}", "test.pnt"));
        assert!(glob_match("test.{jpeg,png}", "test.jpeg"));
        assert!(!glob_match("test.{jpeg,png}", "test.jpg"));
        assert!(glob_match("test.{jpeg,png}", "test.png"));
        assert!(glob_match("test.{jp\\,g,png}", "test.jp,g"));
        assert!(!glob_match("test.{jp\\,g,png}", "test.jxg"));
        assert!(glob_match("test/{foo,bar}/baz", "test/foo/baz"));
        assert!(glob_match("test/{foo,bar}/baz", "test/bar/baz"));
        assert!(!glob_match("test/{foo,bar}/baz", "test/baz/baz"));
        assert!(glob_match("test/{foo*,bar*}/baz", "test/foooooo/baz"));
        assert!(glob_match("test/{foo*,bar*}/baz", "test/barrrrr/baz"));
        assert!(glob_match("test/{*foo,*bar}/baz", "test/xxxxfoo/baz"));
        assert!(glob_match("test/{*foo,*bar}/baz", "test/xxxxbar/baz"));
        assert!(glob_match("test/{foo/**,bar}/baz", "test/bar/baz"));
        assert!(!glob_match("test/{foo/**,bar}/baz", "test/bar/test/baz"));
    }

    #[test]
    fn basic_complex() {
        assert!(!glob_match("*.txt", "some/big/path/to/the/needle.txt"));
        assert!(glob_match(
            "some/**/needle.{js,tsx,mdx,ts,jsx,txt}",
            "some/a/bigger/path/to/the/crazy/needle.txt"
        ));
        assert!(glob_match(
            "some/**/{a,b,c}/**/needle.txt",
            "some/foo/a/bigger/path/to/the/crazy/needle.txt"
        ));
        assert!(!glob_match(
            "some/**/{a,b,c}/**/needle.txt",
            "some/foo/d/bigger/path/to/the/crazy/needle.txt"
        ));
        assert!(glob_match("a/{a{a,b},b}", "a/aa"));
        assert!(glob_match("a/{a{a,b},b}", "a/ab"));
        assert!(!glob_match("a/{a{a,b},b}", "a/ac"));
        assert!(glob_match("a/{a{a,b},b}", "a/b"));
        assert!(!glob_match("a/{a{a,b},b}", "a/c"));
        assert!(glob_match("a/{b,c[}]*}", "a/b"));
        assert!(glob_match("a/{b,c[}]*}", "a/c}xx"));
        assert!(glob_match("{[}],foo}", "}"));
        assert!(glob_match("{[}],foo}", "foo"));
    }

    // The below tests are based on Bash and micromatch.
    // https://github.com/micromatch/picomatch/blob/master/test/bash.js
    // Converted using the following find and replace regex:
    // find: assert\(([!])?isMatch\('(.*?)', ['"](.*?)['"]\)\);
    // replace: assert!($1glob_match("$3", "$2"));

    #[test]
    fn bash_a_star() {
        assert!(!glob_match("a*", "*"));
        assert!(!glob_match("a*", "**"));
        assert!(!glob_match("a*", "\\*"));
        assert!(!glob_match("a*", "a/*"));
        assert!(!glob_match("a*", "b"));
        assert!(!glob_match("a*", "bc"));
        assert!(!glob_match("a*", "bcd"));
        assert!(!glob_match("a*", "bdir/"));
        assert!(!glob_match("a*", "Beware"));
        assert!(glob_match("a*", "a"));
        assert!(glob_match("a*", "ab"));
        assert!(glob_match("a*", "abc"));
    }

    #[test]
    fn bash_escaped_a_star() {
        assert!(!glob_match("\\a*", "*"));
        assert!(!glob_match("\\a*", "**"));
        assert!(!glob_match("\\a*", "\\*"));

        assert!(glob_match("\\a*", "a"));
        assert!(!glob_match("\\a*", "a/*"));
        assert!(glob_match("\\a*", "abc"));
        assert!(glob_match("\\a*", "abd"));
        assert!(glob_match("\\a*", "abe"));
        assert!(!glob_match("\\a*", "b"));
        assert!(!glob_match("\\a*", "bb"));
        assert!(!glob_match("\\a*", "bcd"));
        assert!(!glob_match("\\a*", "bdir/"));
        assert!(!glob_match("\\a*", "Beware"));
        assert!(!glob_match("\\a*", "c"));
        assert!(!glob_match("\\a*", "ca"));
        assert!(!glob_match("\\a*", "cb"));
        assert!(!glob_match("\\a*", "d"));
        assert!(!glob_match("\\a*", "dd"));
        assert!(!glob_match("\\a*", "de"));
    }

    #[test]
    fn bash_directories() {
        assert!(!glob_match("b*/", "*"));
        assert!(!glob_match("b*/", "**"));
        assert!(!glob_match("b*/", "\\*"));
        assert!(!glob_match("b*/", "a"));
        assert!(!glob_match("b*/", "a/*"));
        assert!(!glob_match("b*/", "abc"));
        assert!(!glob_match("b*/", "abd"));
        assert!(!glob_match("b*/", "abe"));
        assert!(!glob_match("b*/", "b"));
        assert!(!glob_match("b*/", "bb"));
        assert!(!glob_match("b*/", "bcd"));
        assert!(glob_match("b*/", "bdir/"));
        assert!(!glob_match("b*/", "Beware"));
        assert!(!glob_match("b*/", "c"));
        assert!(!glob_match("b*/", "ca"));
        assert!(!glob_match("b*/", "cb"));
        assert!(!glob_match("b*/", "d"));
        assert!(!glob_match("b*/", "dd"));
        assert!(!glob_match("b*/", "de"));
    }

    #[test]
    fn bash_escaping_caret() {
        assert!(!glob_match("\\^", "*"));
        assert!(!glob_match("\\^", "**"));
        assert!(!glob_match("\\^", "\\*"));
        assert!(!glob_match("\\^", "a"));
        assert!(!glob_match("\\^", "a/*"));
        assert!(!glob_match("\\^", "abc"));
        assert!(!glob_match("\\^", "abd"));
        assert!(!glob_match("\\^", "abe"));
        assert!(!glob_match("\\^", "b"));
        assert!(!glob_match("\\^", "bb"));
        assert!(!glob_match("\\^", "bcd"));
        assert!(!glob_match("\\^", "bdir/"));
        assert!(!glob_match("\\^", "Beware"));
        assert!(!glob_match("\\^", "c"));
        assert!(!glob_match("\\^", "ca"));
        assert!(!glob_match("\\^", "cb"));
        assert!(!glob_match("\\^", "d"));
        assert!(!glob_match("\\^", "dd"));
        assert!(!glob_match("\\^", "de"));
    }

    #[test]
    fn bash_escaping_backslash_star() {
        assert!(glob_match("\\*", "*"));
        // assert!(glob_match("\\*", "\\*"));
        assert!(!glob_match("\\*", "**"));
        assert!(!glob_match("\\*", "a"));
        assert!(!glob_match("\\*", "a/*"));
        assert!(!glob_match("\\*", "abc"));
        assert!(!glob_match("\\*", "abd"));
        assert!(!glob_match("\\*", "abe"));
        assert!(!glob_match("\\*", "b"));
        assert!(!glob_match("\\*", "bb"));
        assert!(!glob_match("\\*", "bcd"));
        assert!(!glob_match("\\*", "bdir/"));
        assert!(!glob_match("\\*", "Beware"));
        assert!(!glob_match("\\*", "c"));
        assert!(!glob_match("\\*", "ca"));
        assert!(!glob_match("\\*", "cb"));
        assert!(!glob_match("\\*", "d"));
        assert!(!glob_match("\\*", "dd"));
        assert!(!glob_match("\\*", "de"));
    }

    #[test]
    fn bash_escaping_a_backslash_star() {
        assert!(!glob_match("a\\*", "*"));
        assert!(!glob_match("a\\*", "**"));
        assert!(!glob_match("a\\*", "\\*"));
        assert!(!glob_match("a\\*", "a"));
        assert!(!glob_match("a\\*", "a/*"));
        assert!(!glob_match("a\\*", "abc"));
        assert!(!glob_match("a\\*", "abd"));
        assert!(!glob_match("a\\*", "abe"));
        assert!(!glob_match("a\\*", "b"));
        assert!(!glob_match("a\\*", "bb"));
        assert!(!glob_match("a\\*", "bcd"));
        assert!(!glob_match("a\\*", "bdir/"));
        assert!(!glob_match("a\\*", "Beware"));
        assert!(!glob_match("a\\*", "c"));
        assert!(!glob_match("a\\*", "ca"));
        assert!(!glob_match("a\\*", "cb"));
        assert!(!glob_match("a\\*", "d"));
        assert!(!glob_match("a\\*", "dd"));
        assert!(!glob_match("a\\*", "de"));
    }

    #[test]
    fn bash_escaping_star_q_star() {
        assert!(glob_match("*q*", "aqa"));
        assert!(glob_match("*q*", "aaqaa"));
        assert!(!glob_match("*q*", "*"));
        assert!(!glob_match("*q*", "**"));
        assert!(!glob_match("*q*", "\\*"));
        assert!(!glob_match("*q*", "a"));
        assert!(!glob_match("*q*", "a/*"));
        assert!(!glob_match("*q*", "abc"));
        assert!(!glob_match("*q*", "abd"));
        assert!(!glob_match("*q*", "abe"));
        assert!(!glob_match("*q*", "b"));
        assert!(!glob_match("*q*", "bb"));
        assert!(!glob_match("*q*", "bcd"));
        assert!(!glob_match("*q*", "bdir/"));
        assert!(!glob_match("*q*", "Beware"));
        assert!(!glob_match("*q*", "c"));
        assert!(!glob_match("*q*", "ca"));
        assert!(!glob_match("*q*", "cb"));
        assert!(!glob_match("*q*", "d"));
        assert!(!glob_match("*q*", "dd"));
        assert!(!glob_match("*q*", "de"));
    }

    #[test]
    fn bash_escaping_escaped_star_star() {
        assert!(glob_match("\\**", "*"));
        assert!(glob_match("\\**", "**"));
        assert!(!glob_match("\\**", "\\*"));
        assert!(!glob_match("\\**", "a"));
        assert!(!glob_match("\\**", "a/*"));
        assert!(!glob_match("\\**", "abc"));
        assert!(!glob_match("\\**", "abd"));
        assert!(!glob_match("\\**", "abe"));
        assert!(!glob_match("\\**", "b"));
        assert!(!glob_match("\\**", "bb"));
        assert!(!glob_match("\\**", "bcd"));
        assert!(!glob_match("\\**", "bdir/"));
        assert!(!glob_match("\\**", "Beware"));
        assert!(!glob_match("\\**", "c"));
        assert!(!glob_match("\\**", "ca"));
        assert!(!glob_match("\\**", "cb"));
        assert!(!glob_match("\\**", "d"));
        assert!(!glob_match("\\**", "dd"));
        assert!(!glob_match("\\**", "de"));
    }

    #[test]
    fn bash_classes_negated_non_matching() {
        assert!(!glob_match("a*[^c]", "*"));
        assert!(!glob_match("a*[^c]", "**"));
        assert!(!glob_match("a*[^c]", "\\*"));
        assert!(!glob_match("a*[^c]", "a"));
        assert!(!glob_match("a*[^c]", "a/*"));
        assert!(!glob_match("a*[^c]", "abc"));
        assert!(glob_match("a*[^c]", "abd"));
        assert!(glob_match("a*[^c]", "abe"));
        assert!(!glob_match("a*[^c]", "b"));
        assert!(!glob_match("a*[^c]", "bb"));
        assert!(!glob_match("a*[^c]", "bcd"));
        assert!(!glob_match("a*[^c]", "bdir/"));
        assert!(!glob_match("a*[^c]", "Beware"));
    }

    #[test]
    fn bash_classes_negated_remaining() {
        assert!(!glob_match("a*[^c]", "c"));
        assert!(!glob_match("a*[^c]", "ca"));
        assert!(!glob_match("a*[^c]", "cb"));
        assert!(!glob_match("a*[^c]", "d"));
        assert!(!glob_match("a*[^c]", "dd"));
        assert!(!glob_match("a*[^c]", "de"));
        assert!(!glob_match("a*[^c]", "baz"));
        assert!(!glob_match("a*[^c]", "bzz"));
        assert!(!glob_match("a*[^c]", "BZZ"));
        assert!(!glob_match("a*[^c]", "beware"));
        assert!(!glob_match("a*[^c]", "BewAre"));

        assert!(glob_match("a[X-]b", "a-b"));
        assert!(glob_match("a[X-]b", "aXb"));
    }

    #[test]
    fn bash_classes_range_negated_first_half() {
        assert!(!glob_match("[a-y]*[^c]", "*"));
        assert!(glob_match("[a-y]*[^c]", "a*"));
        assert!(!glob_match("[a-y]*[^c]", "**"));
        assert!(!glob_match("[a-y]*[^c]", "\\*"));
        assert!(!glob_match("[a-y]*[^c]", "a"));
        assert!(glob_match("[a-y]*[^c]", "a123b"));
        assert!(!glob_match("[a-y]*[^c]", "a123c"));
        assert!(glob_match("[a-y]*[^c]", "ab"));
        assert!(!glob_match("[a-y]*[^c]", "a/*"));
        assert!(!glob_match("[a-y]*[^c]", "abc"));
        assert!(glob_match("[a-y]*[^c]", "abd"));
        assert!(glob_match("[a-y]*[^c]", "abe"));
        assert!(!glob_match("[a-y]*[^c]", "b"));
        assert!(glob_match("[a-y]*[^c]", "bd"));
        assert!(glob_match("[a-y]*[^c]", "bb"));
        assert!(glob_match("[a-y]*[^c]", "bcd"));
        assert!(glob_match("[a-y]*[^c]", "bdir/"));
        assert!(!glob_match("[a-y]*[^c]", "Beware"));
    }

    #[test]
    fn bash_classes_range_negated_second_half() {
        assert!(!glob_match("[a-y]*[^c]", "c"));
        assert!(glob_match("[a-y]*[^c]", "ca"));
        assert!(glob_match("[a-y]*[^c]", "cb"));
        assert!(!glob_match("[a-y]*[^c]", "d"));
        assert!(glob_match("[a-y]*[^c]", "dd"));
        assert!(glob_match("[a-y]*[^c]", "de"));
        assert!(glob_match("[a-y]*[^c]", "baz"));
        assert!(glob_match("[a-y]*[^c]", "bzz"));
        // assert(!isMatch('bzz', '[a-y]*[^c]', { regex: true }));
        assert!(!glob_match("[a-y]*[^c]", "BZZ"));
        assert!(glob_match("[a-y]*[^c]", "beware"));
        assert!(!glob_match("[a-y]*[^c]", "BewAre"));

        assert!(glob_match("a\\*b/*", "a*b/ooo"));
        assert!(glob_match("a\\*?/*", "a*b/ooo"));
    }

    #[test]
    fn bash_classes_single_char() {
        assert!(!glob_match("a[b]c", "*"));
        assert!(!glob_match("a[b]c", "**"));
        assert!(!glob_match("a[b]c", "\\*"));
        assert!(!glob_match("a[b]c", "a"));
        assert!(!glob_match("a[b]c", "a/*"));
        assert!(glob_match("a[b]c", "abc"));
        assert!(!glob_match("a[b]c", "abd"));
        assert!(!glob_match("a[b]c", "abe"));
        assert!(!glob_match("a[b]c", "b"));
        assert!(!glob_match("a[b]c", "bb"));
        assert!(!glob_match("a[b]c", "bcd"));
        assert!(!glob_match("a[b]c", "bdir/"));
        assert!(!glob_match("a[b]c", "Beware"));
        assert!(!glob_match("a[b]c", "c"));
        assert!(!glob_match("a[b]c", "ca"));
        assert!(!glob_match("a[b]c", "cb"));
        assert!(!glob_match("a[b]c", "d"));
        assert!(!glob_match("a[b]c", "dd"));
        assert!(!glob_match("a[b]c", "de"));
        assert!(!glob_match("a[b]c", "baz"));
        assert!(!glob_match("a[b]c", "bzz"));
        assert!(!glob_match("a[b]c", "BZZ"));
        assert!(!glob_match("a[b]c", "beware"));
        assert!(!glob_match("a[b]c", "BewAre"));
    }

    #[test]
    fn bash_classes_quoted() {
        assert!(!glob_match("a[\"b\"]c", "*"));
        assert!(!glob_match("a[\"b\"]c", "**"));
        assert!(!glob_match("a[\"b\"]c", "\\*"));
        assert!(!glob_match("a[\"b\"]c", "a"));
        assert!(!glob_match("a[\"b\"]c", "a/*"));
        assert!(glob_match("a[\"b\"]c", "abc"));
        assert!(!glob_match("a[\"b\"]c", "abd"));
        assert!(!glob_match("a[\"b\"]c", "abe"));
        assert!(!glob_match("a[\"b\"]c", "b"));
        assert!(!glob_match("a[\"b\"]c", "bb"));
        assert!(!glob_match("a[\"b\"]c", "bcd"));
        assert!(!glob_match("a[\"b\"]c", "bdir/"));
        assert!(!glob_match("a[\"b\"]c", "Beware"));
        assert!(!glob_match("a[\"b\"]c", "c"));
        assert!(!glob_match("a[\"b\"]c", "ca"));
        assert!(!glob_match("a[\"b\"]c", "cb"));
        assert!(!glob_match("a[\"b\"]c", "d"));
        assert!(!glob_match("a[\"b\"]c", "dd"));
        assert!(!glob_match("a[\"b\"]c", "de"));
        assert!(!glob_match("a[\"b\"]c", "baz"));
        assert!(!glob_match("a[\"b\"]c", "bzz"));
        assert!(!glob_match("a[\"b\"]c", "BZZ"));
        assert!(!glob_match("a[\"b\"]c", "beware"));
        assert!(!glob_match("a[\"b\"]c", "BewAre"));
    }

    #[test]
    fn bash_classes_escaped_double_backslash() {
        assert!(!glob_match("a[\\\\b]c", "*"));
        assert!(!glob_match("a[\\\\b]c", "**"));
        assert!(!glob_match("a[\\\\b]c", "\\*"));
        assert!(!glob_match("a[\\\\b]c", "a"));
        assert!(!glob_match("a[\\\\b]c", "a/*"));
        assert!(glob_match("a[\\\\b]c", "abc"));
        assert!(!glob_match("a[\\\\b]c", "abd"));
        assert!(!glob_match("a[\\\\b]c", "abe"));
        assert!(!glob_match("a[\\\\b]c", "b"));
        assert!(!glob_match("a[\\\\b]c", "bb"));
        assert!(!glob_match("a[\\\\b]c", "bcd"));
        assert!(!glob_match("a[\\\\b]c", "bdir/"));
        assert!(!glob_match("a[\\\\b]c", "Beware"));
        assert!(!glob_match("a[\\\\b]c", "c"));
        assert!(!glob_match("a[\\\\b]c", "ca"));
        assert!(!glob_match("a[\\\\b]c", "cb"));
        assert!(!glob_match("a[\\\\b]c", "d"));
        assert!(!glob_match("a[\\\\b]c", "dd"));
        assert!(!glob_match("a[\\\\b]c", "de"));
        assert!(!glob_match("a[\\\\b]c", "baz"));
        assert!(!glob_match("a[\\\\b]c", "bzz"));
        assert!(!glob_match("a[\\\\b]c", "BZZ"));
        assert!(!glob_match("a[\\\\b]c", "beware"));
        assert!(!glob_match("a[\\\\b]c", "BewAre"));
    }

    #[test]
    fn bash_classes_escaped_single_backslash() {
        assert!(!glob_match("a[\\b]c", "*"));
        assert!(!glob_match("a[\\b]c", "**"));
        assert!(!glob_match("a[\\b]c", "\\*"));
        assert!(!glob_match("a[\\b]c", "a"));
        assert!(!glob_match("a[\\b]c", "a/*"));
        assert!(!glob_match("a[\\b]c", "abc"));
        assert!(!glob_match("a[\\b]c", "abd"));
        assert!(!glob_match("a[\\b]c", "abe"));
        assert!(!glob_match("a[\\b]c", "b"));
        assert!(!glob_match("a[\\b]c", "bb"));
        assert!(!glob_match("a[\\b]c", "bcd"));
        assert!(!glob_match("a[\\b]c", "bdir/"));
        assert!(!glob_match("a[\\b]c", "Beware"));
        assert!(!glob_match("a[\\b]c", "c"));
        assert!(!glob_match("a[\\b]c", "ca"));
        assert!(!glob_match("a[\\b]c", "cb"));
        assert!(!glob_match("a[\\b]c", "d"));
        assert!(!glob_match("a[\\b]c", "dd"));
        assert!(!glob_match("a[\\b]c", "de"));
        assert!(!glob_match("a[\\b]c", "baz"));
        assert!(!glob_match("a[\\b]c", "bzz"));
        assert!(!glob_match("a[\\b]c", "BZZ"));
        assert!(!glob_match("a[\\b]c", "beware"));
        assert!(!glob_match("a[\\b]c", "BewAre"));
    }

    #[test]
    fn bash_classes_range_and_question_range() {
        assert!(!glob_match("a[b-d]c", "*"));
        assert!(!glob_match("a[b-d]c", "**"));
        assert!(!glob_match("a[b-d]c", "\\*"));
        assert!(!glob_match("a[b-d]c", "a"));
        assert!(!glob_match("a[b-d]c", "a/*"));
        assert!(glob_match("a[b-d]c", "abc"));
        assert!(!glob_match("a[b-d]c", "abd"));
        assert!(!glob_match("a[b-d]c", "abe"));
        assert!(!glob_match("a[b-d]c", "b"));
        assert!(!glob_match("a[b-d]c", "bb"));
        assert!(!glob_match("a[b-d]c", "bcd"));
        assert!(!glob_match("a[b-d]c", "bdir/"));
        assert!(!glob_match("a[b-d]c", "Beware"));
        assert!(!glob_match("a[b-d]c", "c"));
        assert!(!glob_match("a[b-d]c", "ca"));
        assert!(!glob_match("a[b-d]c", "cb"));
        assert!(!glob_match("a[b-d]c", "d"));
        assert!(!glob_match("a[b-d]c", "dd"));
        assert!(!glob_match("a[b-d]c", "de"));
        assert!(!glob_match("a[b-d]c", "baz"));
        assert!(!glob_match("a[b-d]c", "bzz"));
        assert!(!glob_match("a[b-d]c", "BZZ"));
        assert!(!glob_match("a[b-d]c", "beware"));
        assert!(!glob_match("a[b-d]c", "BewAre"));
    }

    #[test]
    fn bash_classes_range_and_question_qmark_non_matching() {
        assert!(!glob_match("a?c", "*"));
        assert!(!glob_match("a?c", "**"));
        assert!(!glob_match("a?c", "\\*"));
        assert!(!glob_match("a?c", "a"));
        assert!(!glob_match("a?c", "a/*"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "abd"));
        assert!(!glob_match("a?c", "abe"));
        assert!(!glob_match("a?c", "b"));
        assert!(!glob_match("a?c", "bb"));
        assert!(!glob_match("a?c", "bcd"));
        assert!(!glob_match("a?c", "bdir/"));
        assert!(!glob_match("a?c", "Beware"));
    }

    #[test]
    fn bash_classes_range_and_question_qmark_remaining() {
        assert!(!glob_match("a?c", "c"));
        assert!(!glob_match("a?c", "ca"));
        assert!(!glob_match("a?c", "cb"));
        assert!(!glob_match("a?c", "d"));
        assert!(!glob_match("a?c", "dd"));
        assert!(!glob_match("a?c", "de"));
        assert!(!glob_match("a?c", "baz"));
        assert!(!glob_match("a?c", "bzz"));
        assert!(!glob_match("a?c", "BZZ"));
        assert!(!glob_match("a?c", "beware"));
        assert!(!glob_match("a?c", "BewAre"));

        assert!(glob_match("*/man*/bash.*", "man/man1/bash.1"));
    }

    #[test]
    fn bash_classes_negated_range() {
        assert!(glob_match("[^a-c]*", "*"));
        assert!(glob_match("[^a-c]*", "**"));
        assert!(!glob_match("[^a-c]*", "a"));
        assert!(!glob_match("[^a-c]*", "a/*"));
        assert!(!glob_match("[^a-c]*", "abc"));
        assert!(!glob_match("[^a-c]*", "abd"));
        assert!(!glob_match("[^a-c]*", "abe"));
        assert!(!glob_match("[^a-c]*", "b"));
        assert!(!glob_match("[^a-c]*", "bb"));
        assert!(!glob_match("[^a-c]*", "bcd"));
        assert!(!glob_match("[^a-c]*", "bdir/"));
        assert!(glob_match("[^a-c]*", "Beware"));
        assert!(glob_match("[^a-c]*", "Beware"));
        assert!(!glob_match("[^a-c]*", "c"));
        assert!(!glob_match("[^a-c]*", "ca"));
        assert!(!glob_match("[^a-c]*", "cb"));
        assert!(glob_match("[^a-c]*", "d"));
        assert!(glob_match("[^a-c]*", "dd"));
        assert!(glob_match("[^a-c]*", "de"));
        assert!(!glob_match("[^a-c]*", "baz"));
        assert!(!glob_match("[^a-c]*", "bzz"));
        assert!(glob_match("[^a-c]*", "BZZ"));
        assert!(!glob_match("[^a-c]*", "beware"));
        assert!(glob_match("[^a-c]*", "BewAre"));
    }

    #[test]
    fn bash_wildmatch() {
        assert!(!glob_match("a[]-]b", "aab"));
        assert!(!glob_match("[ten]", "ten"));
        assert!(glob_match("]", "]"));
        assert!(glob_match("a[]-]b", "a-b"));
        assert!(glob_match("a[]-]b", "a]b"));
        assert!(glob_match("a[]]b", "a]b"));
        assert!(glob_match("a[\\]a\\-]b", "aab"));
        assert!(glob_match("t[a-g]n", "ten"));
        assert!(glob_match("t[^a-g]n", "ton"));
    }

    #[test]
    fn bash_slashmatch() {
        // assert!(!glob_match("f[^eiu][^eiu][^eiu][^eiu][^eiu]r", "foo/bar"));
        assert!(glob_match("foo[/]bar", "foo/bar"));
        assert!(glob_match("f[^eiu][^eiu][^eiu][^eiu][^eiu]r", "foo-bar"));
    }

    #[test]
    fn bash_extra_stars_simple() {
        assert!(!glob_match("a**c", "bbc"));
        assert!(glob_match("a**c", "abc"));
        assert!(!glob_match("a**c", "bbd"));

        assert!(!glob_match("a***c", "bbc"));
        assert!(glob_match("a***c", "abc"));
        assert!(!glob_match("a***c", "bbd"));

        assert!(!glob_match("a*****?c", "bbc"));
        assert!(glob_match("a*****?c", "abc"));
        assert!(!glob_match("a*****?c", "bbc"));

        assert!(glob_match("?*****??", "bbc"));
        assert!(glob_match("?*****??", "abc"));

        assert!(glob_match("*****??", "bbc"));
        assert!(glob_match("*****??", "abc"));

        assert!(glob_match("?*****?c", "bbc"));
        assert!(glob_match("?*****?c", "abc"));
    }

    #[test]
    fn bash_extra_stars_complex() {
        assert!(glob_match("?***?****c", "bbc"));
        assert!(glob_match("?***?****c", "abc"));
        assert!(!glob_match("?***?****c", "bbd"));

        assert!(glob_match("?***?****?", "bbc"));
        assert!(glob_match("?***?****?", "abc"));

        assert!(glob_match("?***?****", "bbc"));
        assert!(glob_match("?***?****", "abc"));

        assert!(glob_match("*******c", "bbc"));
        assert!(glob_match("*******c", "abc"));

        assert!(glob_match("*******?", "bbc"));
        assert!(glob_match("*******?", "abc"));

        assert!(glob_match("a*cd**?**??k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??k***", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??***k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??***k**", "abcdecdhjk"));
        assert!(glob_match("a****c**?**??*****", "abcdecdhjk"));
    }

    #[test]
    fn stars_basic() {
        assert!(!glob_match("*.js", "a/b/c/z.js"));
        assert!(!glob_match("*.js", "a/b/z.js"));
        assert!(!glob_match("*.js", "a/z.js"));
        assert!(glob_match("*.js", "z.js"));

        // assert!(!glob_match("*/*", "a/.ab"));
        // assert!(!glob_match("*", ".ab"));

        assert!(glob_match("z*.js", "z.js"));
        assert!(glob_match("*/*", "a/z"));
        assert!(glob_match("*/z*.js", "a/z.js"));
        assert!(glob_match("a/z*.js", "a/z.js"));

        assert!(glob_match("*", "ab"));
        assert!(glob_match("*", "abc"));

        assert!(!glob_match("f*", "bar"));
        assert!(!glob_match("*r", "foo"));
        assert!(!glob_match("b*", "foo"));
        assert!(!glob_match("*", "foo/bar"));
        assert!(glob_match("*c", "abc"));
        assert!(glob_match("a*", "abc"));
        assert!(glob_match("a*c", "abc"));
        assert!(glob_match("*r", "bar"));
        assert!(glob_match("b*", "bar"));
        assert!(glob_match("f*", "foo"));

        assert!(glob_match("*abc*", "one abc two"));
        assert!(glob_match("a*b", "a         b"));
    }

    #[test]
    fn stars_dot_patterns_single() {
        assert!(!glob_match("*a*", "foo"));
        assert!(glob_match("*a*", "bar"));
        assert!(glob_match("*abc*", "oneabctwo"));
        assert!(!glob_match("*-bc-*", "a-b.c-d"));
        assert!(glob_match("*-*.*-*", "a-b.c-d"));
        assert!(glob_match("*-b*c-*", "a-b.c-d"));
        assert!(glob_match("*-b.c-*", "a-b.c-d"));
        assert!(glob_match("*.*", "a-b.c-d"));
        assert!(glob_match("*.*-*", "a-b.c-d"));
        assert!(glob_match("*.*-d", "a-b.c-d"));
        assert!(glob_match("*.c-*", "a-b.c-d"));
        assert!(glob_match("*b.*d", "a-b.c-d"));
        assert!(glob_match("a*.c*", "a-b.c-d"));
        assert!(glob_match("a-*.*-d", "a-b.c-d"));
        assert!(glob_match("*.*", "a.b"));
        assert!(glob_match("*.b", "a.b"));
        assert!(glob_match("a.*", "a.b"));
        assert!(glob_match("a.b", "a.b"));
    }

    #[test]
    fn stars_dot_patterns_double() {
        assert!(!glob_match("**-bc-**", "a-b.c-d"));
        assert!(glob_match("**-**.**-**", "a-b.c-d"));
        assert!(glob_match("**-b**c-**", "a-b.c-d"));
        assert!(glob_match("**-b.c-**", "a-b.c-d"));
        assert!(glob_match("**.**", "a-b.c-d"));
        assert!(glob_match("**.**-**", "a-b.c-d"));
        assert!(glob_match("**.**-d", "a-b.c-d"));
        assert!(glob_match("**.c-**", "a-b.c-d"));
        assert!(glob_match("**b.**d", "a-b.c-d"));
        assert!(glob_match("a**.c**", "a-b.c-d"));
        assert!(glob_match("a-**.**-d", "a-b.c-d"));
        assert!(glob_match("**.**", "a.b"));
        assert!(glob_match("**.b", "a.b"));
        assert!(glob_match("a.**", "a.b"));
        assert!(glob_match("a.b", "a.b"));
    }

    #[test]
    fn stars_paths_positive() {
        assert!(glob_match("*/*", "/ab"));
        assert!(glob_match(".", "."));
        assert!(!glob_match("a/", "a/.b"));
        assert!(glob_match("/*", "/ab"));
        assert!(glob_match("/??", "/ab"));
        assert!(glob_match("/?b", "/ab"));
        assert!(glob_match("/*", "/cd"));
        assert!(glob_match("a", "a"));
        assert!(glob_match("a/.*", "a/.b"));
        assert!(glob_match("?/?", "a/b"));
        assert!(glob_match("a/**/j/**/z/*.md", "a/b/c/d/e/j/n/p/o/z/c.md"));
        assert!(glob_match("a/**/z/*.md", "a/b/c/d/e/z/c.md"));
        assert!(glob_match("a/b/c/*.md", "a/b/c/xyz.md"));
        assert!(glob_match("a/b/c/*.md", "a/b/c/xyz.md"));
        assert!(glob_match("a/*/z/.a", "a/b/z/.a"));
        assert!(!glob_match("bz", "a/b/z/.a"));
        assert!(glob_match("a/**/c/*.md", "a/bb.bb/aa/b.b/aa/c/xyz.md"));
        assert!(glob_match("a/**/c/*.md", "a/bb.bb/aa/bb/aa/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb.bb/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bbbb/c/xyz.md"));
        assert!(glob_match("*", "aaa"));
        assert!(glob_match("*", "ab"));
        assert!(glob_match("ab", "ab"));
    }

    #[test]
    fn stars_paths_negative() {
        assert!(!glob_match("*/*/*", "aaa"));
        assert!(!glob_match("*/*/*", "aaa/bb/aa/rr"));
        assert!(!glob_match("aaa*", "aaa/bba/ccc"));
        // assert!(!glob_match("aaa**", "aaa/bba/ccc"));
        assert!(!glob_match("aaa/*", "aaa/bba/ccc"));
        assert!(!glob_match("aaa/*ccc", "aaa/bba/ccc"));
        assert!(!glob_match("aaa/*z", "aaa/bba/ccc"));
        assert!(!glob_match("*/*/*", "aaa/bbb"));
        assert!(!glob_match("*/*jk*/*i", "ab/zzz/ejkl/hi"));
        assert!(glob_match("*/*/*", "aaa/bba/ccc"));
        assert!(glob_match("aaa/**", "aaa/bba/ccc"));
        assert!(glob_match("aaa/*", "aaa/bbb"));
        assert!(glob_match("*/*z*/*/*i", "ab/zzz/ejkl/hi"));
        assert!(glob_match("*j*i", "abzzzejklhi"));
    }

    #[test]
    fn stars_depth_matching_shallow() {
        assert!(glob_match("*", "a"));
        assert!(glob_match("*", "b"));
        assert!(!glob_match("*", "a/a"));
        assert!(!glob_match("*", "a/a/a"));
        assert!(!glob_match("*", "a/a/b"));
        assert!(!glob_match("*", "a/a/a/a"));
        assert!(!glob_match("*", "a/a/a/a/a"));

        assert!(!glob_match("*/*", "a"));
        assert!(glob_match("*/*", "a/a"));
        assert!(!glob_match("*/*", "a/a/a"));

        assert!(!glob_match("*/*/*", "a"));
        assert!(!glob_match("*/*/*", "a/a"));
        assert!(glob_match("*/*/*", "a/a/a"));
        assert!(!glob_match("*/*/*", "a/a/a/a"));
    }

    #[test]
    fn stars_depth_matching_deep() {
        assert!(!glob_match("*/*/*/*", "a"));
        assert!(!glob_match("*/*/*/*", "a/a"));
        assert!(!glob_match("*/*/*/*", "a/a/a"));
        assert!(glob_match("*/*/*/*", "a/a/a/a"));
        assert!(!glob_match("*/*/*/*", "a/a/a/a/a"));

        assert!(!glob_match("*/*/*/*/*", "a"));
        assert!(!glob_match("*/*/*/*/*", "a/a"));
        assert!(!glob_match("*/*/*/*/*", "a/a/a"));
        assert!(!glob_match("*/*/*/*/*", "a/a/b"));
        assert!(!glob_match("*/*/*/*/*", "a/a/a/a"));
        assert!(glob_match("*/*/*/*/*", "a/a/a/a/a"));
        assert!(!glob_match("*/*/*/*/*", "a/a/a/a/a/a"));
    }

    #[test]
    fn stars_prefix_depth_single() {
        assert!(!glob_match("a/*", "a"));
        assert!(glob_match("a/*", "a/a"));
        assert!(!glob_match("a/*", "a/a/a"));
        assert!(!glob_match("a/*", "a/a/a/a"));
        assert!(!glob_match("a/*", "a/a/a/a/a"));

        assert!(!glob_match("a/*/*", "a"));
        assert!(!glob_match("a/*/*", "a/a"));
        assert!(glob_match("a/*/*", "a/a/a"));
        assert!(!glob_match("a/*/*", "b/a/a"));
        assert!(!glob_match("a/*/*", "a/a/a/a"));
        assert!(!glob_match("a/*/*", "a/a/a/a/a"));
    }

    #[test]
    fn stars_prefix_depth_multi() {
        assert!(!glob_match("a/*/*/*", "a"));
        assert!(!glob_match("a/*/*/*", "a/a"));
        assert!(!glob_match("a/*/*/*", "a/a/a"));
        assert!(glob_match("a/*/*/*", "a/a/a/a"));
        assert!(!glob_match("a/*/*/*", "a/a/a/a/a"));

        assert!(!glob_match("a/*/*/*/*", "a"));
        assert!(!glob_match("a/*/*/*/*", "a/a"));
        assert!(!glob_match("a/*/*/*/*", "a/a/a"));
        assert!(!glob_match("a/*/*/*/*", "a/a/b"));
        assert!(!glob_match("a/*/*/*/*", "a/a/a/a"));
        assert!(glob_match("a/*/*/*/*", "a/a/a/a/a"));
    }

    #[test]
    fn stars_prefix_depth_named() {
        assert!(!glob_match("a/*/a", "a"));
        assert!(!glob_match("a/*/a", "a/a"));
        assert!(glob_match("a/*/a", "a/a/a"));
        assert!(!glob_match("a/*/a", "a/a/b"));
        assert!(!glob_match("a/*/a", "a/a/a/a"));
        assert!(!glob_match("a/*/a", "a/a/a/a/a"));

        assert!(!glob_match("a/*/b", "a"));
        assert!(!glob_match("a/*/b", "a/a"));
        assert!(!glob_match("a/*/b", "a/a/a"));
        assert!(glob_match("a/*/b", "a/a/b"));
        assert!(!glob_match("a/*/b", "a/a/a/a"));
        assert!(!glob_match("a/*/b", "a/a/a/a/a"));

        assert!(!glob_match("*/**/a", "a"));
        assert!(!glob_match("*/**/a", "a/a/b"));
        assert!(glob_match("*/**/a", "a/a"));
        assert!(glob_match("*/**/a", "a/a/a"));
        assert!(glob_match("*/**/a", "a/a/a/a"));
        assert!(glob_match("*/**/a", "a/a/a/a/a"));
    }

    #[test]
    fn stars_trailing_slash() {
        assert!(!glob_match("*/", "a"));
        assert!(!glob_match("*/*", "a"));
        assert!(!glob_match("a/*", "a"));
        // assert!(!glob_match("*/*", "a/"));
        // assert!(!glob_match("a/*", "a/"));
        assert!(!glob_match("*", "a/a"));
        assert!(!glob_match("*/", "a/a"));
        assert!(!glob_match("*/", "a/x/y"));
        assert!(!glob_match("*/*", "a/x/y"));
        assert!(!glob_match("a/*", "a/x/y"));
        // assert!(glob_match("*", "a/"));
        assert!(glob_match("*", "a"));
        assert!(glob_match("*/", "a/"));
        assert!(glob_match("*{,/}", "a/"));
        assert!(glob_match("*/*", "a/a"));
        assert!(glob_match("a/*", "a/a"));
    }

    #[test]
    fn stars_txt_patterns() {
        assert!(!glob_match("a/**/*.txt", "a.txt"));
        assert!(glob_match("a/**/*.txt", "a/x/y.txt"));
        assert!(!glob_match("a/**/*.txt", "a/x/y/z"));

        assert!(!glob_match("a/*.txt", "a.txt"));
        assert!(glob_match("a/*.txt", "a/b.txt"));
        assert!(!glob_match("a/*.txt", "a/x/y.txt"));
        assert!(!glob_match("a/*.txt", "a/x/y/z"));

        assert!(glob_match("a*.txt", "a.txt"));
        assert!(!glob_match("a*.txt", "a/b.txt"));
        assert!(!glob_match("a*.txt", "a/x/y.txt"));
        assert!(!glob_match("a*.txt", "a/x/y/z"));

        assert!(glob_match("*.txt", "a.txt"));
        assert!(!glob_match("*.txt", "a/b.txt"));
        assert!(!glob_match("*.txt", "a/x/y.txt"));
        assert!(!glob_match("*.txt", "a/x/y/z"));

        assert!(!glob_match("a*", "a/b"));
        assert!(!glob_match("a/**/b", "a/a/bb"));
        assert!(!glob_match("a/**/b", "a/bb"));

        assert!(!glob_match("*/**", "foo"));
        assert!(!glob_match("**/", "foo/bar"));
        assert!(!glob_match("**/*/", "foo/bar"));
        assert!(!glob_match("*/*/", "foo/bar"));
    }

    #[test]
    fn stars_doublestar_paths_positive() {
        assert!(glob_match("**/..", "/home/foo/.."));
        assert!(glob_match("**/a", "a"));
        assert!(glob_match("**", "a/a"));
        assert!(glob_match("a/**", "a/a"));
        assert!(glob_match("a/**", "a/"));
        // assert!(glob_match("a/**", "a"));
        assert!(!glob_match("**/", "a/a"));
        // assert!(glob_match("**/a/**", "a"));
        // assert!(glob_match("a/**", "a"));
        assert!(!glob_match("**/", "a/a"));
        assert!(glob_match("*/**/a", "a/a"));
        // assert!(glob_match("a/**", "a"));
        assert!(glob_match("*/**", "foo/"));
        assert!(glob_match("**/*", "foo/bar"));
        assert!(glob_match("*/*", "foo/bar"));
        assert!(glob_match("*/**", "foo/bar"));
        assert!(glob_match("**/", "foo/bar/"));
        // assert!(glob_match("**/*", "foo/bar/"));
        assert!(glob_match("**/*/", "foo/bar/"));
        assert!(glob_match("*/**", "foo/bar/"));
        assert!(glob_match("*/*/", "foo/bar/"));
    }

    #[test]
    fn stars_doublestar_paths_negative() {
        assert!(!glob_match("*/foo", "bar/baz/foo"));
        assert!(!glob_match("**/bar/*", "deep/foo/bar"));
        assert!(!glob_match("*/bar/**", "deep/foo/bar/baz/x"));
        assert!(!glob_match("/*", "ef"));
        assert!(!glob_match("foo?bar", "foo/bar"));
        assert!(!glob_match("**/bar*", "foo/bar/baz"));
        // assert!(!glob_match("**/bar**", "foo/bar/baz"));
        assert!(!glob_match("foo**bar", "foo/baz/bar"));
        assert!(!glob_match("foo*bar", "foo/baz/bar"));
        // assert!(glob_match("foo/**", "foo"));
        assert!(glob_match("/*", "/ab"));
        assert!(glob_match("/*", "/cd"));
        assert!(glob_match("/*", "/ef"));
        assert!(glob_match("a/**/j/**/z/*.md", "a/b/j/c/z/x.md"));
        assert!(glob_match("a/**/j/**/z/*.md", "a/j/z/x.md"));
    }

    #[test]
    fn stars_doublestar_paths_deep() {
        assert!(glob_match("**/foo", "bar/baz/foo"));
        assert!(glob_match("**/bar/*", "deep/foo/bar/baz"));
        assert!(glob_match("**/bar/**", "deep/foo/bar/baz/"));
        assert!(glob_match("**/bar/*/*", "deep/foo/bar/baz/x"));
        assert!(glob_match("foo/**/**/bar", "foo/b/a/z/bar"));
        assert!(glob_match("foo/**/bar", "foo/b/a/z/bar"));
        assert!(glob_match("foo/**/**/bar", "foo/bar"));
        assert!(glob_match("foo/**/bar", "foo/bar"));
        assert!(glob_match("*/bar/**", "foo/bar/baz/x"));
        assert!(glob_match("foo/**/**/bar", "foo/baz/bar"));
        assert!(glob_match("foo/**/bar", "foo/baz/bar"));
        assert!(glob_match("**/foo", "XXX/foo"));
    }

    #[test]
    fn globstars_js() {
        assert!(glob_match("**/*.js", "a/b/c/d.js"));
        assert!(glob_match("**/*.js", "a/b/c.js"));
        assert!(glob_match("**/*.js", "a/b.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/c/d/e/f.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/c/d/e.js"));
        assert!(glob_match("a/b/c/**/*.js", "a/b/c/d.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/c/d.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/d.js"));
        assert!(!glob_match("a/b/**/*.js", "a/d.js"));
        assert!(!glob_match("a/b/**/*.js", "d.js"));

        assert!(!glob_match("**c", "a/b/c"));
        assert!(!glob_match("a/**c", "a/b/c"));
        assert!(!glob_match("a/**z", "a/b/c"));
        assert!(!glob_match("a/**b**/c", "a/b/c/b/c"));
        assert!(!glob_match("a/b/c**/*.js", "a/b/c/d/e.js"));
        assert!(glob_match("a/**/b/**/c", "a/b/c/b/c"));
        assert!(glob_match("a/**b**/c", "a/aba/c"));
        assert!(glob_match("a/**b**/c", "a/b/c"));
        assert!(glob_match("a/b/c**/*.js", "a/b/c/d.js"));
    }

    #[test]
    fn globstars_depth_negative() {
        assert!(!glob_match("a/**/*", "a"));
        assert!(!glob_match("a/**/**/*", "a"));
        assert!(!glob_match("a/**/**/**/*", "a"));
        assert!(!glob_match("**/a", "a/"));
        assert!(glob_match("a/**/*", "a/"));
        assert!(glob_match("a/**/**/*", "a/"));
        assert!(glob_match("a/**/**/**/*", "a/"));
        assert!(!glob_match("**/a", "a/b"));
        assert!(!glob_match("a/**/j/**/z/*.md", "a/b/c/j/e/z/c.txt"));
        assert!(!glob_match("a/**/b", "a/bb"));
        assert!(!glob_match("**/a", "a/c"));
        assert!(!glob_match("**/a", "a/b"));
        assert!(!glob_match("**/a", "a/x/y"));
        assert!(!glob_match("**/a", "a/b/c/d"));
    }

    #[test]
    fn globstars_depth_positive() {
        assert!(glob_match("**", "a"));
        assert!(glob_match("**/a", "a"));
        // assert!(glob_match("a/**", "a"));
        assert!(glob_match("**", "a/"));
        assert!(glob_match("**/a/**", "a/"));
        assert!(glob_match("a/**", "a/"));
        assert!(glob_match("a/**/**", "a/"));
        assert!(glob_match("**/a", "a/a"));
        assert!(glob_match("**", "a/b"));
        assert!(glob_match("*/*", "a/b"));
        assert!(glob_match("a/**", "a/b"));
        assert!(glob_match("a/**/*", "a/b"));
        assert!(glob_match("a/**/**/*", "a/b"));
        assert!(glob_match("a/**/**/**/*", "a/b"));
        assert!(glob_match("a/**/b", "a/b"));
    }

    #[test]
    fn globstars_depth_deep() {
        assert!(glob_match("**", "a/b/c"));
        assert!(glob_match("**/*", "a/b/c"));
        assert!(glob_match("**/**", "a/b/c"));
        assert!(glob_match("*/**", "a/b/c"));
        assert!(glob_match("a/**", "a/b/c"));
        assert!(glob_match("a/**/*", "a/b/c"));
        assert!(glob_match("a/**/**/*", "a/b/c"));
        assert!(glob_match("a/**/**/**/*", "a/b/c"));
        assert!(glob_match("**", "a/b/c/d"));
        assert!(glob_match("a/**", "a/b/c/d"));
        assert!(glob_match("a/**/*", "a/b/c/d"));
        assert!(glob_match("a/**/**/*", "a/b/c/d"));
        assert!(glob_match("a/**/**/**/*", "a/b/c/d"));
    }

    #[test]
    fn globstars_deep_paths_md() {
        assert!(glob_match("a/b/**/c/**/*.*", "a/b/c/d.e"));
        assert!(glob_match("a/**/f/*.md", "a/b/c/d/e/f/g.md"));
        assert!(glob_match("a/**/f/**/k/*.md", "a/b/c/d/e/f/g/h/i/j/k/l.md"));
        assert!(glob_match("a/b/c/*.md", "a/b/c/def.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb.bb/c/ddd.md"));
        assert!(glob_match("a/**/f/*.md", "a/bb.bb/cc/d.d/ee/f/ggg.md"));
        assert!(glob_match("a/**/f/*.md", "a/bb.bb/cc/dd/ee/f/ggg.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb/c/ddd.md"));
        assert!(glob_match("a/*/c/*.md", "a/bbbb/c/ddd.md"));

        assert!(glob_match(
            "foo/bar/**/one/**/*.*",
            "foo/bar/baz/one/image.png"
        ));
        assert!(glob_match(
            "foo/bar/**/one/**/*.*",
            "foo/bar/baz/one/two/image.png"
        ));
        assert!(glob_match(
            "foo/bar/**/one/**/*.*",
            "foo/bar/baz/one/two/three/image.png"
        ));
        assert!(!glob_match("a/b/**/f", "a/b/c/d/"));
    }

    #[test]
    fn globstars_deep_paths_general() {
        // assert!(glob_match("a/**", "a"));
        assert!(glob_match("**", "a"));
        assert!(glob_match("a{,/**}", "a"));
        assert!(glob_match("**", "a/"));
        assert!(glob_match("a/**", "a/"));
        assert!(glob_match("**", "a/b/c/d"));
        assert!(glob_match("**", "a/b/c/d/"));
        assert!(glob_match("**/**", "a/b/c/d/"));
        assert!(glob_match("**/b/**", "a/b/c/d/"));
        assert!(glob_match("a/b/**", "a/b/c/d/"));
        assert!(glob_match("a/b/**/", "a/b/c/d/"));
        assert!(glob_match("a/b/**/c/**/", "a/b/c/d/"));
        assert!(glob_match("a/b/**/c/**/d/", "a/b/c/d/"));
        assert!(glob_match("a/b/**/**/*.*", "a/b/c/d/e.f"));
        assert!(glob_match("a/b/**/*.*", "a/b/c/d/e.f"));
        assert!(glob_match("a/b/**/c/**/d/*.*", "a/b/c/d/e.f"));
        assert!(glob_match("a/b/**/d/**/*.*", "a/b/c/d/e.f"));
        assert!(glob_match("a/b/**/d/**/*.*", "a/b/c/d/g/e.f"));
        assert!(glob_match("a/b/**/d/**/*.*", "a/b/c/d/g/g/e.f"));
        assert!(glob_match("a/b-*/**/z.js", "a/b-c/z.js"));
        assert!(glob_match("a/b-*/**/z.js", "a/b-c/d/e/z.js"));
    }

    #[test]
    fn globstars_mixed_positive() {
        assert!(glob_match("*/*", "a/b"));
        assert!(glob_match("a/b/c/*.md", "a/b/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb.bb/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bb/c/xyz.md"));
        assert!(glob_match("a/*/c/*.md", "a/bbbb/c/xyz.md"));

        assert!(glob_match("**/*", "a/b/c"));
        assert!(glob_match("**/**", "a/b/c"));
        assert!(glob_match("*/**", "a/b/c"));
        assert!(glob_match("a/**/j/**/z/*.md", "a/b/c/d/e/j/n/p/o/z/c.md"));
        assert!(glob_match("a/**/z/*.md", "a/b/c/d/e/z/c.md"));
        assert!(glob_match("a/**/c/*.md", "a/bb.bb/aa/b.b/aa/c/xyz.md"));
        assert!(glob_match("a/**/c/*.md", "a/bb.bb/aa/bb/aa/c/xyz.md"));
        assert!(!glob_match("a/**/j/**/z/*.md", "a/b/c/j/e/z/c.txt"));
        assert!(!glob_match("a/b/**/c{d,e}/**/xyz.md", "a/b/c/xyz.md"));
        assert!(!glob_match("a/b/**/c{d,e}/**/xyz.md", "a/b/d/xyz.md"));
        assert!(!glob_match("a/**/", "a/b"));
        // assert!(!glob_match("**/*", "a/b/.js/c.txt"));
        assert!(!glob_match("a/**/", "a/b/c/d"));
        assert!(!glob_match("a/**/", "a/bb"));
        assert!(!glob_match("a/**/", "a/cb"));
    }

    #[test]
    fn globstars_mixed_js_and_paths() {
        assert!(glob_match("/**", "/a/b"));
        assert!(glob_match("**/*", "a.b"));
        assert!(glob_match("**/*", "a.js"));
        assert!(glob_match("**/*.js", "a.js"));
        // assert!(glob_match("a/**/", "a/"));
        assert!(glob_match("**/*.js", "a/a.js"));
        assert!(glob_match("**/*.js", "a/a/b.js"));
        assert!(glob_match("a/**/b", "a/b"));
        assert!(glob_match("a/**b", "a/b"));
        assert!(glob_match("**/*.md", "a/b.md"));
        assert!(glob_match("**/*", "a/b/c.js"));
        assert!(glob_match("**/*", "a/b/c.txt"));
        assert!(glob_match("a/**/", "a/b/c/d/"));
        assert!(glob_match("**/*", "a/b/c/d/a.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/c/z.js"));
        assert!(glob_match("a/b/**/*.js", "a/b/z.js"));
        assert!(glob_match("**/*", "ab"));
        assert!(glob_match("**/*", "ab/c"));
        assert!(glob_match("**/*", "ab/c/d"));
        assert!(glob_match("**/*", "abc.js"));
    }

    #[test]
    fn globstars_negative_and_positive_neg() {
        assert!(!glob_match("**/", "a"));
        assert!(!glob_match("**/a/*", "a"));
        assert!(!glob_match("**/a/*/*", "a"));
        assert!(!glob_match("*/a/**", "a"));
        assert!(!glob_match("a/**/*", "a"));
        assert!(!glob_match("a/**/**/*", "a"));
        assert!(!glob_match("**/", "a/b"));
        assert!(!glob_match("**/b/*", "a/b"));
        assert!(!glob_match("**/b/*/*", "a/b"));
        assert!(!glob_match("b/**", "a/b"));
        assert!(!glob_match("**/", "a/b/c"));
        assert!(!glob_match("**/**/b", "a/b/c"));
        assert!(!glob_match("**/b", "a/b/c"));
        assert!(!glob_match("**/b/*/*", "a/b/c"));
        assert!(!glob_match("b/**", "a/b/c"));
        assert!(!glob_match("**/", "a/b/c/d"));
        assert!(!glob_match("**/d/*", "a/b/c/d"));
        assert!(!glob_match("b/**", "a/b/c/d"));
    }

    #[test]
    fn globstars_negative_and_positive_pos_a() {
        assert!(glob_match("**", "a"));
        assert!(glob_match("**/**", "a"));
        assert!(glob_match("**/**/*", "a"));
        assert!(glob_match("**/**/a", "a"));
        assert!(glob_match("**/a", "a"));
        // assert!(glob_match("**/a/**", "a"));
        // assert!(glob_match("a/**", "a"));
        assert!(glob_match("**", "a/b"));
        assert!(glob_match("**/**", "a/b"));
        assert!(glob_match("**/**/*", "a/b"));
        assert!(glob_match("**/**/b", "a/b"));
        assert!(glob_match("**/b", "a/b"));
        // assert!(glob_match("**/b/**", "a/b"));
        // assert!(glob_match("*/b/**", "a/b"));
        assert!(glob_match("a/**", "a/b"));
        assert!(glob_match("a/**/*", "a/b"));
        assert!(glob_match("a/**/**/*", "a/b"));
    }

    #[test]
    fn globstars_negative_and_positive_pos_deep() {
        assert!(glob_match("**", "a/b/c"));
        assert!(glob_match("**/**", "a/b/c"));
        assert!(glob_match("**/**/*", "a/b/c"));
        assert!(glob_match("**/b/*", "a/b/c"));
        assert!(glob_match("**/b/**", "a/b/c"));
        assert!(glob_match("*/b/**", "a/b/c"));
        assert!(glob_match("a/**", "a/b/c"));
        assert!(glob_match("a/**/*", "a/b/c"));
        assert!(glob_match("a/**/**/*", "a/b/c"));
        assert!(glob_match("**", "a/b/c/d"));
        assert!(glob_match("**/**", "a/b/c/d"));
        assert!(glob_match("**/**/*", "a/b/c/d"));
        assert!(glob_match("**/**/d", "a/b/c/d"));
        assert!(glob_match("**/b/**", "a/b/c/d"));
        assert!(glob_match("**/b/*/*", "a/b/c/d"));
        assert!(glob_match("**/d", "a/b/c/d"));
        assert!(glob_match("*/b/**", "a/b/c/d"));
        assert!(glob_match("a/**", "a/b/c/d"));
        assert!(glob_match("a/**/*", "a/b/c/d"));
        assert!(glob_match("a/**/**/*", "a/b/c/d"));
    }

    #[test]
    fn utf8() {
        assert!(glob_match("フ*/**/*", "フォルダ/aaa.js"));
        assert!(glob_match("フォ*/**/*", "フォルダ/aaa.js"));
        assert!(glob_match("フォル*/**/*", "フォルダ/aaa.js"));
        assert!(glob_match("フ*ル*/**/*", "フォルダ/aaa.js"));
        assert!(glob_match("フォルダ/**/*", "フォルダ/aaa.js"));
    }

    #[test]
    fn negation_basic() {
        assert!(!glob_match("!*", "abc"));
        assert!(!glob_match("!abc", "abc"));
        assert!(!glob_match("*!.md", "bar.md"));
        assert!(!glob_match("foo!.md", "bar.md"));
        assert!(!glob_match("\\!*!*.md", "foo!.md"));
        assert!(!glob_match("\\!*!*.md", "foo!bar.md"));
        assert!(glob_match("*!*.md", "!foo!.md"));
        assert!(glob_match("\\!*!*.md", "!foo!.md"));
        assert!(glob_match("!*foo", "abc"));
        assert!(glob_match("!foo*", "abc"));
        assert!(glob_match("!xyz", "abc"));
        assert!(glob_match("*!*.*", "ba!r.js"));
        assert!(glob_match("*.md", "bar.md"));
        assert!(glob_match("*!*.*", "foo!.md"));
        assert!(glob_match("*!*.md", "foo!.md"));
        assert!(glob_match("*!.md", "foo!.md"));
        assert!(glob_match("*.md", "foo!.md"));
        assert!(glob_match("foo!.md", "foo!.md"));
        assert!(glob_match("*!*.md", "foo!bar.md"));
        assert!(glob_match("*b*.md", "foobar.md"));
    }

    #[test]
    fn negation_double_bang() {
        assert!(!glob_match("a!!b", "a"));
        assert!(!glob_match("a!!b", "aa"));
        assert!(!glob_match("a!!b", "a/b"));
        assert!(!glob_match("a!!b", "a!b"));
        assert!(glob_match("a!!b", "a!!b"));
        assert!(!glob_match("a!!b", "a/!!/b"));
    }

    #[test]
    fn negation_path() {
        assert!(!glob_match("!a/b", "a/b"));
        assert!(glob_match("!a/b", "a"));
        assert!(glob_match("!a/b", "a.b"));
        assert!(glob_match("!a/b", "a/a"));
        assert!(glob_match("!a/b", "a/c"));
        assert!(glob_match("!a/b", "b/a"));
        assert!(glob_match("!a/b", "b/b"));
        assert!(glob_match("!a/b", "b/c"));
    }

    #[test]
    fn negation_multiple_bangs() {
        assert!(!glob_match("!abc", "abc"));
        assert!(glob_match("!!abc", "abc"));
        assert!(!glob_match("!!!abc", "abc"));
        assert!(glob_match("!!!!abc", "abc"));
        assert!(!glob_match("!!!!!abc", "abc"));
        assert!(glob_match("!!!!!!abc", "abc"));
        assert!(!glob_match("!!!!!!!abc", "abc"));
        assert!(glob_match("!!!!!!!!abc", "abc"));
    }

    #[test]
    fn negation_star_slash_negative() {
        // assert!(!glob_match("!(*/*)", "a/a"));
        // assert!(!glob_match("!(*/*)", "a/b"));
        // assert!(!glob_match("!(*/*)", "a/c"));
        // assert!(!glob_match("!(*/*)", "b/a"));
        // assert!(!glob_match("!(*/*)", "b/b"));
        // assert!(!glob_match("!(*/*)", "b/c"));
        // assert!(!glob_match("!(*/b)", "a/b"));
        // assert!(!glob_match("!(*/b)", "b/b"));
        // assert!(!glob_match("!(a/b)", "a/b"));
        assert!(!glob_match("!*", "a"));
        assert!(!glob_match("!*", "a.b"));
        assert!(!glob_match("!*/*", "a/a"));
        assert!(!glob_match("!*/*", "a/b"));
        assert!(!glob_match("!*/*", "a/c"));
        assert!(!glob_match("!*/*", "b/a"));
        assert!(!glob_match("!*/*", "b/b"));
        assert!(!glob_match("!*/*", "b/c"));
        assert!(!glob_match("!*/b", "a/b"));
        assert!(!glob_match("!*/b", "b/b"));
        assert!(!glob_match("!*/c", "a/c"));
        assert!(!glob_match("!*/c", "a/c"));
        assert!(!glob_match("!*/c", "b/c"));
        assert!(!glob_match("!*/c", "b/c"));
        assert!(!glob_match("!*a*", "bar"));
        assert!(!glob_match("!*a*", "fab"));
    }

    #[test]
    fn negation_a_slash_negative() {
        // assert!(!glob_match("!a/(*)", "a/a"));
        // assert!(!glob_match("!a/(*)", "a/b"));
        // assert!(!glob_match("!a/(*)", "a/c"));
        // assert!(!glob_match("!a/(b)", "a/b"));
        assert!(!glob_match("!a/*", "a/a"));
        assert!(!glob_match("!a/*", "a/b"));
        assert!(!glob_match("!a/*", "a/c"));
        assert!(!glob_match("!f*b", "fab"));
    }

    #[test]
    fn negation_star_slash_positive() {
        // assert!(glob_match("!(*/*)", "a"));
        // assert!(glob_match("!(*/*)", "a.b"));
        // assert!(glob_match("!(*/b)", "a"));
        // assert!(glob_match("!(*/b)", "a.b"));
        // assert!(glob_match("!(*/b)", "a/a"));
        // assert!(glob_match("!(*/b)", "a/c"));
        // assert!(glob_match("!(*/b)", "b/a"));
        // assert!(glob_match("!(*/b)", "b/c"));
        // assert!(glob_match("!(a/b)", "a"));
        // assert!(glob_match("!(a/b)", "a.b"));
        // assert!(glob_match("!(a/b)", "a/a"));
        // assert!(glob_match("!(a/b)", "a/c"));
        // assert!(glob_match("!(a/b)", "b/a"));
        // assert!(glob_match("!(a/b)", "b/b"));
        // assert!(glob_match("!(a/b)", "b/c"));
        assert!(glob_match("!*", "a/a"));
        assert!(glob_match("!*", "a/b"));
        assert!(glob_match("!*", "a/c"));
        assert!(glob_match("!*", "b/a"));
        assert!(glob_match("!*", "b/b"));
        assert!(glob_match("!*", "b/c"));
        assert!(glob_match("!*/*", "a"));
        assert!(glob_match("!*/*", "a.b"));
        assert!(glob_match("!*/b", "a"));
        assert!(glob_match("!*/b", "a.b"));
        assert!(glob_match("!*/b", "a/a"));
        assert!(glob_match("!*/b", "a/c"));
        assert!(glob_match("!*/b", "b/a"));
        assert!(glob_match("!*/b", "b/c"));
        assert!(glob_match("!*/c", "a"));
        assert!(glob_match("!*/c", "a.b"));
        assert!(glob_match("!*/c", "a/a"));
        assert!(glob_match("!*/c", "a/b"));
        assert!(glob_match("!*/c", "b/a"));
        assert!(glob_match("!*/c", "b/b"));
        assert!(glob_match("!*a*", "foo"));
    }

    #[test]
    fn negation_a_slash_positive() {
        // assert!(glob_match("!a/(*)", "a"));
        // assert!(glob_match("!a/(*)", "a.b"));
        // assert!(glob_match("!a/(*)", "b/a"));
        // assert!(glob_match("!a/(*)", "b/b"));
        // assert!(glob_match("!a/(*)", "b/c"));
        // assert!(glob_match("!a/(b)", "a"));
        // assert!(glob_match("!a/(b)", "a.b"));
        // assert!(glob_match("!a/(b)", "a/a"));
        // assert!(glob_match("!a/(b)", "a/c"));
        // assert!(glob_match("!a/(b)", "b/a"));
        // assert!(glob_match("!a/(b)", "b/b"));
        // assert!(glob_match("!a/(b)", "b/c"));
        assert!(glob_match("!a/*", "a"));
        assert!(glob_match("!a/*", "a.b"));
        assert!(glob_match("!a/*", "b/a"));
        assert!(glob_match("!a/*", "b/b"));
        assert!(glob_match("!a/*", "b/c"));
        assert!(glob_match("!f*b", "bar"));
        assert!(glob_match("!f*b", "foo"));
    }

    #[test]
    fn negation_md_extension() {
        assert!(!glob_match("!.md", ".md"));
        assert!(glob_match("!**/*.md", "a.js"));
        // assert!(!glob_match("!**/*.md", "b.md"));
        assert!(glob_match("!**/*.md", "c.txt"));
        assert!(glob_match("!*.md", "a.js"));
        assert!(!glob_match("!*.md", "b.md"));
        assert!(glob_match("!*.md", "c.txt"));
        assert!(!glob_match("!*.md", "abc.md"));
        assert!(glob_match("!*.md", "abc.txt"));
        assert!(!glob_match("!*.md", "foo.md"));
        assert!(glob_match("!.md", "foo.md"));
    }

    #[test]
    fn negation_path_patterns() {
        assert!(glob_match("!*.md", "a.js"));
        assert!(glob_match("!*.md", "b.txt"));
        assert!(!glob_match("!*.md", "c.md"));
        assert!(!glob_match("!a/*/a.js", "a/a/a.js"));
        assert!(!glob_match("!a/*/a.js", "a/b/a.js"));
        assert!(!glob_match("!a/*/a.js", "a/c/a.js"));
        assert!(!glob_match("!a/*/*/a.js", "a/a/a/a.js"));
        assert!(glob_match("!a/*/*/a.js", "b/a/b/a.js"));
        assert!(glob_match("!a/*/*/a.js", "c/a/c/a.js"));
        assert!(!glob_match("!a/a*.txt", "a/a.txt"));
        assert!(glob_match("!a/a*.txt", "a/b.txt"));
        assert!(glob_match("!a/a*.txt", "a/c.txt"));
        assert!(!glob_match("!a.a*.txt", "a.a.txt"));
        assert!(glob_match("!a.a*.txt", "a.b.txt"));
        assert!(glob_match("!a.a*.txt", "a.c.txt"));
        assert!(!glob_match("!a/*.txt", "a/a.txt"));
        assert!(!glob_match("!a/*.txt", "a/b.txt"));
        assert!(!glob_match("!a/*.txt", "a/c.txt"));
    }

    #[test]
    fn negation_globstar_md() {
        assert!(glob_match("!*.md", "a.js"));
        assert!(glob_match("!*.md", "b.txt"));
        assert!(!glob_match("!*.md", "c.md"));
        // assert!(!glob_match("!**/a.js", "a/a/a.js"));
        // assert!(!glob_match("!**/a.js", "a/b/a.js"));
        // assert!(!glob_match("!**/a.js", "a/c/a.js"));
        assert!(glob_match("!**/a.js", "a/a/b.js"));
        assert!(!glob_match("!a/**/a.js", "a/a/a/a.js"));
        assert!(glob_match("!a/**/a.js", "b/a/b/a.js"));
        assert!(glob_match("!a/**/a.js", "c/a/c/a.js"));
        assert!(glob_match("!**/*.md", "a/b.js"));
        assert!(glob_match("!**/*.md", "a.js"));
        assert!(!glob_match("!**/*.md", "a/b.md"));
        // assert!(!glob_match("!**/*.md", "a.md"));
        assert!(!glob_match("**/*.md", "a/b.js"));
        assert!(!glob_match("**/*.md", "a.js"));
        assert!(glob_match("**/*.md", "a/b.md"));
        assert!(glob_match("**/*.md", "a.md"));
        assert!(glob_match("!**/*.md", "a/b.js"));
        assert!(glob_match("!**/*.md", "a.js"));
        assert!(!glob_match("!**/*.md", "a/b.md"));
        // assert!(!glob_match("!**/*.md", "a.md"));
        assert!(glob_match("!*.md", "a/b.js"));
        assert!(glob_match("!*.md", "a.js"));
        assert!(glob_match("!*.md", "a/b.md"));
        assert!(!glob_match("!*.md", "a.md"));
        assert!(glob_match("!**/*.md", "a.js"));
        // assert!(!glob_match("!**/*.md", "b.md"));
        assert!(glob_match("!**/*.md", "c.txt"));
    }

    #[test]
    fn question_mark_single_multi() {
        assert!(glob_match("?", "a"));
        assert!(!glob_match("?", "aa"));
        assert!(!glob_match("?", "ab"));
        assert!(!glob_match("?", "aaa"));
        assert!(!glob_match("?", "abcdefg"));

        assert!(!glob_match("??", "a"));
        assert!(glob_match("??", "aa"));
        assert!(glob_match("??", "ab"));
        assert!(!glob_match("??", "aaa"));
        assert!(!glob_match("??", "abcdefg"));

        assert!(!glob_match("???", "a"));
        assert!(!glob_match("???", "aa"));
        assert!(!glob_match("???", "ab"));
        assert!(glob_match("???", "aaa"));
        assert!(!glob_match("???", "abcdefg"));
    }

    #[test]
    fn question_mark_with_literals() {
        assert!(!glob_match("a?c", "aaa"));
        assert!(glob_match("a?c", "aac"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("ab?", "a"));
        assert!(!glob_match("ab?", "aa"));
        assert!(!glob_match("ab?", "ab"));
        assert!(!glob_match("ab?", "ac"));
        assert!(!glob_match("ab?", "abcd"));
        assert!(!glob_match("ab?", "abbb"));
        assert!(glob_match("a?b", "acb"));
    }

    #[test]
    fn question_mark_paths() {
        assert!(!glob_match("a/?/c/?/e.md", "a/bb/c/dd/e.md"));
        assert!(glob_match("a/??/c/??/e.md", "a/bb/c/dd/e.md"));
        assert!(!glob_match("a/??/c.md", "a/bbb/c.md"));
        assert!(glob_match("a/?/c.md", "a/b/c.md"));
        assert!(glob_match("a/?/c/?/e.md", "a/b/c/d/e.md"));
        assert!(!glob_match("a/?/c/???/e.md", "a/b/c/d/e.md"));
        assert!(glob_match("a/?/c/???/e.md", "a/b/c/zzz/e.md"));
        assert!(!glob_match("a/?/c.md", "a/bb/c.md"));
        assert!(glob_match("a/??/c.md", "a/bb/c.md"));
        assert!(glob_match("a/???/c.md", "a/bbb/c.md"));
        assert!(glob_match("a/????/c.md", "a/bbbb/c.md"));
    }

    #[test]
    fn braces_basic() {
        assert!(glob_match("{a,b,c}", "a"));
        assert!(glob_match("{a,b,c}", "b"));
        assert!(glob_match("{a,b,c}", "c"));
        assert!(!glob_match("{a,b,c}", "aa"));
        assert!(!glob_match("{a,b,c}", "bb"));
        assert!(!glob_match("{a,b,c}", "cc"));

        assert!(glob_match("a/{a,b}", "a/a"));
        assert!(glob_match("a/{a,b}", "a/b"));
        assert!(!glob_match("a/{a,b}", "a/c"));
        assert!(!glob_match("a/{a,b}", "b/b"));
        assert!(!glob_match("a/{a,b,c}", "b/b"));
        assert!(glob_match("a/{a,b,c}", "a/c"));
        assert!(glob_match("a{b,bc}.txt", "abc.txt"));

        assert!(glob_match("foo[{a,b}]baz", "foo{baz"));
    }

    #[test]
    fn braces_empty_alternative() {
        assert!(!glob_match("a{,b}.txt", "abc.txt"));
        assert!(!glob_match("a{a,b,}.txt", "abc.txt"));
        assert!(!glob_match("a{b,}.txt", "abc.txt"));
        assert!(glob_match("a{,b}.txt", "a.txt"));
        assert!(glob_match("a{b,}.txt", "a.txt"));
        assert!(glob_match("a{a,b,}.txt", "aa.txt"));
        assert!(glob_match("a{a,b,}.txt", "aa.txt"));
        assert!(glob_match("a{,b}.txt", "ab.txt"));
        assert!(glob_match("a{b,}.txt", "ab.txt"));
    }

    #[test]
    fn braces_slash_alternatives() {
        // assert!(glob_match("{a/,}a/**", "a"));
        assert!(glob_match("a{a,b/}*.txt", "aa.txt"));
        assert!(glob_match("a{a,b/}*.txt", "ab/.txt"));
        assert!(glob_match("a{a,b/}*.txt", "ab/a.txt"));
        // assert!(glob_match("{a/,}a/**", "a/"));
        assert!(glob_match("{a/,}a/**", "a/a/"));
        // assert!(glob_match("{a/,}a/**", "a/a"));
        assert!(glob_match("{a/,}a/**", "a/a/a"));
        assert!(glob_match("{a/,}a/**", "a/a/"));
        assert!(glob_match("{a/,}a/**", "a/a/a/"));
        assert!(glob_match("{a/,}b/**", "a/b/a/"));
        assert!(glob_match("{a/,}b/**", "b/a/"));
        assert!(glob_match("a{,/}*.txt", "a.txt"));
        assert!(glob_match("a{,/}*.txt", "ab.txt"));
        assert!(glob_match("a{,/}*.txt", "a/b.txt"));
        assert!(glob_match("a{,/}*.txt", "a/ab.txt"));
    }

    #[test]
    fn braces_nested_foo_db() {
        assert!(glob_match("a{,.*{foo,db},\\(bar\\)}.txt", "a.txt"));
        assert!(!glob_match("a{,.*{foo,db},\\(bar\\)}.txt", "adb.txt"));
        assert!(glob_match("a{,.*{foo,db},\\(bar\\)}.txt", "a.db.txt"));

        assert!(glob_match("a{,*.{foo,db},\\(bar\\)}.txt", "a.txt"));
        assert!(!glob_match("a{,*.{foo,db},\\(bar\\)}.txt", "adb.txt"));
        assert!(glob_match("a{,*.{foo,db},\\(bar\\)}.txt", "a.db.txt"));

        assert!(glob_match("a{,.*{foo,db},\\(bar\\)}", "a"));
        assert!(!glob_match("a{,.*{foo,db},\\(bar\\)}", "adb"));
        assert!(glob_match("a{,.*{foo,db},\\(bar\\)}", "a.db"));

        assert!(glob_match("a{,*.{foo,db},\\(bar\\)}", "a"));
        assert!(!glob_match("a{,*.{foo,db},\\(bar\\)}", "adb"));
        assert!(glob_match("a{,*.{foo,db},\\(bar\\)}", "a.db"));

        assert!(!glob_match("{,.*{foo,db},\\(bar\\)}", "a"));
        assert!(!glob_match("{,.*{foo,db},\\(bar\\)}", "adb"));
        assert!(!glob_match("{,.*{foo,db},\\(bar\\)}", "a.db"));
        assert!(glob_match("{,.*{foo,db},\\(bar\\)}", ".db"));

        assert!(!glob_match("{,*.{foo,db},\\(bar\\)}", "a"));
        assert!(glob_match("{*,*.{foo,db},\\(bar\\)}", "a"));
        assert!(!glob_match("{,*.{foo,db},\\(bar\\)}", "adb"));
        assert!(glob_match("{,*.{foo,db},\\(bar\\)}", "a.db"));
    }

    #[test]
    fn braces_globstar_paths() {
        assert!(!glob_match("a/b/**/c{d,e}/**/xyz.md", "a/b/c/xyz.md"));
        assert!(!glob_match("a/b/**/c{d,e}/**/xyz.md", "a/b/d/xyz.md"));
        assert!(glob_match("a/b/**/c{d,e}/**/xyz.md", "a/b/cd/xyz.md"));
        assert!(glob_match("a/b/**/{c,d,e}/**/xyz.md", "a/b/c/xyz.md"));
        assert!(glob_match("a/b/**/{c,d,e}/**/xyz.md", "a/b/d/xyz.md"));
        assert!(glob_match("a/b/**/{c,d,e}/**/xyz.md", "a/b/e/xyz.md"));

        assert!(glob_match("*{a,b}*", "xax"));
        assert!(glob_match("*{a,b}*", "xxax"));
        assert!(glob_match("*{a,b}*", "xbx"));

        assert!(glob_match("*{*a,b}", "xba"));
        assert!(glob_match("*{*a,b}", "xb"));
    }

    #[test]
    fn braces_star_question_combos() {
        assert!(!glob_match("*??", "a"));
        assert!(!glob_match("*???", "aa"));
        assert!(glob_match("*???", "aaa"));
        assert!(!glob_match("*****??", "a"));
        assert!(!glob_match("*****???", "aa"));
        assert!(glob_match("*****???", "aaa"));

        assert!(!glob_match("a*?c", "aaa"));
        assert!(glob_match("a*?c", "aac"));
        assert!(glob_match("a*?c", "abc"));

        assert!(glob_match("a**?c", "abc"));
        assert!(!glob_match("a**?c", "abb"));
        assert!(glob_match("a**?c", "acc"));
        assert!(glob_match("a*****?c", "abc"));

        assert!(glob_match("*****?", "a"));
        assert!(glob_match("*****?", "aa"));
        assert!(glob_match("*****?", "abc"));
        assert!(glob_match("*****?", "zzz"));
        assert!(glob_match("*****?", "bbb"));
        assert!(glob_match("*****?", "aaaa"));
    }

    #[test]
    fn braces_star_question_combos_two() {
        assert!(!glob_match("*****??", "a"));
        assert!(glob_match("*****??", "aa"));
        assert!(glob_match("*****??", "abc"));
        assert!(glob_match("*****??", "zzz"));
        assert!(glob_match("*****??", "bbb"));
        assert!(glob_match("*****??", "aaaa"));

        assert!(!glob_match("?*****??", "a"));
        assert!(!glob_match("?*****??", "aa"));
        assert!(glob_match("?*****??", "abc"));
        assert!(glob_match("?*****??", "zzz"));
        assert!(glob_match("?*****??", "bbb"));
        assert!(glob_match("?*****??", "aaaa"));

        assert!(glob_match("?*****?c", "abc"));
        assert!(!glob_match("?*****?c", "abb"));
        assert!(!glob_match("?*****?c", "zzz"));

        assert!(glob_match("?***?****c", "abc"));
        assert!(!glob_match("?***?****c", "bbb"));
        assert!(!glob_match("?***?****c", "zzz"));
    }

    #[test]
    fn braces_complex_star_patterns() {
        assert!(glob_match("?***?****?", "abc"));
        assert!(glob_match("?***?****?", "bbb"));
        assert!(glob_match("?***?****?", "zzz"));

        assert!(glob_match("?***?****", "abc"));
        assert!(glob_match("*******c", "abc"));
        assert!(glob_match("*******?", "abc"));
        assert!(glob_match("a*cd**?**??k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??k***", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??***k", "abcdecdhjk"));
        assert!(glob_match("a**?**cd**?**??***k**", "abcdecdhjk"));
        assert!(glob_match("a****c**?**??*****", "abcdecdhjk"));
    }

    #[test]
    fn braces_question_path_patterns() {
        assert!(!glob_match("a/?/c/?/*/e.md", "a/b/c/d/e.md"));
        assert!(glob_match("a/?/c/?/*/e.md", "a/b/c/d/e/e.md"));
        assert!(glob_match("a/?/c/?/*/e.md", "a/b/c/d/efghijk/e.md"));
        assert!(glob_match("a/?/**/e.md", "a/b/c/d/efghijk/e.md"));
        assert!(!glob_match("a/?/e.md", "a/bb/e.md"));
        assert!(glob_match("a/??/e.md", "a/bb/e.md"));
        assert!(!glob_match("a/?/**/e.md", "a/bb/e.md"));
        assert!(glob_match("a/?/**/e.md", "a/b/ccc/e.md"));
        assert!(glob_match("a/*/?/**/e.md", "a/b/c/d/efghijk/e.md"));
        assert!(glob_match("a/*/?/**/e.md", "a/b/c/d/efgh.ijk/e.md"));
        assert!(glob_match("a/*/?/**/e.md", "a/b.bb/c/d/efgh.ijk/e.md"));
        assert!(glob_match("a/*/?/**/e.md", "a/bbb/c/d/efgh.ijk/e.md"));

        assert!(glob_match("a/*/ab??.md", "a/bbb/abcd.md"));
        assert!(glob_match("a/bbb/ab??.md", "a/bbb/abcd.md"));
        assert!(glob_match("a/bbb/ab???md", "a/bbb/abcd.md"));
    }

    fn test_captures<'a>(glob: &str, path: &'a str) -> Option<Vec<&'a str>> {
        glob_match_with_captures(glob, path)
            .map(|v| v.into_iter().map(|capture| &path[capture]).collect())
    }

    #[test]
    fn captures_basic() {
        assert_eq!(test_captures("a/b", "a/b"), Some(vec![]));
        assert_eq!(test_captures("a/*/c", "a/bx/c"), Some(vec!["bx"]));
        assert_eq!(test_captures("a/*/c", "a/test/c"), Some(vec!["test"]));
        assert_eq!(
            test_captures("a/*/c/*/e", "a/b/c/d/e"),
            Some(vec!["b", "d"])
        );
        assert_eq!(
            test_captures("a/*/c/*/e", "a/b/c/d/e"),
            Some(vec!["b", "d"])
        );
        assert_eq!(test_captures("a/{b,x}/c", "a/b/c"), Some(vec!["b"]));
        assert_eq!(test_captures("a/{b,x}/c", "a/x/c"), Some(vec!["x"]));
        assert_eq!(test_captures("a/?/c", "a/b/c"), Some(vec!["b"]));
        assert_eq!(test_captures("a/*?x/c", "a/yybx/c"), Some(vec!["yy", "b"]));
        assert_eq!(
            test_captures("a/*[a-z]x/c", "a/yybx/c"),
            Some(vec!["yy", "b"])
        );
        assert_eq!(
            test_captures("a/{b*c,c}y", "a/bdcy"),
            Some(vec!["bdc", "d"])
        );
        assert_eq!(test_captures("a/{b*,c}y", "a/bdy"), Some(vec!["bd", "d"]));
        assert_eq!(test_captures("a/{b*c,c}", "a/bdc"), Some(vec!["bdc", "d"]));
        assert_eq!(test_captures("a/{b*,c}", "a/bd"), Some(vec!["bd", "d"]));
        assert_eq!(test_captures("a/{b*,c}", "a/c"), Some(vec!["c", ""]));
        assert_eq!(
            test_captures("a/{b{c,d},c}y", "a/bdy"),
            Some(vec!["bd", "d"])
        );
        assert_eq!(
            test_captures("a/{b*,c*}y", "a/bdy"),
            Some(vec!["bd", "d", ""])
        );
        assert_eq!(
            test_captures("a/{b*,c*}y", "a/cdy"),
            Some(vec!["cd", "", "d"])
        );
        assert_eq!(test_captures("a/{b,c}", "a/b"), Some(vec!["b"]));
        assert_eq!(test_captures("a/{b,c}", "a/c"), Some(vec!["c"]));
        assert_eq!(test_captures("a/{b,c[}]*}", "a/b"), Some(vec!["b", "", ""]));
        assert_eq!(
            test_captures("a/{b,c[}]*}", "a/c}xx"),
            Some(vec!["c}xx", "}", "xx"])
        );
    }

    #[test]
    fn captures_globstar() {
        // assert\.deepEqual\(([!])?capture\('(.*?)', ['"](.*?)['"]\), (.*)?\);
        // assert_eq!(test_captures("$2", "$3"), Some(vec!$4));

        assert_eq!(test_captures("test/*", "test/foo"), Some(vec!["foo"]));
        assert_eq!(
            test_captures("test/*/bar", "test/foo/bar"),
            Some(vec!["foo"])
        );
        assert_eq!(
            test_captures("test/*/bar/*", "test/foo/bar/baz"),
            Some(vec!["foo", "baz"])
        );
        assert_eq!(test_captures("test/*.js", "test/foo.js"), Some(vec!["foo"]));
        assert_eq!(
            test_captures("test/*-controller.js", "test/foo-controller.js"),
            Some(vec!["foo"])
        );

        assert_eq!(
            test_captures("test/**/*.js", "test/a.js"),
            Some(vec!["", "a"])
        );
        assert_eq!(
            test_captures("test/**/*.js", "test/dir/a.js"),
            Some(vec!["dir", "a"])
        );
        assert_eq!(
            test_captures("test/**/*.js", "test/dir/test/a.js"),
            Some(vec!["dir/test", "a"])
        );
        assert_eq!(
            test_captures("**/*.js", "test/dir/a.js"),
            Some(vec!["test/dir", "a"])
        );
        assert_eq!(
            test_captures("**/**/**/**/a", "foo/bar/baz/a"),
            Some(vec!["foo/bar/baz"])
        );
        assert_eq!(
            test_captures("a/{b/**/y,c/**/d}", "a/b/y"),
            Some(vec!["b/y", "", ""])
        );
        assert_eq!(
            test_captures("a/{b/**/y,c/**/d}", "a/b/x/x/y"),
            Some(vec!["b/x/x/y", "x/x", ""])
        );
        assert_eq!(
            test_captures("a/{b/**/y,c/**/d}", "a/c/x/x/d"),
            Some(vec!["c/x/x/d", "", "x/x"])
        );
        assert_eq!(
            test_captures("a/{b/**/**/y,c/**/**/d}", "a/b/x/x/x/x/x/y"),
            Some(vec!["b/x/x/x/x/x/y", "x/x/x/x/x", ""])
        );
        assert_eq!(
            test_captures("a/{b/**/**/y,c/**/**/d}", "a/c/x/x/x/x/x/d"),
            Some(vec!["c/x/x/x/x/x/d", "", "x/x/x/x/x"])
        );
        assert_eq!(
            test_captures(
                "some/**/{a,b,c}/**/needle.txt",
                "some/path/a/to/the/needle.txt"
            ),
            Some(vec!["path", "a", "to/the"])
        );
    }

    #[test]
    fn issue_9_globstar_wildcard_dot_in_path() {
        // https://github.com/devongovett/glob-match/issues/9
        // https://github.com/devongovett/glob-match/pull/18
        assert!(glob_match("**/*.js", "a/b.c/c.js"));
        assert!(glob_match("/**/*a", "/a/a"));
        assert!(glob_match("**/**/*.js", "a/b.c/c.js"));
        assert!(glob_match("a/**/*.d", "a/b/c.d"));
        assert!(glob_match("a/**/*.d", "a/.b/c.d"));
        assert!(glob_match("**/*/**", "a/b/c"));
        assert!(glob_match("**/*/c.js", "a/b/c.js"));
        assert!(glob_match("**/**.txt.js", "/foo/bar.txt.js"));
    }

    #[test]
    fn issue_8_leading_doublestar_in_braces() {
        // https://github.com/devongovett/glob-match/issues/8
        assert!(glob_match("{**/*b}", "ab"));
    }

    #[test]
    fn issue_16_globstar_braces() {
        // https://github.com/devongovett/glob-match/issues/16
        // Still open upstream — brace backtracking can't re-enter a committed
        // alternative to extend an inner wildcard.
        // assert!(glob_match("**/foo{bar,b*z}", "foobuzz"));
    }

    #[test]
    fn pr_24_empty_alternatives_and_globstar_in_braces() {
        // https://github.com/devongovett/glob-match/pull/24
        assert!(glob_match("a{,/**}", "a"));
        assert!(glob_match("a{,/**}", "a/b"));
        assert!(glob_match("a{,/**}", "a/b/c"));
        assert!(glob_match("a{,.*{foo,db},\\(bar\\)}", "a"));
        assert!(glob_match("a{,*.{foo,db},\\(bar\\)}", "a"));
    }

    #[test]
    fn fuzz_tests() {
        // https://github.com/devongovett/glob-match/issues/1
        let s = "{*{??*{??**,Uz*zz}w**{*{**a,z***b*[!}w??*azzzzzzzz*!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!z[za,z&zz}w**z*z*}";
        assert!(!glob_match(s, s));
        let s = "**** *{*{??*{??***\u{5} *{*{??*{??***\u{5},\0U\0}]*****\u{1},\0***\0,\0\0}w****,\0U\0}]*****\u{1},\0***\0,\0\0}w*****\u{1}***{}*.*\0\0*\0";
        assert!(!glob_match(s, s));
    }
}
