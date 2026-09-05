//! Read-only access to a built artifact.
//!
//! Every method is a point read: one key, one record, decoded with a typed
//! error. Scans belong to the offline build, never to a request path.

use std::path::{Path, PathBuf};

use concept_graph::ordinal::Ordinal;
use redb::{ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableHandle};

use crate::record::{Concept, Designation, PropertyValue, RecordError};
use crate::tables;

/// A failure while opening or reading an artifact.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database could not be opened or read.
    #[error("cannot open the artifact at {path}")]
    Open {
        /// The artifact.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: redb::DatabaseError,
    },
    /// A transaction failed.
    #[error("cannot begin a read transaction")]
    Transaction(#[from] redb::TransactionError),
    /// A table is missing or has another type.
    #[error("cannot open table {table}")]
    Table {
        /// The table name.
        table: String,
        /// The underlying error.
        #[source]
        source: redb::TableError,
    },
    /// A read failed inside the database.
    #[error("storage read failed")]
    Storage(#[from] redb::StorageError),
    /// The artifact was written by another layout version.
    #[error("artifact layout {found:?}, expected {expected:?}")]
    Layout {
        /// The layout found, if any.
        found: Option<String>,
        /// The layout this build reads.
        expected: &'static str,
    },
    /// A stored record is damaged.
    #[error("damaged record in table {table} at key {key}")]
    Record {
        /// The table name.
        table: String,
        /// The key, rendered.
        key: String,
        /// The underlying error.
        #[source]
        source: RecordError,
    },
}

/// An opened artifact.
pub struct Store {
    path: PathBuf,
    db: ReadOnlyDatabase,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

macro_rules! open_table {
    ($txn:expr, $def:expr) => {
        $txn.open_table($def).map_err(|source| StoreError::Table {
            table: $def.name().to_owned(),
            source,
        })
    };
}

impl Store {
    /// Opens the artifact at `path` read-only and checks its layout version.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the file cannot be opened, is not an
    /// artifact of this layout, or a table is missing.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let db = ReadOnlyDatabase::open(path).map_err(|source| StoreError::Open {
            path: path.to_path_buf(),
            source,
        })?;
        let store = Self {
            path: path.to_path_buf(),
            db,
        };
        let layout = store.meta(tables::META_LAYOUT)?;
        if layout.as_deref() != Some(tables::LAYOUT_VERSION) {
            return Err(StoreError::Layout {
                found: layout,
                expected: tables::LAYOUT_VERSION,
            });
        }
        Ok(store)
    }

    /// The artifact's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// An artifact-level fact by key.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::META)?;
        Ok(table.get(key)?.map(|v| v.value().to_owned()))
    }

    /// The ordinal of a native code, if the version has it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn ordinal(&self, code: &str) -> Result<Option<Ordinal>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::CODES)?;
        Ok(table.get(code)?.map(|v| Ordinal::new(v.value())))
    }

    /// The concept at `ordinal`, if any.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or the record is damaged.
    pub fn concept(&self, ordinal: Ordinal) -> Result<Option<Concept>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::CONCEPTS)?;
        table
            .get(ordinal.index())?
            .map(|v| {
                Concept::decode(v.value()).map_err(|source| StoreError::Record {
                    table: tables::CONCEPTS.name().to_owned(),
                    key: ordinal.to_string(),
                    source,
                })
            })
            .transpose()
    }

    /// The concepts at `ordinals`, in the order given, `None` where the store
    /// has no such concept.
    ///
    /// One read transaction answers the whole batch: a caller that reads many
    /// concepts (the children of one, the members of a page) pays the
    /// transaction once rather than per concept.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a record is
    /// damaged.
    pub fn concepts(
        &self,
        ordinals: impl IntoIterator<Item = Ordinal>,
    ) -> Result<Vec<Option<Concept>>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::CONCEPTS)?;
        let mut out = Vec::new();
        for ordinal in ordinals {
            let found = table
                .get(ordinal.index())?
                .map(|v| {
                    Concept::decode(v.value()).map_err(|source| StoreError::Record {
                        table: tables::CONCEPTS.name().to_owned(),
                        key: ordinal.to_string(),
                        source,
                    })
                })
                .transpose()?;
            out.push(found);
        }
        Ok(out)
    }

    /// The native codes at `ordinals`, in the order given, `None` where the
    /// store has no such concept.
    ///
    /// One read transaction answers the whole batch, and each record is
    /// decoded only as far as its code. A caller that names concepts by code
    /// (the children of one concept, the members of a page) pays neither the
    /// transaction per concept nor the rest of the record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a code is
    /// damaged.
    pub fn codes(
        &self,
        ordinals: impl IntoIterator<Item = Ordinal>,
    ) -> Result<Vec<Option<String>>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::CONCEPTS)?;
        let mut out = Vec::new();
        for ordinal in ordinals {
            let found = table
                .get(ordinal.index())?
                .map(|v| {
                    Concept::decode_code(v.value()).map_err(|source| StoreError::Record {
                        table: tables::CONCEPTS.name().to_owned(),
                        key: ordinal.to_string(),
                        source,
                    })
                })
                .transpose()?;
            out.push(found);
        }
        Ok(out)
    }

    /// Every designation of `ordinal`, in index order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a record is damaged.
    pub fn designations(&self, ordinal: Ordinal) -> Result<Vec<Designation>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::DESIGNATIONS)?;
        let mut out = Vec::new();
        for entry in table.range((ordinal.index(), 0)..(ordinal.index(), u32::MAX))? {
            let (key, value) = entry?;
            out.push(
                Designation::decode(value.value()).map_err(|source| StoreError::Record {
                    table: tables::DESIGNATIONS.name().to_owned(),
                    key: format!("{:?}", key.value()),
                    source,
                })?,
            );
        }
        Ok(out)
    }

    /// The acceptability ordinal of a designation in a language reference set.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn acceptability(
        &self,
        ordinal: Ordinal,
        designation: u32,
        language_refset: u32,
    ) -> Result<Option<u32>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::ACCEPTABILITY)?;
        Ok(table
            .get((ordinal.index(), designation, language_refset))?
            .map(|v| v.value()))
    }

    /// The preferred designation of `ordinal` for a language reference set and
    /// designation use, as the build precomputed it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or the record is damaged.
    pub fn preferred(
        &self,
        ordinal: Ordinal,
        language_refset: u32,
        use_ordinal: u32,
    ) -> Result<Option<Designation>, StoreError> {
        let txn = self.db.begin_read()?;
        let preferred = open_table!(txn, tables::PREFERRED)?;
        let Some(index) = preferred.get((ordinal.index(), language_refset, use_ordinal))? else {
            return Ok(None);
        };
        let designations = open_table!(txn, tables::DESIGNATIONS)?;
        designations
            .get((ordinal.index(), index.value()))?
            .map(|v| {
                Designation::decode(v.value()).map_err(|source| StoreError::Record {
                    table: tables::DESIGNATIONS.name().to_owned(),
                    key: format!("({ordinal}, {})", index.value()),
                    source,
                })
            })
            .transpose()
    }

    /// The preferred designation of `ordinal` in the first of `language_refsets`
    /// that names one `accept` admits.
    ///
    /// One read transaction answers the whole walk, and the walk stops at the
    /// first accepted designation. A caller choosing a display asks the
    /// reference sets in order until one carries the language it wants, so a
    /// transaction per reference set would make the cost the match position.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a record is
    /// damaged.
    pub fn preferred_first(
        &self,
        ordinal: Ordinal,
        language_refsets: impl IntoIterator<Item = u32>,
        use_ordinal: u32,
        accept: impl Fn(&Designation) -> bool,
    ) -> Result<Option<Designation>, StoreError> {
        let txn = self.db.begin_read()?;
        let preferred = open_table!(txn, tables::PREFERRED)?;
        let designations = open_table!(txn, tables::DESIGNATIONS)?;
        for refset in language_refsets {
            let Some(index) = preferred.get((ordinal.index(), refset, use_ordinal))? else {
                continue;
            };
            let Some(value) = designations.get((ordinal.index(), index.value()))? else {
                continue;
            };
            let designation =
                Designation::decode(value.value()).map_err(|source| StoreError::Record {
                    table: tables::DESIGNATIONS.name().to_owned(),
                    key: format!("({ordinal}, {})", index.value()),
                    source,
                })?;
            if accept(&designation) {
                return Ok(Some(designation));
            }
        }
        Ok(None)
    }

    /// Every property of `ordinal`, as `(property key ordinal, values)`, in key order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read or a record is damaged.
    pub fn properties(
        &self,
        ordinal: Ordinal,
    ) -> Result<Vec<(u32, Vec<PropertyValue>)>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, tables::PROPERTIES)?;
        let mut out = Vec::new();
        for entry in table.range((ordinal.index(), 0)..(ordinal.index(), u32::MAX))? {
            let (key, value) = entry?;
            let values =
                PropertyValue::decode_list(value.value()).map_err(|source| StoreError::Record {
                    table: tables::PROPERTIES.name().to_owned(),
                    key: format!("{:?}", key.value()),
                    source,
                })?;
            out.push((key.value().1, values));
        }
        Ok(out)
    }

    /// A vocabulary entry: the name of a property key, designation use,
    /// language reference set, or acceptability ordinal.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn vocabulary(&self, kind: Vocabulary, ordinal: u32) -> Result<Option<String>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, kind.table())?;
        Ok(table.get(ordinal)?.map(|v| v.value().to_owned()))
    }

    /// The ordinal of a vocabulary name, by scanning the (small) vocabulary table.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be read.
    pub fn vocabulary_ordinal(
        &self,
        kind: Vocabulary,
        name: &str,
    ) -> Result<Option<u32>, StoreError> {
        let txn = self.db.begin_read()?;
        let table = open_table!(txn, kind.table())?;
        for entry in table.iter()? {
            let (key, value) = entry?;
            if value.value() == name {
                return Ok(Some(key.value()));
            }
        }
        Ok(None)
    }
}

/// The small name tables of an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vocabulary {
    /// Property key names.
    PropertyKeys,
    /// Designation use codes.
    DesignationUses,
    /// Language reference set codes.
    LanguageRefsets,
    /// Acceptability codes.
    Acceptabilities,
}

impl Vocabulary {
    fn table(self) -> redb::TableDefinition<'static, u32, &'static str> {
        match self {
            Self::PropertyKeys => tables::PROPERTY_KEYS,
            Self::DesignationUses => tables::DESIGNATION_USES,
            Self::LanguageRefsets => tables::LANGUAGE_REFSETS,
            Self::Acceptabilities => tables::ACCEPTABILITIES,
        }
    }
}
