// Originally from yada (https://github.com/takuyaa/yada)
// Licensed under MIT OR Apache-2.0

use alloc::string::String;
use core::fmt;

/// `UnitID` is an alias of `usize`.
pub type UnitID = usize;

/// The size of a single [`Unit`] in bytes (4).
pub const UNIT_SIZE: usize = core::mem::size_of::<u32>();

/// A 32-bit node in the double-array trie.
///
/// Non-leaf layout:
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +---------------+-+-+-----------------------------------------+-+
/// |     LABEL     |H|E|                 OFFSET                  |I|
/// +---------------+-+-+-----------------------------------------+-+
/// ```
///
/// Leaf layout:
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-------------------------------------------------------------+-+
/// |                            VALUE                            |I|
/// +-------------------------------------------------------------+-+
/// ```
#[derive(Copy, Clone)]
pub struct Unit(u32);

impl Unit {
    /// Creates a new zero-initialized unit.
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Creates a unit from a raw `u32`.
    #[inline]
    pub fn from_u32(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw `u32` representation.
    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Returns `true` if this unit has a leaf child.
    #[inline]
    pub fn has_leaf(&self) -> bool {
        self.0 >> 8 & 1 == 1
    }

    /// Returns `true` if this unit is a leaf (stores a value).
    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.0 >> 31 == 1
    }

    /// Returns the 31-bit value stored in a leaf unit.
    #[inline]
    pub fn value(&self) -> u32 {
        self.0 & 0x7FFF_FFFF
    }

    /// Returns the label byte (valid only for non-leaf units).
    #[inline]
    pub fn label(&self) -> u32 {
        self.0 & ((1 << 31) | 0xFF)
    }

    /// Returns the offset, accounting for the extension flag.
    #[inline]
    pub fn offset(&self) -> u32 {
        (self.0 >> 10) << ((self.0 & (1 << 9)) >> 6)
    }

    /// Sets the offset.
    ///
    /// # Panics
    ///
    /// Panics if `offset >= 2^29`, or if an extended offset has non-zero
    /// lower 8 bits.
    #[inline]
    pub fn set_offset(&mut self, offset: u32) {
        assert!(offset < (1u32 << 29));

        if offset < (1u32 << 21) {
            self.0 = offset << 10 | (self.0 << 23_u32) >> 23;
        } else {
            // Extended offset: lower 8 bits must be zero.
            assert_eq!(offset & 0xFF, 0, "lower 8 bits of offset should be 0");
            self.0 = offset << 2 | (1 << 9) | (self.0 << 23_u32) >> 23;
        }
    }

    /// Sets the `has_leaf` flag.
    #[inline]
    pub fn set_has_leaf(&mut self, has_leaf: bool) {
        self.0 = if has_leaf {
            self.0 | 1 << 8
        } else {
            self.0 & !(1 << 8)
        };
    }

    /// Sets the label byte.
    #[inline]
    pub fn set_label(&mut self, label: u8) {
        self.0 = (self.0 >> 8) << 8 | u32::from(label);
    }

    /// Sets the leaf value (also sets the `IS_LEAF` flag).
    #[inline]
    pub fn set_value(&mut self, value: u32) {
        self.0 = value | 1 << 31;
    }

    /// Returns a human-readable string representation.
    pub fn display(&self) -> String {
        if self.is_leaf() {
            alloc::format!("Unit {{ value: {} }}", self.value())
        } else {
            let label = self.label();
            #[allow(clippy::cast_possible_truncation)]
            let label_str = match label {
                0 => String::from("NULL"),
                1..=255 => {
                    let ch = label as u8 as char;
                    alloc::format!("{}", ch.escape_default())
                }
                _ => String::from("INVALID"),
            };
            alloc::format!(
                "Unit {{ offset: {}, label: {}, has_leaf: {} }}",
                self.offset(),
                label_str,
                self.has_leaf()
            )
        }
    }
}

impl Default for Unit {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[cfg(test)]
mod tests {
    use crate::Unit;

    #[test]
    fn unit_value() {
        let mut unit = Unit::new();
        assert_eq!(unit.value(), 0);

        unit.set_value(5);
        assert_eq!(unit.value(), 5);

        unit.set_value((1 << 31) - 1);
        assert_eq!(unit.value(), (1 << 31) - 1);

        unit.set_value(1 << 31);
        assert_eq!(unit.value(), 0);
    }

    #[test]
    fn label() {
        let unit = Unit::new();
        assert_eq!(unit.label(), 0);

        let mut unit = Unit::new();
        unit.set_label(0);
        assert_eq!(unit.label(), 0);

        let mut unit = Unit::new();
        unit.set_label(1);
        assert_eq!(unit.label(), 1);

        let mut unit = Unit::new();
        unit.set_label(255);
        assert_eq!(unit.label(), 255);
    }

    #[test]
    fn offset() {
        let unit = Unit::new();
        assert_eq!(unit.offset(), 0);

        let mut unit = Unit::new();
        unit.set_offset(0);
        assert_eq!(unit.offset(), 0);

        let mut unit = Unit::new();
        unit.set_offset(1);
        assert_eq!(unit.offset(), 1);

        let mut unit = Unit::new();
        unit.set_offset((1 << 21) - 1);
        assert_eq!(unit.offset(), (1 << 21) - 1);

        let mut unit = Unit::new();
        unit.set_offset(1 << 21);
        assert_eq!(unit.offset(), 1 << 21);

        let mut unit = Unit::new();
        unit.set_offset(1 << 28);
        assert_eq!(unit.offset(), 1 << 28);
    }
}
