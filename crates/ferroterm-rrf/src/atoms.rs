//! The atom table beside a built `RxNorm` artifact: `RXAUI` to concept ordinal.
//!
//! The FHIR `RxNorm` page lets a relationship filter name its target as
//! `AUI:[RXAUI]` (<https://hl7.org/fhir/R4B/rxnorm.html>), so the served
//! artifact keeps every atom identifier next to its concept. No spec governs
//! the layout: our own design, little-endian, a magic and version, a count,
//! then sorted `(RXAUI as u64, ordinal as u32)` pairs.

use std::io::{self, Read, Write};

const MAGIC: &[u8; 8] = b"FTATOMS\0";
const VERSION: u32 = 1;

/// A failure while reading or writing the table.
#[derive(Debug, thiserror::Error)]
pub enum AtomsError {
    /// An I/O failure.
    #[error("atom table I/O failed")]
    Io(#[from] io::Error),
    /// The bytes do not start with the table magic.
    #[error("not an atom table")]
    Magic,
    /// The layout version is not the one this build reads.
    #[error("atom table version {found}, expected {expected}")]
    Version {
        /// The version found.
        found: u32,
        /// The version this build reads.
        expected: u32,
    },
}

/// The sorted atom table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Atoms {
    pairs: Vec<(u64, u32)>,
}

impl Atoms {
    /// Builds the table from `(RXAUI, ordinal)` pairs, in any order.
    #[must_use]
    pub fn new(mut pairs: Vec<(u64, u32)>) -> Self {
        pairs.sort_unstable();
        pairs.dedup();
        Self { pairs }
    }

    /// The concept ordinal of the atom `rxaui`.
    #[must_use]
    pub fn concept(&self, rxaui: u64) -> Option<u32> {
        self.pairs
            .binary_search_by_key(&rxaui, |(a, _)| *a)
            .ok()
            .and_then(|i| self.pairs.get(i))
            .map(|(_, o)| *o)
    }

    /// The atom count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Writes the layout.
    ///
    /// # Errors
    ///
    /// Returns [`AtomsError::Io`] when writing fails.
    pub fn write_to(&self, out: &mut impl Write) -> Result<(), AtomsError> {
        out.write_all(MAGIC)?;
        out.write_all(&VERSION.to_le_bytes())?;
        let len =
            u64::try_from(self.pairs.len()).map_err(|_| io::Error::other("too many atoms"))?;
        out.write_all(&len.to_le_bytes())?;
        for (rxaui, ordinal) in &self.pairs {
            out.write_all(&rxaui.to_le_bytes())?;
            out.write_all(&ordinal.to_le_bytes())?;
        }
        Ok(())
    }

    /// Reads the layout.
    ///
    /// # Errors
    ///
    /// Returns [`AtomsError`] for a truncated or foreign table.
    pub fn read_from(input: &mut impl Read) -> Result<Self, AtomsError> {
        let mut magic = [0_u8; 8];
        input.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(AtomsError::Magic);
        }
        let mut word = [0_u8; 4];
        input.read_exact(&mut word)?;
        let version = u32::from_le_bytes(word);
        if version != VERSION {
            return Err(AtomsError::Version {
                found: version,
                expected: VERSION,
            });
        }
        let mut long = [0_u8; 8];
        input.read_exact(&mut long)?;
        let len = usize::try_from(u64::from_le_bytes(long))
            .map_err(|_| io::Error::other("atom table too large"))?;
        let mut pairs = Vec::with_capacity(len);
        for _ in 0..len {
            input.read_exact(&mut long)?;
            input.read_exact(&mut word)?;
            pairs.push((u64::from_le_bytes(long), u32::from_le_bytes(word)));
        }
        Ok(Self { pairs })
    }
}

#[cfg(test)]
mod tests {
    use super::{Atoms, AtomsError};

    #[test]
    fn the_table_answers_by_atom_and_round_trips() {
        let atoms = Atoms::new(vec![(829, 0), (12_251_526, 1), (2_798_745, 1), (829, 0)]);
        assert_eq!(atoms.len(), 3);
        assert_eq!(atoms.concept(2_798_745), Some(1));
        assert_eq!(atoms.concept(1), None);
        let mut bytes = Vec::new();
        atoms.write_to(&mut bytes).expect("writes");
        assert_eq!(
            Atoms::read_from(&mut bytes.as_slice()).expect("reads"),
            atoms
        );
        assert!(matches!(
            Atoms::read_from(&mut b"XXXXXXXX\0\0\0\0".as_slice()),
            Err(AtomsError::Magic)
        ));
    }
}
