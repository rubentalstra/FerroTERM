// SPDX-License-Identifier: BUSL-1.1
//! The client resources a deployment persists: `CodeSystem`, `ValueSet`, and
//! `ConceptMap` written through the REST API, in a `redb` database beside the
//! indexes.
//!
//! No FHIR specification governs the storage; this layout is our own design.
//! Each record keeps the resource as the client sent it, with the FHIR version
//! it arrived in, so a read renders it in whatever version asks and an
//! operation converts it exactly as it converts a resource from disk.

use std::path::{Path, PathBuf};

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

/// The current resources, keyed `<type>/<id>`, each a JSON [`Record`].
const RESOURCES: TableDefinition<'_, &str, &str> = TableDefinition::new("resources");

/// Every version of every resource, keyed `<type>/<id>/<versionId>`.
///
/// A version read answers from here, so the history of a resource outlives the
/// delete of its current version
/// (<https://hl7.org/fhir/R4B/http.html#vread>).
const HISTORY: TableDefinition<'_, &str, &str> = TableDefinition::new("history");

/// A resource type the server persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceType {
    /// `CodeSystem`.
    CodeSystem,
    /// `ValueSet`.
    ValueSet,
    /// `ConceptMap`.
    ConceptMap,
}

impl ResourceType {
    /// The type as the FHIR `resourceType` element spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CodeSystem => "CodeSystem",
            Self::ValueSet => "ValueSet",
            Self::ConceptMap => "ConceptMap",
        }
    }

    /// The type a `resourceType` names, when the server persists it.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "CodeSystem" => Some(Self::CodeSystem),
            "ValueSet" => Some(Self::ValueSet),
            "ConceptMap" => Some(Self::ConceptMap),
            _ => None,
        }
    }
}

/// One persisted resource.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    /// The resource type.
    pub resource_type: String,
    /// The logical id.
    pub id: String,
    /// `url`, when the resource carries one.
    pub url: Option<String>,
    /// `version`, when the resource carries one.
    pub version: Option<String>,
    /// The FHIR version the resource arrived in (`4.0.1`, `4.3.0`, ...).
    pub fhir_version: String,
    /// `meta.versionId`, counted from `1` and raised by every update.
    pub version_id: u32,
    /// `meta.lastModified`, an instant in the FHIR `instant` form.
    pub last_modified: String,
    /// The resource as the client sent it, with `id` and `meta` set.
    pub resource: serde_json::Map<String, serde_json::Value>,
}

impl Record {
    /// The `ETag` of the record's current version, the FHIR weak form
    /// (<https://hl7.org/fhir/R4B/http.html#concurrency>).
    #[must_use]
    pub fn etag(&self) -> String {
        format!("W/\"{}\"", self.version_id)
    }
}

/// A failure of the persisted store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database cannot be opened or created.
    #[error("cannot open the resource database at {path}")]
    Open {
        /// The path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<redb::DatabaseError>,
    },
    /// A transaction failed.
    #[error("the resource store transaction failed")]
    Transaction(#[from] redb::TransactionError),
    /// A table cannot be opened.
    #[error("the resource table cannot be opened")]
    Table(#[from] redb::TableError),
    /// A read or write failed.
    #[error("the resource store failed")]
    Storage(#[from] redb::StorageError),
    /// A commit failed.
    #[error("the resource store commit failed")]
    Commit(#[from] redb::CommitError),
    /// A stored record is not the JSON this build writes.
    #[error("the record at {key} does not read")]
    Record {
        /// The key.
        key: String,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
}

/// The persisted resources of one deployment.
#[derive(Debug)]
pub struct ResourceStore {
    database: Database,
    path: PathBuf,
}

impl ResourceStore {
    /// Opens the database at `path`, creating it when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Open`] when the file cannot be opened or created.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let database = Database::create(path).map_err(|source| StoreError::Open {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        // The tables exist from the first open, so a read of an empty store is
        // not a missing-table error.
        let write = database.begin_write()?;
        {
            let resources = write.open_table(RESOURCES)?;
            let history = write.open_table(HISTORY)?;
            drop(resources);
            drop(history);
        }
        write.commit()?;
        Ok(Self {
            database,
            path: path.to_path_buf(),
        })
    }

    /// The path the store lives at.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every record, sorted by key.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the store cannot be read, and
    /// [`StoreError::Record`] when a record does not parse.
    pub fn all(&self) -> Result<Vec<Record>, StoreError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(RESOURCES)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let record =
                serde_json::from_str(value.value()).map_err(|source| StoreError::Record {
                    key: key.value().to_owned(),
                    source,
                })?;
            out.push(record);
        }
        Ok(out)
    }

    /// The record of `resource_type` with `id`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the store cannot be read.
    pub fn get(&self, resource_type: ResourceType, id: &str) -> Result<Option<Record>, StoreError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(RESOURCES)?;
        let key = key_of(resource_type, id);
        let Some(value) = table.get(key.as_str())? else {
            return Ok(None);
        };
        let record = serde_json::from_str(value.value())
            .map_err(|source| StoreError::Record { key, source })?;
        Ok(Some(record))
    }

    /// The record of `resource_type` with `id` as of `version_id`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the store cannot be read.
    pub fn version(
        &self,
        resource_type: ResourceType,
        id: &str,
        version_id: u32,
    ) -> Result<Option<Record>, StoreError> {
        let read = self.database.begin_read()?;
        let table = read.open_table(HISTORY)?;
        let key = version_key_of(resource_type, id, version_id);
        let Some(value) = table.get(key.as_str())? else {
            return Ok(None);
        };
        let record = serde_json::from_str(value.value())
            .map_err(|source| StoreError::Record { key, source })?;
        Ok(Some(record))
    }

    /// Writes `record`, replacing any record of the same type and id and
    /// keeping the written version in the history.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the write does not commit.
    pub fn put(&self, record: &Record) -> Result<(), StoreError> {
        let Some(resource_type) = ResourceType::parse(&record.resource_type) else {
            return Ok(());
        };
        let key = key_of(resource_type, &record.id);
        let value = serde_json::to_string(record).map_err(|source| StoreError::Record {
            key: key.clone(),
            source,
        })?;
        let versioned = version_key_of(resource_type, &record.id, record.version_id);
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(RESOURCES)?;
            table.insert(key.as_str(), value.as_str())?;
            let mut history = write.open_table(HISTORY)?;
            history.insert(versioned.as_str(), value.as_str())?;
        }
        write.commit()?;
        Ok(())
    }

    /// Removes the record of `resource_type` with `id`; `false` when there was
    /// none.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the delete does not commit.
    pub fn delete(&self, resource_type: ResourceType, id: &str) -> Result<bool, StoreError> {
        let key = key_of(resource_type, id);
        let write = self.database.begin_write()?;
        let removed;
        {
            let mut table = write.open_table(RESOURCES)?;
            removed = table.remove(key.as_str())?.is_some();
        }
        write.commit()?;
        Ok(removed)
    }
}

/// The store key of a resource: `<type>/<id>`.
fn key_of(resource_type: ResourceType, id: &str) -> String {
    format!("{}/{id}", resource_type.name())
}

/// The history key of one version of a resource: `<type>/<id>/<versionId>`.
fn version_key_of(resource_type: ResourceType, id: &str, version_id: u32) -> String {
    format!("{}/{id}/{version_id}", resource_type.name())
}

#[cfg(test)]
mod tests {
    use super::{Record, ResourceStore, ResourceType};

    fn record(id: &str, version_id: u32) -> Record {
        Record {
            resource_type: String::from("ValueSet"),
            id: id.to_owned(),
            url: Some(format!("http://example.org/{id}")),
            version: Some(String::from("1.0")),
            fhir_version: String::from("4.3.0"),
            version_id,
            last_modified: String::from("2026-09-04T00:00:00Z"),
            resource: [
                (String::from("resourceType"), serde_json::json!("ValueSet")),
                (String::from("id"), serde_json::json!(id)),
            ]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn a_record_survives_a_reopen_and_an_update_replaces_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("resources.redb");
        {
            let store = ResourceStore::open(&path).expect("opens");
            assert!(store.all().expect("reads").is_empty());
            store.put(&record("pets", 1)).expect("writes");
            store.put(&record("colours", 1)).expect("writes");
        }
        let store = ResourceStore::open(&path).expect("reopens");
        let all = store.all().expect("reads");
        assert_eq!(all.len(), 2, "both survive the reopen");
        let stored = store
            .get(ResourceType::ValueSet, "pets")
            .expect("reads")
            .expect("pets");
        assert_eq!(stored.version_id, 1);
        assert_eq!(stored.etag(), "W/\"1\"");

        store.put(&record("pets", 2)).expect("updates");
        let stored = store
            .get(ResourceType::ValueSet, "pets")
            .expect("reads")
            .expect("pets");
        assert_eq!(stored.version_id, 2, "the update replaces the record");
        assert_eq!(store.all().expect("reads").len(), 2, "and adds none");

        assert!(
            store
                .delete(ResourceType::ValueSet, "pets")
                .expect("deletes")
        );
        assert!(
            !store
                .delete(ResourceType::ValueSet, "pets")
                .expect("deletes"),
            "deleting twice is not an error"
        );
        assert_eq!(store.all().expect("reads").len(), 1);
        assert!(
            store
                .get(ResourceType::ValueSet, "pets")
                .expect("reads")
                .is_none()
        );
        let first = store
            .version(ResourceType::ValueSet, "pets", 1)
            .expect("reads")
            .expect("the first version");
        assert_eq!(
            first.version_id, 1,
            "a delete leaves the history of the resource"
        );
        assert!(
            store
                .version(ResourceType::ValueSet, "pets", 3)
                .expect("reads")
                .is_none(),
            "a version that was never written is absent"
        );
    }
}
