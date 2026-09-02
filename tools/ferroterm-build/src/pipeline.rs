//! The pipeline: read the Snapshot, number everything, write the artifacts.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use ferroterm_graph::closure::{Closure, ClosureError};
use ferroterm_graph::csr::{Csr, CsrError};
use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::persist::Hierarchy;
use ferroterm_rf2::component::{Concept, Description, Relationship, Rows};
use ferroterm_rf2::constants;
use ferroterm_rf2::edition::{Edition, EditionError};
use ferroterm_rf2::file::{ContentType, Release, ReleaseError, ReleaseType};
use ferroterm_rf2::id::{ConceptId, DescriptionId, RefsetId};
use ferroterm_rf2::reader::Rf2Error;
use ferroterm_rf2::refset::{LanguageMember, Members, ModuleDependencyMember, ViewError};
use ferroterm_store::builder::{BuildError, PreferredRule, StoreBuilder};
use ferroterm_store::record;
use ferroterm_store::store::Vocabulary;
use ferroterm_store::tables;
use ferroterm_text::index::{IndexBuilder, Input};
use serde_json::json;

/// The store file name inside the output directory.
pub const STORE_FILE: &str = "store.redb";
/// The manifest file name inside the output directory.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The manifest layout this tool writes.
pub const MANIFEST_VERSION: u32 = 1;

/// The property keys the SNOMED loader writes, by ordinal.
///
/// `parent` follows the FHIR `$lookup` property of that name
/// (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>); the others
/// are the RF2 concept columns.
pub const PROPERTY_KEYS: [&str; 3] = ["parent", "definitionStatus", "module"];

/// The designation-use ordinals: FSN, synonym, definition, in that order.
pub const DESIGNATION_USES: [ConceptId; 3] = [
    constants::FULLY_SPECIFIED_NAME,
    constants::SYNONYM,
    constants::DEFINITION,
];

/// The acceptability ordinals: preferred, acceptable, in that order.
pub const ACCEPTABILITIES: [ConceptId; 2] = [constants::PREFERRED, constants::ACCEPTABLE];

/// A build failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The release directory does not read as an RF2 Snapshot.
    #[error("cannot open the RF2 release")]
    Release(#[from] ReleaseError),
    /// A file the release needs is missing.
    #[error("the release has no {0} file")]
    MissingFile(&'static str),
    /// An RF2 row does not parse.
    #[error("cannot read an RF2 row")]
    Rf2(#[from] Rf2Error),
    /// A reference set member does not fit its typed view.
    #[error("a reference set member does not fit its view")]
    View(#[from] ViewError),
    /// A language reference set member refers to something other than a description.
    #[error("language reference set member {member} refers to {component}, not a description")]
    NotADescription {
        /// The member.
        member: String,
        /// The referenced component.
        component: String,
    },
    /// The module dependency reference set does not identify one edition.
    #[error("cannot identify the edition")]
    Edition(#[from] EditionError),
    /// A relationship names a concept the release does not define.
    #[error("relationship {relationship} names unknown concept {concept}")]
    UnknownConcept {
        /// The relationship.
        relationship: String,
        /// The concept it names.
        concept: ConceptId,
    },
    /// The is-a graph has a cycle.
    #[error("the inferred is-a hierarchy is not acyclic")]
    Closure(#[from] ClosureError),
    /// An edge is out of range (cannot happen after numbering; kept as a typed error).
    #[error("cannot build the adjacency")]
    Csr(#[from] CsrError),
    /// The store cannot be written.
    #[error("cannot write the store")]
    Store(#[from] BuildError),
    /// The hierarchy cannot be serialized.
    #[error("cannot serialize the hierarchy")]
    Graph(#[from] ferroterm_graph::persist::PersistError),
    /// The text index cannot be built or serialized.
    #[error("cannot build the designation index")]
    TextBuild(#[from] ferroterm_text::index::BuildError),
    /// The text index cannot be serialized.
    #[error("cannot serialize the designation index")]
    TextPersist(#[from] ferroterm_text::persist::PersistError),
    /// The output directory or manifest cannot be written.
    #[error("cannot write {path}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// More components than a `u32` ordinal addresses.
    #[error("the release has more {0} than the artifact can number")]
    TooMany(&'static str),
}

/// What a build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The SNOMED edition URI (`http://snomed.info/sct/{module}`).
    pub edition_uri: String,
    /// The edition version URI.
    pub version_uri: String,
    /// The store file.
    pub store: PathBuf,
    /// The manifest file.
    pub manifest: PathBuf,
    /// Concepts written, active and inactive.
    pub concepts: u64,
    /// Designations written.
    pub designations: u64,
    /// Active inferred is-a edges.
    pub is_a_edges: u64,
    /// Distinct words in the designation index.
    pub words: u64,
}

fn ordinal_of(count: usize, what: &'static str) -> Result<u32, Error> {
    u32::try_from(count).map_err(|_| Error::TooMany(what))
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Builds the artifacts for the Snapshot under `rf2` into `out`.
///
/// # Errors
///
/// Returns [`Error`] when the release does not read, the edition cannot be
/// identified, the hierarchy has a cycle, or an artifact cannot be written.
pub fn build(rf2: &Path, out: &Path) -> Result<Report, Error> {
    let release = Release::open(rf2, ReleaseType::Snapshot)?;
    let edition = identify_edition(&release)?;
    std::fs::create_dir_all(out).map_err(io_error(out))?;

    let concepts = read_concepts(&release)?;
    let ordinals: BTreeMap<ConceptId, Ordinal> = concepts
        .iter()
        .enumerate()
        .map(|(i, concept)| Ok((concept.id, Ordinal::new(ordinal_of(i, "concepts")?))))
        .collect::<Result<_, Error>>()?;
    let is_a = read_is_a(&release, &ordinals)?;
    let designations = read_designations(&release, &ordinals)?;
    let (refsets, acceptabilities) = read_acceptabilities(&release, &designations)?;

    let store_path = out.join(STORE_FILE);
    let version_uri = edition.version_uri();
    let mut builder = StoreBuilder::create(&store_path, "http://snomed.info/sct", &version_uri)?;
    write_vocabularies(&mut builder, &refsets)?;
    let is_a_edges = write_concepts(&mut builder, &concepts, &ordinals, &is_a)?;
    let designation_count = write_designations(&mut builder, &designations)?;
    for ((ordinal, index), memberships) in &acceptabilities {
        for (refset, acceptability) in memberships {
            builder.acceptability(*ordinal, *index, *refset, *acceptability)?;
        }
    }
    let hierarchy = build_hierarchy(&concepts, &is_a)?;
    let mut graph_bytes = Vec::new();
    hierarchy.write_to(&mut graph_bytes)?;
    builder.blob(tables::BLOB_HIERARCHY, &graph_bytes)?;
    let (text_bytes, words) = build_text(&designations, &acceptabilities)?;
    builder.blob(tables::BLOB_TEXT, &text_bytes)?;
    builder.finish(&PreferredRule { preferred: 0 })?;

    let manifest_path = out.join(MANIFEST_FILE);
    let manifest = json!({
        "manifest": MANIFEST_VERSION,
        "system": "http://snomed.info/sct",
        "edition": edition.edition_uri(),
        "version": version_uri,
        "releaseDate": release.date().to_string(),
        "store": STORE_FILE,
        "storeLayout": tables::LAYOUT_VERSION,
        "concepts": concepts.len(),
        "designations": designation_count,
        "isAEdges": is_a_edges,
        "words": words,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|source| Error::Io {
        path: manifest_path.clone(),
        source: io::Error::other(source),
    })?;
    std::fs::write(&manifest_path, format!("{text}\n")).map_err(io_error(&manifest_path))?;

    Ok(Report {
        edition_uri: edition.edition_uri(),
        version_uri,
        store: store_path,
        manifest: manifest_path,
        concepts: u64::try_from(concepts.len()).unwrap_or(u64::MAX),
        designations: designation_count,
        is_a_edges,
        words,
    })
}

fn identify_edition(release: &Release) -> Result<Edition, Error> {
    let file = release
        .refsets()
        .find(|f| f.name.summary == "ModuleDependency")
        .ok_or(Error::MissingFile("module dependency reference set"))?;
    let ContentType::Refset(kinds) = &file.name.content_type else {
        return Err(Error::MissingFile("module dependency reference set"));
    };
    let mut members = Vec::new();
    for member in Members::open(&file.path, kinds)? {
        members.push(ModuleDependencyMember::try_from(member?)?);
    }
    Ok(Edition::identify(&members, release.date())?)
}

/// Every concept row, sorted by identifier so ordinals are stable.
fn read_concepts(release: &Release) -> Result<Vec<Concept>, Error> {
    let file = release
        .of_type(&ContentType::Concept)
        .next()
        .ok_or(Error::MissingFile("concept"))?;
    let mut concepts = Vec::new();
    for concept in Rows::<_, Concept>::open(&file.path)? {
        concepts.push(concept?);
    }
    concepts.sort_by_key(|concept| concept.id);
    Ok(concepts)
}

/// The active inferred is-a edges as (child, parent) ordinals, sorted.
fn read_is_a(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<Vec<(Ordinal, Ordinal)>, Error> {
    let mut edges = Vec::new();
    for file in release.of_type(&ContentType::Relationship) {
        for relationship in Rows::<_, Relationship>::open(&file.path)? {
            let relationship = relationship?;
            if !relationship.base.active || relationship.type_id != constants::IS_A {
                continue;
            }
            let lookup = |concept: ConceptId| {
                ordinals
                    .get(&concept)
                    .copied()
                    .ok_or_else(|| Error::UnknownConcept {
                        relationship: relationship.id.to_string(),
                        concept,
                    })
            };
            edges.push((
                lookup(relationship.source_id)?,
                lookup(relationship.destination_id)?,
            ));
        }
    }
    edges.sort_unstable();
    edges.dedup();
    Ok(edges)
}

/// One designation, placed under its concept.
#[derive(Debug)]
struct Placed {
    id: DescriptionId,
    ordinal: Ordinal,
    index: u32,
    record: record::Designation,
}

/// Every description and text definition, numbered per concept in identifier order.
fn read_designations(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<Vec<Placed>, Error> {
    let mut rows: Vec<Description> = Vec::new();
    for content in [ContentType::Description, ContentType::TextDefinition] {
        for file in release.of_type(&content) {
            for description in Rows::<_, Description>::open(&file.path)? {
                rows.push(description?);
            }
        }
    }
    rows.sort_by_key(|row| row.id);
    let mut per_concept: BTreeMap<Ordinal, u32> = BTreeMap::new();
    let mut placed = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(ordinal) = ordinals.get(&row.concept_id).copied() else {
            // NOTE: a description of a concept outside the release is not an
            // error in RF2 (the concept lives in a dependency the release omits);
            // it has no home in this store and is skipped.
            continue;
        };
        let index = per_concept.entry(ordinal).or_insert(0);
        let use_ordinal = DESIGNATION_USES
            .iter()
            .position(|use_id| *use_id == row.type_id)
            .map(|p| ordinal_of(p, "designation uses"))
            .transpose()?
            .unwrap_or(1);
        placed.push(Placed {
            id: row.id,
            ordinal,
            index: *index,
            record: record::Designation {
                id: Some(row.id.to_string()),
                term: row.term,
                language: row.language_code,
                use_ordinal,
                active: row.base.active,
            },
        });
        *index = index.checked_add(1).ok_or(Error::TooMany("designations"))?;
    }
    Ok(placed)
}

type Acceptabilities = BTreeMap<(Ordinal, u32), Vec<(u32, u32)>>;

/// The language reference sets (by ordinal) and, per designation, its
/// (refset ordinal, acceptability ordinal) memberships.
fn read_acceptabilities(
    release: &Release,
    designations: &[Placed],
) -> Result<(Vec<RefsetId>, Acceptabilities), Error> {
    let by_id: BTreeMap<DescriptionId, (Ordinal, u32)> = designations
        .iter()
        .map(|d| (d.id, (d.ordinal, d.index)))
        .collect();
    let mut members: Vec<(RefsetId, DescriptionId, u32)> = Vec::new();
    for file in release.refsets().filter(|f| f.name.summary == "Language") {
        let ContentType::Refset(kinds) = &file.name.content_type else {
            continue;
        };
        for member in Members::open(&file.path, kinds)? {
            let member = LanguageMember::try_from(member?)?;
            if !member.member.active {
                continue;
            }
            let description = DescriptionId::try_from(member.member.referenced_component_id)
                .map_err(|_| Error::NotADescription {
                    member: member.member.id.to_string(),
                    component: member.member.referenced_component_id.to_string(),
                })?;
            let acceptability = ACCEPTABILITIES
                .iter()
                .position(|a| *a == member.acceptability_id)
                .map(|p| ordinal_of(p, "acceptabilities"))
                .transpose()?
                .unwrap_or(1);
            members.push((member.member.refset_id, description, acceptability));
        }
    }
    members.sort_unstable();
    let mut refsets: Vec<RefsetId> = members.iter().map(|m| m.0).collect();
    refsets.sort_unstable();
    refsets.dedup();
    let mut acceptabilities: Acceptabilities = BTreeMap::new();
    for (refset, description, acceptability) in members {
        let Some(place) = by_id.get(&description).copied() else {
            continue;
        };
        let refset_ordinal = refsets
            .binary_search(&refset)
            .map_err(|_| Error::TooMany("language reference sets"))
            .and_then(|p| ordinal_of(p, "language reference sets"))?;
        acceptabilities
            .entry(place)
            .or_default()
            .push((refset_ordinal, acceptability));
    }
    Ok((refsets, acceptabilities))
}

fn write_vocabularies(builder: &mut StoreBuilder, refsets: &[RefsetId]) -> Result<(), Error> {
    for (i, key) in PROPERTY_KEYS.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::PropertyKeys,
            ordinal_of(i, "property keys")?,
            key,
        )?;
    }
    for (i, use_id) in DESIGNATION_USES.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::DesignationUses,
            ordinal_of(i, "designation uses")?,
            &use_id.to_string(),
        )?;
    }
    for (i, acceptability) in ACCEPTABILITIES.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::Acceptabilities,
            ordinal_of(i, "acceptabilities")?,
            &acceptability.to_string(),
        )?;
    }
    for (i, refset) in refsets.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::LanguageRefsets,
            ordinal_of(i, "language reference sets")?,
            &refset.to_string(),
        )?;
    }
    Ok(())
}

/// Writes every concept and its properties; returns the is-a edge count.
fn write_concepts(
    builder: &mut StoreBuilder,
    concepts: &[Concept],
    ordinals: &BTreeMap<ConceptId, Ordinal>,
    is_a: &[(Ordinal, Ordinal)],
) -> Result<u64, Error> {
    let mut parents: BTreeMap<Ordinal, Vec<record::PropertyValue>> = BTreeMap::new();
    for (child, parent) in is_a {
        parents
            .entry(*child)
            .or_default()
            .push(record::PropertyValue::Concept(*parent));
    }
    for (i, concept) in concepts.iter().enumerate() {
        let ordinal = Ordinal::new(ordinal_of(i, "concepts")?);
        let module = ordinals.get(&concept.base.module_id.concept()).copied();
        builder.concept(
            ordinal,
            &record::Concept {
                code: concept.id.to_string(),
                active: concept.base.active,
                effective_time: Some(concept.base.effective_time.compact()),
                module,
            },
        )?;
        if let Some(values) = parents.get(&ordinal) {
            builder.properties(ordinal, 0, values)?;
        }
        builder.properties(
            ordinal,
            1,
            &[record::PropertyValue::Code(
                concept.definition_status_id.to_string(),
            )],
        )?;
        builder.properties(
            ordinal,
            2,
            &[record::PropertyValue::Code(
                concept.base.module_id.to_string(),
            )],
        )?;
    }
    Ok(u64::try_from(is_a.len()).unwrap_or(u64::MAX))
}

fn write_designations(builder: &mut StoreBuilder, designations: &[Placed]) -> Result<u64, Error> {
    for placed in designations {
        builder.designation(placed.ordinal, placed.index, &placed.record)?;
    }
    Ok(u64::try_from(designations.len()).unwrap_or(u64::MAX))
}

fn build_hierarchy(concepts: &[Concept], is_a: &[(Ordinal, Ordinal)]) -> Result<Hierarchy, Error> {
    let nodes = ordinal_of(concepts.len(), "concepts")?;
    let csr = Csr::build(nodes, is_a.iter().copied())?;
    let closure = Closure::compute(&csr)?;
    Ok(Hierarchy { is_a: csr, closure })
}

fn build_text(
    designations: &[Placed],
    acceptabilities: &Acceptabilities,
) -> Result<(Vec<u8>, u64), Error> {
    let mut index = IndexBuilder::new();
    for placed in designations {
        let refsets: Vec<u32> = acceptabilities
            .get(&(placed.ordinal, placed.index))
            .map(|m| m.iter().map(|(refset, _)| *refset).collect())
            .unwrap_or_default();
        index.add(&Input {
            concept: placed.ordinal,
            index: placed.index,
            term: &placed.record.term,
            language: &placed.record.language,
            use_ordinal: placed.record.use_ordinal,
            active: placed.record.active,
            refsets: &refsets,
        })?;
    }
    let index = index.build()?;
    let mut bytes = Vec::new();
    ferroterm_text::persist::write_to(&index, &mut bytes)?;
    Ok((bytes, u64::try_from(index.words()).unwrap_or(u64::MAX)))
}
