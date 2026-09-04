//! The byte encodings of the stored records.
//!
//! Little-endian, length-prefixed strings, a tag byte per property value.
//! Encoding and decoding are inverse; a decode failure names what was found.

use std::fmt;

use concept_graph::ordinal::Ordinal;

/// A damaged record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    /// The bytes end before the record does.
    #[error("record truncated at byte {at}")]
    Truncated {
        /// The offset where more bytes were expected.
        at: usize,
    },
    /// A string is not UTF-8.
    #[error("string at byte {at} is not UTF-8")]
    Utf8 {
        /// The offset of the string.
        at: usize,
        /// The cause.
        #[source]
        source: std::str::Utf8Error,
    },
    /// A property value tag is not one this build knows.
    #[error("unknown property value tag {tag}")]
    Tag {
        /// The tag byte.
        tag: u8,
    },
    /// Bytes remain after the record.
    #[error("{remaining} trailing byte(s) after the record")]
    Trailing {
        /// How many bytes remain.
        remaining: usize,
    },
}

/// A stored concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concept {
    /// The native code, for example the SCTID.
    pub code: String,
    /// Whether the concept is active in this version.
    pub active: bool,
    /// The version the concept last changed, as the code system writes it.
    pub effective_time: Option<String>,
    /// The module or owner concept, when the code system has one.
    pub module: Option<Ordinal>,
}

/// A stored designation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Designation {
    /// The native identifier of the designation, when the code system has one.
    pub id: Option<String>,
    /// The text.
    pub term: String,
    /// The BCP 47 language.
    pub language: String,
    /// The designation use (a `DESIGNATION_USES` ordinal).
    pub use_ordinal: u32,
    /// Whether the designation is active.
    pub active: bool,
}

/// One typed property value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// A reference to another concept of the same version.
    Concept(Ordinal),
    /// A code the code system defines.
    Code(String),
    /// A string.
    String(String),
    /// An integer.
    Integer(i64),
    /// A boolean.
    Boolean(bool),
    /// A decimal in its lexical form.
    Decimal(String),
    /// A date or time in its lexical form.
    DateTime(String),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concept(o) => write!(f, "{o}"),
            Self::Code(s) | Self::String(s) | Self::Decimal(s) | Self::DateTime(s) => {
                f.write_str(s)
            }
            Self::Integer(i) => write!(f, "{i}"),
            Self::Boolean(b) => write!(f, "{b}"),
        }
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(u32::try_from(s.len()).unwrap_or(u32::MAX));
        self.0.extend_from_slice(s.as_bytes());
    }
    fn opt_str(&mut self, s: Option<&str>) {
        match s {
            Some(s) => {
                self.u8(1);
                self.str(s);
            }
            None => self.u8(0),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RecordError> {
        let end = self
            .at
            .checked_add(n)
            .ok_or(RecordError::Truncated { at: self.at })?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(RecordError::Truncated { at: self.at })?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, RecordError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RecordError::Truncated { at: self.at })
    }
    fn u32(&mut self) -> Result<u32, RecordError> {
        // The error names the offset; the slice-length error adds nothing to it.
        let Ok(bytes) = <[u8; 4]>::try_from(self.take(4)?) else {
            return Err(RecordError::Truncated { at: self.at });
        };
        Ok(u32::from_le_bytes(bytes))
    }
    fn i64(&mut self) -> Result<i64, RecordError> {
        let Ok(bytes) = <[u8; 8]>::try_from(self.take(8)?) else {
            return Err(RecordError::Truncated { at: self.at });
        };
        Ok(i64::from_le_bytes(bytes))
    }
    fn str(&mut self) -> Result<String, RecordError> {
        let Ok(len) = usize::try_from(self.u32()?) else {
            return Err(RecordError::Truncated { at: self.at });
        };
        let at = self.at;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|source| RecordError::Utf8 { at, source })
    }
    fn opt_str(&mut self) -> Result<Option<String>, RecordError> {
        match self.u8()? {
            0 => Ok(None),
            _ => self.str().map(Some),
        }
    }
    fn finish(self) -> Result<(), RecordError> {
        let remaining = self.bytes.len().saturating_sub(self.at);
        if remaining == 0 {
            Ok(())
        } else {
            Err(RecordError::Trailing { remaining })
        }
    }
}

impl Concept {
    /// The record's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.str(&self.code);
        w.u8(u8::from(self.active));
        w.opt_str(self.effective_time.as_deref());
        match self.module {
            Some(m) => {
                w.u8(1);
                w.u32(m.index());
            }
            None => w.u8(0),
        }
        w.0
    }

    /// Decodes a record.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError`] for truncated, non-UTF-8, or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut r = Reader { bytes, at: 0 };
        let code = r.str()?;
        let active = r.u8()? != 0;
        let effective_time = r.opt_str()?;
        let module = match r.u8()? {
            0 => None,
            _ => Some(Ordinal::new(r.u32()?)),
        };
        r.finish()?;
        Ok(Self {
            code,
            active,
            effective_time,
            module,
        })
    }
}

impl Designation {
    /// The record's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.opt_str(self.id.as_deref());
        w.str(&self.term);
        w.str(&self.language);
        w.u32(self.use_ordinal);
        w.u8(u8::from(self.active));
        w.0
    }

    /// Decodes a record.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError`] for truncated, non-UTF-8, or trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, RecordError> {
        let mut r = Reader { bytes, at: 0 };
        let designation = Self {
            id: r.opt_str()?,
            term: r.str()?,
            language: r.str()?,
            use_ordinal: r.u32()?,
            active: r.u8()? != 0,
        };
        r.finish()?;
        Ok(designation)
    }
}

impl PropertyValue {
    fn encode_into(&self, w: &mut Writer) {
        match self {
            Self::Concept(o) => {
                w.u8(0);
                w.u32(o.index());
            }
            Self::Code(s) => {
                w.u8(1);
                w.str(s);
            }
            Self::String(s) => {
                w.u8(2);
                w.str(s);
            }
            Self::Integer(i) => {
                w.u8(3);
                w.i64(*i);
            }
            Self::Boolean(b) => {
                w.u8(4);
                w.u8(u8::from(*b));
            }
            Self::Decimal(s) => {
                w.u8(5);
                w.str(s);
            }
            Self::DateTime(s) => {
                w.u8(6);
                w.str(s);
            }
        }
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, RecordError> {
        Ok(match r.u8()? {
            0 => Self::Concept(Ordinal::new(r.u32()?)),
            1 => Self::Code(r.str()?),
            2 => Self::String(r.str()?),
            3 => Self::Integer(r.i64()?),
            4 => Self::Boolean(r.u8()? != 0),
            5 => Self::Decimal(r.str()?),
            6 => Self::DateTime(r.str()?),
            tag => return Err(RecordError::Tag { tag }),
        })
    }

    /// Encodes a list of values as one record.
    #[must_use]
    pub fn encode_list(values: &[Self]) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.u32(u32::try_from(values.len()).unwrap_or(u32::MAX));
        for value in values {
            value.encode_into(&mut w);
        }
        w.0
    }

    /// Decodes a list of values.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError`] for truncated bytes or an unknown tag.
    pub fn decode_list(bytes: &[u8]) -> Result<Vec<Self>, RecordError> {
        let mut r = Reader { bytes, at: 0 };
        let count = r.u32()?;
        let mut values = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for _ in 0..count {
            values.push(Self::decode_from(&mut r)?);
        }
        r.finish()?;
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::{Concept, Designation, PropertyValue, RecordError};
    use concept_graph::ordinal::Ordinal;

    #[test]
    fn records_round_trip() {
        let concept = Concept {
            code: "123456789".to_owned(),
            active: true,
            effective_time: Some("20260101".to_owned()),
            module: Some(Ordinal::new(7)),
        };
        assert_eq!(Concept::decode(&concept.encode()), Ok(concept));
        let designation = Designation {
            id: None,
            term: "Synthetisch kind".to_owned(),
            language: "nl".to_owned(),
            use_ordinal: 2,
            active: false,
        };
        assert_eq!(Designation::decode(&designation.encode()), Ok(designation));
        let values = vec![
            PropertyValue::Concept(Ordinal::new(3)),
            PropertyValue::Code("x".to_owned()),
            PropertyValue::String("s".to_owned()),
            PropertyValue::Integer(-42),
            PropertyValue::Boolean(true),
            PropertyValue::Decimal("2.50".to_owned()),
            PropertyValue::DateTime("2026-01-01".to_owned()),
        ];
        assert_eq!(
            PropertyValue::decode_list(&PropertyValue::encode_list(&values)),
            Ok(values)
        );
    }

    #[test]
    fn damaged_records_are_refused() {
        let bytes = Concept {
            code: "1".to_owned(),
            active: true,
            effective_time: None,
            module: None,
        }
        .encode();
        assert!(matches!(
            Concept::decode(&bytes[..3]),
            Err(RecordError::Truncated { .. })
        ));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            Concept::decode(&trailing),
            Err(RecordError::Trailing { remaining: 1 })
        ));
        assert!(matches!(
            PropertyValue::decode_list(&[1, 0, 0, 0, 9]),
            Err(RecordError::Tag { tag: 9 })
        ));
        assert!(matches!(
            Designation::decode(&[0, 1, 0, 0, 0, 0xff]),
            Err(RecordError::Utf8 { .. })
        ));
    }
}
