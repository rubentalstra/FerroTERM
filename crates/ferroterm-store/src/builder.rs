//! Writing an artifact, offline, in one transaction.
//!
//! The build tool feeds concepts, designations, acceptability, properties, and
//! the hierarchy blob; `finish` computes the preferred designations and
//! commits. Two builds from the same input produce the same bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ferroterm_graph::ordinal::Ordinal;
use redb::{Database, TableHandle};

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
///
/// Rows are buffered in memory, sorted by key, and written in `finish`, each
/// table opened once. redb fills pages tightly when keys arrive in order, and
/// opening a table is not free; the first build wrote every row through its
/// own `open_table` in arrival order and took ten minutes for the NL edition.
pub struct StoreBuilder {
    path: PathBuf,
    db: Database,
    system: String,
    version: String,
    vocabularies: BTreeMap<&'static str, BTreeMap<u32, String>>,
    codes: BTreeMap<String, u32>,
    concepts: BTreeMap<u32, Vec<u8>>,
    designations: BTreeMap<(u32, u32), Vec<u8>>,
    acceptability: BTreeMap<(u32, u32, u32), u32>,
    designation_uses: BTreeMap<(u32, u32), u32>,
    properties: BTreeMap<(u32, u32), Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
}

impl std::fmt::Debug for StoreBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreBuilder")
            .field("path", &self.path)
            .field("concepts", &self.concepts.len())
            .field("designations", &self.designations.len())
            .finish_non_exhaustive()
    }
}

impl StoreBuilder {
    /// Creates the artifact at `path`, replacing any file there, and records
    /// the layout version, system, and version.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when the file cannot be created.
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
        Ok(Self {
            path: path.to_path_buf(),
            db,
            system: system.to_owned(),
            version: version.to_owned(),
            vocabularies: BTreeMap::new(),
            codes: BTreeMap::new(),
            concepts: BTreeMap::new(),
            designations: BTreeMap::new(),
            acceptability: BTreeMap::new(),
            designation_uses: BTreeMap::new(),
            properties: BTreeMap::new(),
            blobs: BTreeMap::new(),
        })
    }

    /// Names a vocabulary ordinal (a property key, designation use, language
    /// reference set, or acceptability).
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Vocabulary`] when `name` already has another ordinal.
    pub fn vocabulary(
        &mut self,
        kind: crate::store::Vocabulary,
        ordinal: u32,
        name: &str,
    ) -> Result<(), BuildError> {
        let table_name = match kind {
            crate::store::Vocabulary::PropertyKeys => tables::PROPERTY_KEYS.name(),
            crate::store::Vocabulary::DesignationUses => tables::DESIGNATION_USES.name(),
            crate::store::Vocabulary::LanguageRefsets => tables::LANGUAGE_REFSETS.name(),
            crate::store::Vocabulary::Acceptabilities => tables::ACCEPTABILITIES.name(),
        };
        let entries = self.vocabularies.entry(table_name).or_default();
        if let Some((existing, _)) = entries
            .iter()
            .find(|(key, value)| value.as_str() == name && **key != ordinal)
        {
            return Err(BuildError::Vocabulary {
                kind: table_name.to_owned(),
                name: name.to_owned(),
                existing: *existing,
                requested: ordinal,
            });
        }
        entries.insert(ordinal, name.to_owned());
        Ok(())
    }

    /// Adds a concept under `ordinal`.
    ///
    /// # Errors
    ///
    /// Cannot fail today; the signature keeps the door open for a full buffer.
    pub fn concept(&mut self, ordinal: Ordinal, concept: &Concept) -> Result<(), BuildError> {
        self.codes.insert(concept.code.clone(), ordinal.index());
        self.concepts.insert(ordinal.index(), concept.encode());
        Ok(())
    }

    /// Adds designation `index` of concept `ordinal`.
    ///
    /// # Errors
    ///
    /// Cannot fail today; the signature keeps the door open for a full buffer.
    pub fn designation(
        &mut self,
        ordinal: Ordinal,
        index: u32,
        designation: &Designation,
    ) -> Result<(), BuildError> {
        self.designations
            .insert((ordinal.index(), index), designation.encode());
        self.designation_uses
            .insert((ordinal.index(), index), designation.use_ordinal);
        Ok(())
    }

    /// Records the acceptability of a designation in a language reference set.
    ///
    /// # Errors
    ///
    /// Cannot fail today; the signature keeps the door open for a full buffer.
    pub fn acceptability(
        &mut self,
        ordinal: Ordinal,
        index: u32,
        language_refset: u32,
        acceptability: u32,
    ) -> Result<(), BuildError> {
        self.acceptability
            .insert((ordinal.index(), index, language_refset), acceptability);
        Ok(())
    }

    /// Sets the values of property `key` on concept `ordinal`.
    ///
    /// # Errors
    ///
    /// Cannot fail today; the signature keeps the door open for a full buffer.
    pub fn properties(
        &mut self,
        ordinal: Ordinal,
        key: u32,
        values: &[PropertyValue],
    ) -> Result<(), BuildError> {
        self.properties
            .insert((ordinal.index(), key), PropertyValue::encode_list(values));
        Ok(())
    }

    /// Stores a named blob (the hierarchy, the text index).
    ///
    /// # Errors
    ///
    /// Cannot fail today; the signature keeps the door open for a full buffer.
    pub fn blob(&mut self, name: &str, bytes: &[u8]) -> Result<(), BuildError> {
        self.blobs.insert(name.to_owned(), bytes.to_vec());
        Ok(())
    }

    /// Writes every buffered row in key order, computes the preferred
    /// designations per language reference set and use, records the concept
    /// count, and commits.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] when a table cannot be written or the commit fails.
    pub fn finish(self, rule: &PreferredRule) -> Result<PathBuf, BuildError> {
        let txn = self.db.begin_write()?;
        {
            let mut meta = txn.open_table(tables::META)?;
            meta.insert(tables::META_LAYOUT, tables::LAYOUT_VERSION)?;
            meta.insert(tables::META_SYSTEM, self.system.as_str())?;
            meta.insert(tables::META_VERSION, self.version.as_str())?;
            meta.insert(
                tables::META_CONCEPTS,
                self.concepts.len().to_string().as_str(),
            )?;
        }
        {
            let mut codes = txn.open_table(tables::CODES)?;
            for (code, ordinal) in &self.codes {
                codes.insert(code.as_str(), *ordinal)?;
            }
        }
        {
            let mut concepts = txn.open_table(tables::CONCEPTS)?;
            for (ordinal, bytes) in &self.concepts {
                concepts.insert(*ordinal, bytes.as_slice())?;
            }
        }
        {
            let mut designations = txn.open_table(tables::DESIGNATIONS)?;
            for (key, bytes) in &self.designations {
                designations.insert(*key, bytes.as_slice())?;
            }
        }
        {
            let mut acceptability = txn.open_table(tables::ACCEPTABILITY)?;
            for (key, value) in &self.acceptability {
                acceptability.insert(*key, *value)?;
            }
        }
        {
            let mut properties = txn.open_table(tables::PROPERTIES)?;
            for (key, bytes) in &self.properties {
                properties.insert(*key, bytes.as_slice())?;
            }
        }
        {
            let mut blobs = txn.open_table(tables::BLOBS)?;
            for (name, bytes) in &self.blobs {
                blobs.insert(name.as_str(), bytes.as_slice())?;
            }
        }
        for table_def in [
            tables::PROPERTY_KEYS,
            tables::DESIGNATION_USES,
            tables::LANGUAGE_REFSETS,
            tables::ACCEPTABILITIES,
        ] {
            let mut table = txn.open_table(table_def)?;
            if let Some(entries) = self.vocabularies.get(table_def.name()) {
                for (ordinal, name) in entries {
                    table.insert(*ordinal, name.as_str())?;
                }
            }
        }
        {
            // The preferred designation per (concept, refset, use): the lowest
            // index among those the refset marks preferred.
            let mut chosen: BTreeMap<(u32, u32, u32), u32> = BTreeMap::new();
            for ((concept, index, refset), acceptability) in &self.acceptability {
                if *acceptability != rule.preferred {
                    continue;
                }
                let Some(use_ordinal) = self.designation_uses.get(&(*concept, *index)) else {
                    continue;
                };
                chosen
                    .entry((*concept, *refset, *use_ordinal))
                    .and_modify(|existing| *existing = (*existing).min(*index))
                    .or_insert(*index);
            }
            let mut preferred = txn.open_table(tables::PREFERRED)?;
            for (key, index) in &chosen {
                preferred.insert(*key, *index)?;
            }
        }
        txn.commit()?;
        drop(self.db);
        Ok(self.path)
    }
}
