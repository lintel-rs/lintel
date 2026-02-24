//! A fast, compact double-array trie.
//!
//! `tried` is a fork of [yada](https://crates.io/crates/yada) with `no_std` +
//! `alloc` support. It provides O(key-length) exact-match and common-prefix
//! search over a flat, cache-friendly byte buffer.
//!
//! # Example
//!
//! ```
//! use tried::{DoubleArray, DoubleArrayBuilder};
//!
//! let keyset: &[(&[u8], u32)] = &[
//!     (b"a", 0),
//!     (b"ab", 1),
//!     (b"abc", 2),
//! ];
//! let bytes = DoubleArrayBuilder::build(keyset).unwrap();
//! let da = DoubleArray::new(bytes);
//!
//! assert_eq!(da.exact_match_search(b"ab"), Some(1));
//!
//! let prefixes: Vec<_> = da.common_prefix_search(b"abcd").collect();
//! assert_eq!(prefixes, vec![(0, 1), (1, 2), (2, 3)]);
//! ```
#![no_std]
extern crate alloc;

pub mod builder;
pub mod unit;

use core::ops::Deref;

pub use crate::builder::DoubleArrayBuilder;
use crate::unit::{UNIT_SIZE, Unit, UnitID};

/// A read-only double-array trie backed by a contiguous byte buffer.
///
/// The type parameter `T` can be any type that dereferences to `[u8]`:
/// `Vec<u8>`, `Box<[u8]>`, `&[u8]`, etc.
#[derive(Clone)]
pub struct DoubleArray<T>(pub T)
where
    T: Deref<Target = [u8]>;

impl<T> DoubleArray<T>
where
    T: Deref<Target = [u8]>,
{
    /// Creates a new `DoubleArray` from the given byte buffer.
    pub fn new(bytes: T) -> Self {
        Self(bytes)
    }

    /// Finds the value associated with an exact `key`, or `None`.
    pub fn exact_match_search<K>(&self, key: K) -> Option<u32>
    where
        K: AsRef<[u8]>,
    {
        self.exact_match_search_bytes(key.as_ref())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn exact_match_search_bytes(&self, key: &[u8]) -> Option<u32> {
        let mut node_pos: UnitID = 0;
        let mut unit = self.get_unit(node_pos)?;

        for &c in key {
            debug_assert!(!unit.is_leaf());
            debug_assert_ne!(c, 0); // keys must not contain NUL

            node_pos = (unit.offset() ^ node_pos as u32 ^ u32::from(c)) as UnitID;
            unit = self.get_unit(node_pos)?;

            if unit.label() != u32::from(c) {
                return None;
            }
        }

        if !unit.has_leaf() {
            return None;
        }

        // Traverse by NUL to reach the leaf.
        let node_pos = (unit.offset() ^ node_pos as u32) as UnitID;
        unit = self.get_unit(node_pos)?;
        debug_assert!(unit.is_leaf());

        Some(unit.value())
    }

    /// Returns an iterator over all `(value, key_length)` pairs whose keys are
    /// prefixes of `key`.
    ///
    /// The iterator yields matches in ascending key-length order and performs
    /// zero heap allocation.
    pub fn common_prefix_search<'a, K>(&'a self, key: &'a K) -> CommonPrefixSearch<'a, T>
    where
        K: AsRef<[u8]> + ?Sized,
    {
        CommonPrefixSearch {
            key: key.as_ref(),
            double_array: self,
            unit_id: 0,
            key_pos: 0,
        }
    }

    #[inline]
    fn get_unit(&self, index: usize) -> Option<Unit> {
        let start = index * UNIT_SIZE;
        let end = start + UNIT_SIZE;
        let b = self.0.get(start..end)?;
        let bytes: [u8; 4] = b.try_into().ok()?;
        Some(Unit::from_u32(u32::from_le_bytes(bytes)))
    }
}

/// Zero-allocation iterator returned by [`DoubleArray::common_prefix_search`].
pub struct CommonPrefixSearch<'a, T>
where
    T: Deref<Target = [u8]>,
{
    key: &'a [u8],
    double_array: &'a DoubleArray<T>,
    unit_id: UnitID,
    key_pos: usize,
}

impl<T> Iterator for CommonPrefixSearch<'_, T>
where
    T: Deref<Target = [u8]>,
{
    type Item = (u32, usize);

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn next(&mut self) -> Option<Self::Item> {
        while self.key_pos < self.key.len() {
            let unit = self.double_array.get_unit(self.unit_id)?;

            let c = *self.key.get(self.key_pos)?;
            self.key_pos += 1;

            self.unit_id = (unit.offset() ^ self.unit_id as u32 ^ u32::from(c)) as UnitID;
            let unit = self.double_array.get_unit(self.unit_id)?;
            if unit.label() != u32::from(c) {
                return None;
            }
            if unit.has_leaf() {
                let leaf_pos = unit.offset() ^ self.unit_id as u32;
                let leaf_unit = self.double_array.get_unit(leaf_pos as UnitID)?;
                return Some((leaf_unit.value(), self.key_pos));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use crate::DoubleArray;
    use crate::builder::DoubleArrayBuilder;

    #[test]
    fn build_and_search() {
        let keyset = &[
            ("a".as_bytes(), 0),
            ("ab".as_bytes(), 1),
            ("aba".as_bytes(), 2),
            ("ac".as_bytes(), 3),
            ("acb".as_bytes(), 4),
            ("acc".as_bytes(), 5),
            ("ad".as_bytes(), 6),
            ("ba".as_bytes(), 7),
            ("bb".as_bytes(), 8),
            ("bc".as_bytes(), 9),
            ("c".as_bytes(), 10),
            ("caa".as_bytes(), 11),
        ];

        let da_bytes = DoubleArrayBuilder::build(keyset);
        assert!(da_bytes.is_some());

        let da = DoubleArray::new(da_bytes.expect("build failed"));

        for (key, value) in keyset {
            assert_eq!(da.exact_match_search(key), Some(*value));
        }
        assert_eq!(da.exact_match_search("aa".as_bytes()), None);
        assert_eq!(da.exact_match_search("abc".as_bytes()), None);
        assert_eq!(da.exact_match_search("b".as_bytes()), None);
        assert_eq!(da.exact_match_search("ca".as_bytes()), None);

        assert_eq!(
            da.common_prefix_search("a".as_bytes()).collect::<Vec<_>>(),
            vec![(0, 1)]
        );
        assert_eq!(
            da.common_prefix_search("aa".as_bytes()).collect::<Vec<_>>(),
            vec![(0, 1)]
        );
        assert_eq!(
            da.common_prefix_search("abbb".as_bytes())
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 2)]
        );
        assert_eq!(
            da.common_prefix_search("abaa".as_bytes())
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 2), (2, 3)]
        );
        assert_eq!(
            da.common_prefix_search("caa".as_bytes())
                .collect::<Vec<_>>(),
            vec![(10, 1), (11, 3)]
        );
        assert_eq!(
            da.common_prefix_search("d".as_bytes()).collect::<Vec<_>>(),
            vec![]
        );
    }

    #[test]
    fn exact_match_search_corner_case() {
        // Regression test from https://github.com/takuyaa/yada/pull/28
        let keyset = &[
            ("a".as_bytes(), 97),
            ("ab".as_bytes(), 1),
            ("de".as_bytes(), 2),
        ];

        let da_bytes = DoubleArrayBuilder::build(keyset);
        assert!(da_bytes.is_some());

        let da = DoubleArray::new(da_bytes.expect("build failed"));

        for (key, value) in keyset {
            assert_eq!(da.exact_match_search(key), Some(*value));
        }
        assert_eq!(da.exact_match_search("dasss"), None);
    }

    #[test]
    fn clone_and_search() {
        let keyset = &[
            ("a".as_bytes(), 0),
            ("ab".as_bytes(), 1),
            ("abc".as_bytes(), 2),
        ];

        let da_bytes = DoubleArrayBuilder::build(keyset).expect("build failed");
        let da_orig = DoubleArray::new(da_bytes);
        let da = da_orig.clone();

        assert_eq!(da.exact_match_search("a"), Some(0));
        assert_eq!(da.exact_match_search("ab"), Some(1));
        assert_eq!(da.exact_match_search("abc"), Some(2));
        assert_eq!(da.exact_match_search("d"), None);
    }
}
