//! Writing an artifact, offline, in one transaction.
//!
//! The build tool feeds concepts, designations, acceptability, properties, and
//! the hierarchy blob; `finish` computes the preferred designations and
//! commits. Two builds from the same input produce the same bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ferroterm_graph::ordinal::Ordinal;
use redb::{Database, ReadableTable, TableHandle, WriteTransaction};

use crate::record::{Concept, Designation, PropertyValue};
use crate::tables;

/// A failure while building.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The database could not be created.
    #[error("cannot create the artifact at {path}")]
    Create {
        /// The artifact.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: redb::DatabaseError,
    },
    /// A transaction failed.
    #[error("transaction failed")]
    Transaction(#[from] redb::TransactionError),
    /// A table could not be opened.
    #[error("cannot open table")]
    Table(#[from] redb::TableError),
    /// A write failed inside the database.
    #[error("storage write failed")]
    Storage(#[from] redb::StorageError),
    /// The commit failed.
    #[error("commit failed")]
    Commit(#[from] redb::CommitError),
    /// A vocabulary name was registered twice with different ordinals.
    #[error("{kind} {name:?} is already ordinal {existing}, not {requested}")]
    Vocabulary {
        /// The vocabulary.
        kind: String,
        /// The name.
        name: String,
        /// The ordinal it already has.
        existing: u32,
        /// The ordinal requested.
        requested: u32,
    },
}

/// The acceptability that marks a designation as preferred in a language
/// reference set, as the code system spells it (for SNOMED, the SCTID of
/// `|Preferred|`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferredRule {
    /// The acceptability ordinal meaning preferred.
    pub preferred: u32,
}

/// An artifact under construction.
pub struct StoreBuilder {
    path: PathBuf,
    db: Database,
    txn: Option<WriteTransaction>,
    acceptability: BTreeMap<(u32, u32, u32), u32>,
    designation_uses: BTreeMap<(u32, u32), u32>,
    concepts: u64,
}

impl std::fmt::Debug for StoreBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreBuilder")
            .field("path", &self.path)
            .field("concepts", &self.concepts)
            .finish_non_exhaustive()
    }
}

impl StoreBuilder {
    /// Creates the artifact at `path`, replacing any file there, and records
    /// the layout version, system, and version.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the file cannot be created or written.
    pub fn create(path: &Path, system: &str, version: &str) -> Result<Self, BuildError> {
        if path.exists() {
            std::fs::remove_file(path).map_err(|source| BuildError::Create {
                path: path.to_path_buf(),
                source: redb::DatabaseError::Storage(redb::StorageError::Io(source)),
            })?;
        }
        let db = Database::create(path).map_err(|source| BuildError::Create {
            path: path.to_path_buf(),
            source,
        })?;
        let txn = db.begin_write()?;
        {
            let mut meta = txn.open_table(tables::META)?;
            meta.insert(tables::META_LAYOUT, tables::LAYOUT_VERSION)?;
            meta.insert(tables::META_SYSTEM, system)?;
            meta.insert(tables::META_VERSION, version)?;
            // Every table exists even when empty, so readers never meet a missing table.
            txn.open_table(tables::CODES)?;
            txn.open_table(tables::CONCEPTS)?;
            txn.open_table(tables::DESIGNATIONS)?;
            txn.open_table(tables::ACCEPTABILITY)?;
            txn.open_table(tables::PREFERRED)?;
            txn.open_table(tables::PROPERTIES)?;
            txn.open_table(tables::PROPERTY_KEYS)?;
            txn.open_table(tables::DESIGNATION_USES)?;
            txn.open_table(tables::LANGUAGE_REFSETS)?;
            txn.open_table(tables::ACCEPTABILITIES)?;
            txn.open_table(tables::BLOBS)?;
        }
        Ok(Self {
            path: path.to_path_buf(),
            db,
            txn: Some(txn),
            acceptability: BTreeMap::new(),
            designation_uses: BTreeMap::new(),
            concepts: 0,
        })
    }

    fn txn(&self) -> Result<&WriteTransaction, BuildError> {
        self.txn.as_ref().ok_or_else(|| {
            BuildError::Transaction(redb::TransactionError::Storage(redb::StorageError::Io(
                std::io::Error::other("the builder is already finished"),
            )))
        })
    }

    /// Registers a name in a vocabulary table under `ordinal`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Vocabulary`] when the name already has another ordinal.
    pub fn vocabulary(
        &mut self,
        kind: crate::store::Vocabulary,
        ordinal: u32,
        name: &str,
    ) -> Result<(), BuildError> {
        let table_def = match kind {
            crate::store::Vocabulary::PropertyKeys => tables::PROPERTY_KEYS,
            crate::store::Vocabulary::DesignationUses => tables::DESIGNATION_USES,
            crate::store::Vocabulary::LanguageRefsets => tables::LANGUAGE_REFSETS,
            crate::store::Vocabulary::Acceptabilities => tables::ACCEPTABILITIES,
        };
        let txn = self.txn()?;
        let mut table = txn.open_table(table_def)?;
        for entry in table.iter()? {
            let (key, value) = entry?;
            if value.value() == name && key.value() != ordinal {
                return Err(BuildError::Vocabulary {
                    kind: table_def.name().to_owned(),
                    name: name.to_owned(),
                    existing: key.value(),
                    requested: ordinal,
                });
            }
        }
        table.insert(ordinal, name)?;
        Ok(())
    }

    /// Stores a concept at `ordinal` and indexes its code.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the write fails.
    pub fn concept(&mut self, ordinal: Ordinal, concept: &Concept) -> Result<(), BuildError> {
        let txn = self.txn()?;
        txn.open_table(tables::CODES)?
            .insert(concept.code.as_str(), ordinal.index())?;
        txn.open_table(tables::CONCEPTS)?
            .insert(ordinal.index(), concept.encode().as_slice())?;
        self.concepts = self.concepts.saturating_add(1);
        Ok(())
    }

    /// Stores designation `index` of `ordinal`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the write fails.
    pub fn designation(
        &mut self,
        ordinal: Ordinal,
        index: u32,
        designation: &Designation,
    ) -> Result<(), BuildError> {
        let txn = self.txn()?;
        txn.open_table(tables::DESIGNATIONS)?
            .insert((ordinal.index(), index), designation.encode().as_slice())?;
        self.designation_uses
            .insert((ordinal.index(), index), designation.use_ordinal);
        Ok(())
    }

    /// Records the acceptability of designation `index` of `ordinal` in a language reference set.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the write fails.
    pub fn acceptability(
        &mut self,
        ordinal: Ordinal,
        index: u32,
        language_refset: u32,
        acceptability: u32,
    ) -> Result<(), BuildError> {
        let txn = self.txn()?;
        txn.open_table(tables::ACCEPTABILITY)?
            .insert((ordinal.index(), index, language_refset), acceptability)?;
        self.acceptability
            .insert((ordinal.index(), index, language_refset), acceptability);
        Ok(())
    }

    /// Stores the values of property `key` on `ordinal`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the write fails.
    pub fn properties(
        &mut self,
        ordinal: Ordinal,
        key: u32,
        values: &[PropertyValue],
    ) -> Result<(), BuildError> {
        let txn = self.txn()?;
        txn.open_table(tables::PROPERTIES)?.insert(
            (ordinal.index(), key),
            PropertyValue::encode_list(values).as_slice(),
        )?;
        Ok(())
    }

    /// Stores a named blob.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the write fails.
    pub fn blob(&mut self, name: &str, bytes: &[u8]) -> Result<(), BuildError> {
        let txn = self.txn()?;
        txn.open_table(tables::BLOBS)?.insert(name, bytes)?;
        Ok(())
    }

    /// Computes the preferred designations per language reference set and
    /// use, records the concept count, and commits.
    ///
    /// A designation is preferred when its acceptability equals
    /// `rule.preferred`; with several, the lowest index wins, deterministically.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when a write or the commit fails.
    pub fn finish(mut self, rule: &PreferredRule) -> Result<PathBuf, BuildError> {
        let txn = self.txn.take().ok_or_else(|| {
            BuildError::Transaction(redb::TransactionError::Storage(redb::StorageError::Io(
                std::io::Error::other("the builder is already finished"),
            )))
        })?;
        {
            let mut preferred = txn.open_table(tables::PREFERRED)?;
            for ((concept, index, refset), acceptability) in &self.acceptability {
                if *acceptability != rule.preferred {
                    continue;
                }
                let Some(use_ordinal) = self.designation_uses.get(&(*concept, *index)) else {
                    continue;
                };
                let key = (*concept, *refset, *use_ordinal);
                let current = preferred.get(key)?.map(|v| v.value());
                if current.is_none_or(|existing| *index < existing) {
                    preferred.insert(key, *index)?;
                }
            }
            let mut meta = txn.open_table(tables::META)?;
            meta.insert(tables::META_CONCEPTS, self.concepts.to_string().as_str())?;
        }
        txn.commit()?;
        drop(self.db);
        Ok(self.path)
    }
}
