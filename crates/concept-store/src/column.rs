//! A dense column: one record per ordinal, addressed by position.
//!
//! An ordinal is a position, so the record at that position is found by
//! reading two offsets and slicing, with no search and no per-read
//! transaction. No spec governs the layout: our own design, the same shape
//! `concept-graph` stores its adjacency in. Little-endian throughout: a
//! record count, then one offset per record plus a terminator, then the
//! records themselves end to end.
//!
//! A record the build never wrote has an empty range. Every encoding in
//! [`crate::record`] is longer than zero bytes, so an empty range means
//! absent and never an empty record.

use std::fmt;

use concept_graph::ordinal::{Ordinal, to_usize};

/// The width of the count and of each offset.
const WIDTH: usize = 4;

/// A column whose bytes are not the layout this build reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ColumnError {
    /// The bytes end before the header or the offsets do.
    #[error("the column is {found} bytes, shorter than the {needed} its header declares")]
    Truncated {
        /// The length found.
        found: usize,
        /// The length the header needs.
        needed: usize,
    },
    /// An offset points outside the records, or before the one before it.
    #[error(
        "offset {at} is {offset}, outside the {len} bytes of records or behind its predecessor"
    )]
    Offset {
        /// The index of the offending offset.
        at: usize,
        /// Its value.
        offset: usize,
        /// The length of the records.
        len: usize,
    },
}

/// A read-only dense column.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Column {
    /// `count + 1` offsets into `records`, ascending.
    offsets: Vec<u32>,
    records: Vec<u8>,
}

impl fmt::Debug for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Column")
            .field("len", &self.len())
            .field("bytes", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl Column {
    /// The column `records` pack, in ordinal order.
    ///
    /// A record is placed at its ordinal, and an ordinal no record names gets
    /// an empty range.
    #[must_use]
    pub fn pack<'a>(count: u32, records: impl IntoIterator<Item = (Ordinal, &'a [u8])>) -> Vec<u8> {
        let count = to_usize(count);
        let mut placed: Vec<&[u8]> = vec![&[]; count];
        for (ordinal, bytes) in records {
            let at = to_usize(ordinal.index());
            if let Some(slot) = placed.get_mut(at) {
                *slot = bytes;
            }
        }
        let total: usize = placed.iter().map(|r| r.len()).sum();
        let mut out = Vec::with_capacity(WIDTH * (count + 2) + total);
        out.extend_from_slice(&u32::try_from(count).unwrap_or(u32::MAX).to_le_bytes());
        let mut at = 0_u32;
        for record in &placed {
            out.extend_from_slice(&at.to_le_bytes());
            at = at.saturating_add(u32::try_from(record.len()).unwrap_or(u32::MAX));
        }
        out.extend_from_slice(&at.to_le_bytes());
        for record in placed {
            out.extend_from_slice(record);
        }
        out
    }

    /// Reads a packed column, checking its offsets once so a read never can.
    ///
    /// # Errors
    ///
    /// Returns [`ColumnError`] when the bytes are shorter than the header
    /// declares or an offset lies outside the records.
    pub fn read(bytes: &[u8]) -> Result<Self, ColumnError> {
        let short = |needed: usize| ColumnError::Truncated {
            found: bytes.len(),
            needed,
        };
        let head = bytes.get(..WIDTH).ok_or_else(|| short(WIDTH))?;
        let count = to_usize(u32::from_le_bytes(head.try_into().unwrap_or([0, 0, 0, 0])));
        let end = WIDTH.saturating_mul(count.saturating_add(2));
        let table = bytes.get(WIDTH..end).ok_or_else(|| short(end))?;
        let offsets: Vec<u32> = table
            .as_chunks::<WIDTH>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        let records = bytes.get(end..).unwrap_or_default().to_vec();
        let mut previous = 0_u32;
        for (at, offset) in offsets.iter().enumerate() {
            if *offset < previous || to_usize(*offset) > records.len() {
                return Err(ColumnError::Offset {
                    at,
                    offset: to_usize(*offset),
                    len: records.len(),
                });
            }
            previous = *offset;
        }
        Ok(Self { offsets, records })
    }

    /// How many ordinals the column holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Whether the column holds no ordinals.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The record at `ordinal`, `None` beyond the column or where the build
    /// wrote none.
    ///
    /// The offsets were checked when the column was read, so this slices
    /// without failing.
    #[must_use]
    pub fn get(&self, ordinal: Ordinal) -> Option<&[u8]> {
        let at = to_usize(ordinal.index());
        let start = to_usize(*self.offsets.get(at)?);
        let end = to_usize(*self.offsets.get(at.checked_add(1)?)?);
        if start == end {
            return None;
        }
        self.records.get(start..end)
    }
}

#[cfg(test)]
mod tests {
    use concept_graph::ordinal::Ordinal;

    use super::{Column, ColumnError};

    #[test]
    fn a_packed_column_reads_back_what_was_placed() {
        let packed = Column::pack(
            4,
            [
                (Ordinal::new(0), b"first".as_slice()),
                (Ordinal::new(3), b"fourth".as_slice()),
            ],
        );
        let column = Column::read(&packed).expect("reads");
        assert_eq!(column.len(), 4);
        assert!(!column.is_empty());
        assert_eq!(column.get(Ordinal::new(0)), Some(b"first".as_slice()));
        assert_eq!(column.get(Ordinal::new(1)), None, "no record was placed");
        assert_eq!(column.get(Ordinal::new(3)), Some(b"fourth".as_slice()));
        assert_eq!(column.get(Ordinal::new(4)), None, "beyond the column");
    }

    #[test]
    fn an_empty_column_holds_nothing() {
        let column = Column::read(&Column::pack(0, [])).expect("reads");
        assert!(column.is_empty());
        assert_eq!(column.get(Ordinal::new(0)), None);
    }

    #[test]
    fn a_short_or_crossed_column_is_refused() {
        assert!(matches!(
            Column::read(&[0, 0]),
            Err(ColumnError::Truncated { .. })
        ));
        // The header survives and the records do not, so an offset now points
        // past the end of them.
        let packed = Column::pack(2, [(Ordinal::new(0), b"ab".as_slice())]);
        assert!(matches!(
            Column::read(packed.get(..packed.len() - 1).expect("shortened")),
            Err(ColumnError::Offset { .. })
        ));
    }
}
