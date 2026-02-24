use crate::error::{Error, ErrorKind};

/// Validate a glob pattern for structural correctness.
///
/// Returns `Ok(())` if the pattern is valid, or an `Error` describing the issue.
pub(crate) fn validate(pattern: &str) -> Result<(), Error> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut brace_depth: u32 = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return Err(Error::new(ErrorKind::DanglingEscape).with_glob(pattern));
                }
                // Skip the escaped character.
            }
            b'[' => {
                i += 1;
                // Skip negation.
                if i < bytes.len() && matches!(bytes[i], b'^' | b'!') {
                    i += 1;
                }
                // Allow `]` as first character in class.
                if i < bytes.len() && bytes[i] == b']' {
                    i += 1;
                }
                let mut found_close = false;
                while i < bytes.len() {
                    if bytes[i] == b']' {
                        found_close = true;
                        break;
                    }
                    if bytes[i] == b'\\' {
                        i += 1;
                        if i >= bytes.len() {
                            return Err(Error::new(ErrorKind::DanglingEscape).with_glob(pattern));
                        }
                    }
                    // Check for ranges like [a-z].
                    if i + 2 < bytes.len() && bytes[i + 1] == b'-' && bytes[i + 2] != b']' {
                        let lo = bytes[i];
                        let hi = bytes[i + 2];
                        if lo > hi {
                            return Err(Error::new(ErrorKind::InvalidRange(
                                lo as char, hi as char,
                            ))
                            .with_glob(pattern));
                        }
                        i += 2;
                    }
                    i += 1;
                }
                if !found_close {
                    return Err(Error::new(ErrorKind::UnclosedClass).with_glob(pattern));
                }
            }
            b'{' => {
                brace_depth += 1;
            }
            b'}' => {
                if brace_depth == 0 {
                    return Err(Error::new(ErrorKind::UnopenedAlternates).with_glob(pattern));
                }
                brace_depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }

    if brace_depth > 0 {
        return Err(Error::new(ErrorKind::UnclosedAlternates).with_glob(pattern));
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn valid_patterns() {
        assert!(validate("abc").is_ok());
        assert!(validate("a*b").is_ok());
        assert!(validate("**/*.rs").is_ok());
        assert!(validate("[abc]").is_ok());
        assert!(validate("[a-z]").is_ok());
        assert!(validate("{a,b,c}").is_ok());
        assert!(validate("a\\*b").is_ok());
        assert!(validate("a?b").is_ok());
        assert!(validate("[]]").is_ok());
        assert!(validate("[^]]").is_ok());
    }

    #[test]
    fn unclosed_class() {
        assert_eq!(
            validate("[abc").unwrap_err().kind(),
            &ErrorKind::UnclosedClass
        );
        assert_eq!(
            validate("[a-z").unwrap_err().kind(),
            &ErrorKind::UnclosedClass
        );
    }

    #[test]
    fn invalid_range() {
        assert_eq!(
            validate("[z-a]").unwrap_err().kind(),
            &ErrorKind::InvalidRange('z', 'a')
        );
    }

    #[test]
    fn unopened_alternates() {
        assert_eq!(
            validate("a}b").unwrap_err().kind(),
            &ErrorKind::UnopenedAlternates
        );
    }

    #[test]
    fn unclosed_alternates() {
        assert_eq!(
            validate("{a,b").unwrap_err().kind(),
            &ErrorKind::UnclosedAlternates
        );
    }

    #[test]
    fn dangling_escape() {
        assert_eq!(
            validate("abc\\").unwrap_err().kind(),
            &ErrorKind::DanglingEscape
        );
    }
}
