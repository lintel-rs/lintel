// Originally from yada (https://github.com/takuyaa/yada)
// Licensed under MIT OR Apache-2.0

use alloc::vec;
use alloc::vec::Vec;

use hashbrown::HashSet;

use crate::unit::{Unit, UnitID};

const BLOCK_SIZE: usize = 256;
const NUM_TARGET_BLOCKS: i32 = 16; // the number of target blocks to find offsets
const INVALID_NEXT: u8 = 0; // 0 means that there is no next unused unit
const INVALID_PREV: u8 = 255; // 255 means that there is no previous unused unit

/// A double-array trie builder.
#[derive(Debug)]
pub struct DoubleArrayBuilder {
    blocks: Vec<DoubleArrayBlock>,
    used_offsets: HashSet<u32>,
}

impl DoubleArrayBuilder {
    /// Constructs a new `DoubleArrayBuilder` with an empty `DoubleArrayBlock`.
    fn new() -> Self {
        Self {
            blocks: vec![DoubleArrayBlock::new(0)],
            used_offsets: HashSet::new(),
        }
    }

    /// Builds a double-array trie from a sorted `keyset`.
    ///
    /// Returns the serialised byte buffer on success, or `None` if the keyset
    /// is malformed.
    pub fn build<T>(keyset: &[(T, u32)]) -> Option<Vec<u8>>
    where
        T: AsRef<[u8]>,
    {
        Self::new().build_from_keyset(keyset)
    }

    /// Builds a double-array trie from a sorted `keyset`, reusing this builder.
    pub fn build_from_keyset<T>(&mut self, keyset: &[(T, u32)]) -> Option<Vec<u8>>
    where
        T: AsRef<[u8]>,
    {
        self.reserve(0); // reserve root node
        self.build_recursive(keyset, 0, 0, keyset.len(), 0)?;

        let mut da_bytes = Vec::with_capacity(self.blocks.len() * BLOCK_SIZE);
        for block in &self.blocks {
            for unit in &block.units {
                da_bytes.extend_from_slice(&unit.as_u32().to_le_bytes());
            }
        }

        Some(da_bytes)
    }

    /// Returns the total number of `Unit`s that this builder contains.
    #[allow(clippy::cast_possible_truncation)]
    pub fn num_units(&self) -> u32 {
        (self.blocks.len() * BLOCK_SIZE) as u32
    }

    /// Returns the number of used `Unit`s that this builder contains.
    pub fn num_used_units(&self) -> u32 {
        self.blocks
            .iter()
            .map(|block| {
                block
                    .is_used
                    .iter()
                    .fold(0, |acc, &is_used| acc + u32::from(is_used))
            })
            .sum::<u32>()
    }

    fn get_block(&self, unit_id: UnitID) -> Option<&DoubleArrayBlock> {
        self.blocks.get(unit_id / BLOCK_SIZE)
    }

    fn get_block_mut(&mut self, unit_id: UnitID) -> Option<&mut DoubleArrayBlock> {
        self.blocks.get_mut(unit_id / BLOCK_SIZE)
    }

    fn extend_block(&mut self) -> &DoubleArrayBlock {
        let block_id = self.blocks.len();
        self.blocks.push(DoubleArrayBlock::new(block_id));
        // SAFETY: we just pushed an element
        self.blocks.last().expect("just pushed")
    }

    fn extend_block_mut(&mut self) -> &mut DoubleArrayBlock {
        let block_id = self.blocks.len();
        self.blocks.push(DoubleArrayBlock::new(block_id));
        // SAFETY: we just pushed an element
        self.blocks.last_mut().expect("just pushed")
    }

    fn get_unit_mut(&mut self, unit_id: UnitID) -> &mut Unit {
        while self.get_block(unit_id).is_none() {
            self.extend_block_mut();
        }
        let block = self.get_block_mut(unit_id).expect("block exists");
        &mut block.units[unit_id % BLOCK_SIZE]
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reserve(&mut self, unit_id: UnitID) {
        while self.get_block(unit_id).is_none() {
            self.extend_block_mut();
        }
        let block = self.get_block_mut(unit_id).expect("block exists");
        assert!(unit_id % BLOCK_SIZE < 256);
        block.reserve((unit_id % BLOCK_SIZE) as u8);
    }

    #[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
    fn build_recursive<T>(
        &mut self,
        keyset: &[(T, u32)],
        depth: usize,
        begin: usize,
        end: usize,
        unit_id: UnitID,
    ) -> Option<()>
    where
        T: AsRef<[u8]>,
    {
        // element of labels is a tuple (label, start_position, end_position)
        let mut labels: Vec<(u8, usize, usize)> = Vec::with_capacity(256);
        let mut value = None;

        for i in begin..end {
            let key_value = keyset.get(i)?;
            let label = {
                let key = key_value.0.as_ref();
                if depth == key.len() {
                    0
                } else {
                    *key.get(depth)?
                }
            };
            if label == 0 {
                assert!(value.is_none()); // there is just one '\0' in a key
                value = Some(key_value.1);
            }
            match labels.last_mut() {
                Some(last_label) => {
                    if last_label.0 != label {
                        last_label.2 = i; // set end position
                        labels.push((label, i, 0));
                    }
                }
                None => {
                    labels.push((label, i, 0));
                }
            }
        }
        assert!(!labels.is_empty());

        let last_label = labels.last_mut().expect("labels non-empty");
        last_label.2 = end;

        let label_keys: Vec<u8> = labels.iter().map(|(key, _, _)| *key).collect();
        assert!(!label_keys.is_empty());

        // Search an offset where these children fit in unused positions.
        let offset: u32 = loop {
            if let Some(o) = self.find_offset(unit_id, &label_keys) {
                break o;
            }
            self.extend_block();
        };
        assert!(
            offset < (1u32 << 29),
            "offset must be represented as 29 bits integer"
        );

        // Mark the offset used.
        self.used_offsets.insert(offset);

        let has_leaf = label_keys.first() == Some(&0);

        // Populate offset and has_leaf flag to parent node.
        let parent_unit = self.get_unit_mut(unit_id);
        assert_eq!(
            parent_unit.offset(),
            0,
            "offset() should return 0 before set_offset()"
        );
        parent_unit.set_offset(offset ^ unit_id as u32); // store the relative offset
        assert!(
            !parent_unit.has_leaf(),
            "has_leaf() should return false before set_has_leaf()"
        );
        parent_unit.set_has_leaf(has_leaf);

        // Populate label or associated value to children nodes.
        for &label in &label_keys {
            let child_id = (offset ^ u32::from(label)) as UnitID;
            self.reserve(child_id);

            let unit = self.get_unit_mut(child_id);

            // Child node units should be empty.
            assert_eq!(unit.offset(), 0);
            assert_eq!(unit.label(), 0);
            assert_eq!(unit.value(), 0);
            assert!(!unit.has_leaf());

            if label == 0 {
                unit.set_value(value.expect("leaf must have a value"));
            } else {
                unit.set_label(label);
            }
        }

        // Recursive call in depth-first order.
        for (label, begin, end) in labels {
            self.build_recursive(
                keyset,
                depth + 1,
                begin,
                end,
                (u32::from(label) ^ offset) as UnitID,
            );
        }

        Some(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn find_offset(&self, unit_id: UnitID, labels: &[u8]) -> Option<u32> {
        let head_block = self.blocks.len().saturating_sub(NUM_TARGET_BLOCKS as usize);
        self.blocks
            .iter()
            .skip(head_block) // search for offset in last N blocks
            .find_map(|block| {
                // Find the first valid offset in a block.
                for offset in block.find_offset(unit_id, labels) {
                    let offset_u32 = (block.id as u32) << 8 | u32::from(offset);
                    if !self.used_offsets.contains(&offset_u32) {
                        return Some(offset_u32);
                    }
                }
                None
            })
    }
}

const DEFAULT_UNITS: [Unit; BLOCK_SIZE] = [Unit::new(); BLOCK_SIZE];
const DEFAULT_IS_USED: [bool; BLOCK_SIZE] = [false; BLOCK_SIZE];
#[allow(clippy::cast_possible_truncation)]
const DEFAULT_NEXT_UNUSED: [u8; BLOCK_SIZE] = {
    let mut next_unused = [INVALID_NEXT; BLOCK_SIZE];
    let mut i = 0;
    while i < next_unused.len() - 1 {
        next_unused[i] = (i + 1) as u8;
        i += 1;
    }
    next_unused
};
#[allow(clippy::cast_possible_truncation)]
const DEFAULT_PREV_UNUSED: [u8; BLOCK_SIZE] = {
    let mut prev_unused = [INVALID_PREV; BLOCK_SIZE];
    let mut i = 1;
    while i < prev_unused.len() {
        prev_unused[i] = (i - 1) as u8;
        i += 1;
    }
    prev_unused
};

/// A block containing a shard of a double-array and bookkeeping structures.
pub struct DoubleArrayBlock {
    id: usize,
    units: [Unit; BLOCK_SIZE],
    is_used: [bool; BLOCK_SIZE],
    head_unused: u8,
    next_unused: [u8; BLOCK_SIZE],
    prev_unused: [u8; BLOCK_SIZE],
}

impl DoubleArrayBlock {
    const fn new(id: usize) -> Self {
        Self {
            id,
            units: DEFAULT_UNITS,
            is_used: DEFAULT_IS_USED,
            head_unused: 0,
            next_unused: DEFAULT_NEXT_UNUSED,
            prev_unused: DEFAULT_PREV_UNUSED,
        }
    }

    /// Finds valid offsets in this block.
    fn find_offset<'a>(
        &'a self,
        unit_id: UnitID,
        labels: &'a [u8],
    ) -> impl Iterator<Item = u8> + 'a {
        assert!(!labels.is_empty());
        FindOffset {
            unused_id: self.head_unused,
            block: self,
            unit_id,
            labels,
        }
    }

    fn reserve(&mut self, id: u8) {
        // maintain is_used
        self.is_used[id as usize] = true;

        let prev_id = self.prev_unused[id as usize];
        let next_id = self.next_unused[id as usize];

        // maintain next_unused
        if prev_id != INVALID_PREV {
            self.next_unused[prev_id as usize] = next_id;
        }
        self.next_unused[id as usize] = INVALID_NEXT;

        // maintain prev_unused
        if next_id != INVALID_NEXT {
            self.prev_unused[next_id as usize] = prev_id;
        }
        self.prev_unused[id as usize] = INVALID_PREV;

        // maintain head_unused
        if id == self.head_unused {
            self.head_unused = next_id;
        }
    }
}

struct FindOffset<'a> {
    unused_id: u8,
    block: &'a DoubleArrayBlock,
    unit_id: UnitID, // parent node position to set the offset
    labels: &'a [u8],
}

impl FindOffset<'_> {
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn is_valid_offset(&self, offset: u8) -> bool {
        let offset_u32 = (self.block.id as u32) << 8 | u32::from(offset);
        let relative_offset = self.unit_id as u32 ^ offset_u32;
        if (relative_offset & (0xFF << 21)) > 0 && (relative_offset & 0xFF) > 0 {
            return false;
        }

        self.labels.iter().skip(1).all(|label| {
            let id = offset ^ label;
            self.block
                .is_used
                .get(id as usize)
                .is_some_and(|is_used| !*is_used)
        })
    }
}

impl Iterator for FindOffset<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.unused_id == INVALID_NEXT && self.block.is_used[self.unused_id as usize] {
            return None;
        }

        // Return if this block is full.
        if self.block.head_unused == INVALID_NEXT && self.block.is_used[0] {
            debug_assert!(self.block.is_used.iter().all(|is_used| *is_used));
            return None;
        }
        debug_assert!(!self.block.is_used.iter().all(|is_used| *is_used));

        loop {
            debug_assert!(!self.block.is_used[self.unused_id as usize]);

            let first_label = *self.labels.first()?;
            let offset = self.unused_id ^ first_label;

            let is_valid_offset = self.is_valid_offset(offset);

            // Update unused_id to next unused node.
            self.unused_id = self.block.next_unused[self.unused_id as usize];

            if is_valid_offset {
                return Some(offset);
            }

            if self.unused_id == INVALID_NEXT {
                return None;
            }
        }
    }
}

impl core::fmt::Debug for DoubleArrayBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DoubleArrayBlock")
            .field("id", &self.id)
            .field(
                "units",
                &format_args!(
                    "[{}]",
                    DisplayJoin {
                        items: &self.units,
                        fmt_item: |u: &Unit, f: &mut core::fmt::Formatter<'_>| write!(f, "{u:?}"),
                    }
                ),
            )
            .field(
                "is_used",
                &format_args!(
                    "[{}]",
                    DisplayJoin {
                        items: &self.is_used,
                        fmt_item: |b: &bool, f: &mut core::fmt::Formatter<'_>| write!(f, "{b}"),
                    }
                ),
            )
            .field("head_unused", &self.head_unused)
            .field(
                "next_unused",
                &format_args!(
                    "[{}]",
                    DisplayJoin {
                        items: &self.next_unused,
                        fmt_item: |n: &u8, f: &mut core::fmt::Formatter<'_>| write!(f, "{n}"),
                    }
                ),
            )
            .field(
                "prev_unused",
                &format_args!(
                    "[{}]",
                    DisplayJoin {
                        items: &self.prev_unused,
                        fmt_item: |n: &u8, f: &mut core::fmt::Formatter<'_>| write!(f, "{n}"),
                    }
                ),
            )
            .finish()
    }
}

/// Helper for comma-joined Display without allocating a `Vec<String>`.
struct DisplayJoin<'a, T, F> {
    items: &'a [T],
    fmt_item: F,
}

impl<T, F> core::fmt::Display for DisplayJoin<'_, T, F>
where
    F: Fn(&T, &mut core::fmt::Formatter<'_>) -> core::fmt::Result,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            (self.fmt_item)(item, f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::DoubleArrayBuilder;

    #[test]
    fn build() {
        let keyset: &[(&[u8], u32)] = &[
            ("a".as_bytes(), 0),
            ("aa".as_bytes(), 0),
            ("aaa".as_bytes(), 0),
            ("aaaa".as_bytes(), 0),
            ("aaaaa".as_bytes(), 0),
            ("ab".as_bytes(), 0),
            ("abc".as_bytes(), 0),
            ("abcd".as_bytes(), 0),
            ("abcde".as_bytes(), 0),
            ("abcdef".as_bytes(), 0),
        ];

        let mut builder = DoubleArrayBuilder::new();
        let da = builder.build_from_keyset(keyset);
        assert!(da.is_some());

        assert!(0 < builder.num_units());
        assert!(0 < builder.num_used_units());
        assert!(builder.num_used_units() < builder.num_units());
    }
}
