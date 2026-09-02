//! The table set of one artifact.
//!
//! Every value is a byte string decoded by `record`, so a damaged artifact is
//! a typed error at read time rather than a panic inside the database. Keys
//! are dense ordinals; the vocabulary tables give each property key,
//! designation use, and language reference set its ordinal and name.

use redb::TableDefinition;

/// Artifact-level facts: system URI, version, edition, layout version.
pub const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
/// Native code to concept ordinal.
pub const CODES: TableDefinition<&str, u32> = TableDefinition::new("codes");
/// Concept ordinal to its encoded [`crate::record::Concept`].
pub const CONCEPTS: TableDefinition<u32, &[u8]> = TableDefinition::new("concepts");
/// `(concept ordinal, designation index)` to an encoded [`crate::record::Designation`].
pub const DESIGNATIONS: TableDefinition<(u32, u32), &[u8]> = TableDefinition::new("designations");
/// `(concept ordinal, designation index, language refset ordinal)` to an acceptability ordinal.
pub const ACCEPTABILITY: TableDefinition<(u32, u32, u32), u32> =
    TableDefinition::new("acceptability");
/// `(concept ordinal, language refset ordinal, designation use ordinal)` to the
/// preferred designation index, precomputed by the build.
pub const PREFERRED: TableDefinition<(u32, u32, u32), u32> = TableDefinition::new("preferred");
/// `(concept ordinal, property key ordinal)` to an encoded list of [`crate::record::PropertyValue`].
pub const PROPERTIES: TableDefinition<(u32, u32), &[u8]> = TableDefinition::new("properties");
/// Property key ordinal to its name.
pub const PROPERTY_KEYS: TableDefinition<u32, &str> = TableDefinition::new("property_keys");
/// Designation use ordinal to its code (for SNOMED, the description type SCTID).
pub const DESIGNATION_USES: TableDefinition<u32, &str> = TableDefinition::new("designation_uses");
/// Language reference set ordinal to its code.
pub const LANGUAGE_REFSETS: TableDefinition<u32, &str> = TableDefinition::new("language_refsets");
/// Acceptability ordinal to its code.
pub const ACCEPTABILITIES: TableDefinition<u32, &str> = TableDefinition::new("acceptabilities");
/// Named binary blobs; `is-a` holds the graph artifact.
pub const BLOBS: TableDefinition<&str, &[u8]> = TableDefinition::new("blobs");

/// The `META` key of the layout version.
pub const META_LAYOUT: &str = "layout";
/// The layout version this build writes and reads.
pub const LAYOUT_VERSION: &str = "1";
/// The `META` key of the code system URI.
pub const META_SYSTEM: &str = "system";
/// The `META` key of the code system version string.
pub const META_VERSION: &str = "version";
/// The `META` key of the number of concepts.
pub const META_CONCEPTS: &str = "concepts";
/// The `BLOBS` key of the hierarchy artifact.
pub const BLOB_HIERARCHY: &str = "is-a";
/// The blob slot holding the designation search index (`ferroterm-text`).
pub const BLOB_TEXT: &str = "text";
