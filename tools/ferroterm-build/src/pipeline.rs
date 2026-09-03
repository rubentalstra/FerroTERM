//! The pipeline: read the Snapshot, number everything, write the artifacts.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use ferroterm_graph::attributes::{self, Attributes, AttributesError};
use ferroterm_graph::closure::{Closure, ClosureError};
use ferroterm_graph::csr::{Csr, CsrError};
use ferroterm_graph::identifiers::{Identifiers, IdentifiersError};
use ferroterm_graph::members::{MembersError, Memberships};
use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::persist::Hierarchy;
use ferroterm_graph::refsets::{self, MemberRow, RefsetMembers, RefsetsError};
use ferroterm_rf2::component::{
    AlternateIdentifier, Concept, ConcreteRelationship, ConcreteValue, Description, Relationship,
    Rows,
};
use ferroterm_rf2::constants;
use ferroterm_rf2::edition::{Edition, EditionError};
use ferroterm_rf2::file::{ContentType, FieldKind, Release, ReleaseError, ReleaseType};
use ferroterm_rf2::id::{ConceptId, DescriptionId, RefsetId};
use ferroterm_rf2::reader::Rf2Error;
use ferroterm_rf2::refset::{
    FieldValue, LanguageMember, Members, ModuleDependencyMember, ViewError,
};
use ferroterm_rf2::time::EffectiveTime;
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
pub const MANIFEST_VERSION: u32 = 2;
/// The hierarchy artifact (`ferroterm-graph`), beside the store.
pub const HIERARCHY_FILE: &str = "hierarchy.bin";
/// The designation index (`ferroterm-text`), beside the store.
pub const TEXT_FILE: &str = "text.bin";
/// The reference set memberships file beside the store.
pub const REFSETS_FILE: &str = "refsets.bin";
/// The attribute relationships with their role groups and concrete values.
pub const ATTRIBUTES_FILE: &str = "attributes.bin";
/// The reference set member rows with their fields.
pub const MEMBERS_FILE: &str = "members.bin";
/// The alternate identifiers.
pub const IDENTIFIERS_FILE: &str = "identifiers.bin";

/// The fixed property keys the SNOMED loader writes, by ordinal; every
/// attribute type found in the release follows, keyed by its SCTID.
///
/// `parent` follows the FHIR `$lookup` property of that name
/// (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>); the others
/// are the RF2 concept columns. The attribute properties are the SNOMED CT on
/// FHIR rule that every concept-model attribute is a property keyed by the
/// attribute's concept id (<https://hl7.org/fhir/R4B/snomedct.html>).
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
    /// The reference set memberships cannot be serialized.
    #[error("cannot serialize the reference set memberships")]
    Members(#[from] MembersError),
    /// The attribute relationships cannot be built or serialized.
    #[error("cannot build the attribute relationships")]
    Attributes(#[from] AttributesError),
    /// The reference set member rows cannot be built or serialized.
    #[error("cannot build the reference set member rows")]
    Refsets(#[from] RefsetsError),
    /// The alternate identifiers cannot be serialized.
    #[error("cannot serialize the alternate identifiers")]
    Identifiers(#[from] IdentifiersError),
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
    /// The hierarchy file.
    pub hierarchy: PathBuf,
    /// The designation index file.
    pub text: PathBuf,
    /// The manifest file.
    pub manifest: PathBuf,
    /// Concepts written, active and inactive.
    pub concepts: u64,
    /// Designations written.
    pub designations: u64,
    /// Active inferred is-a edges.
    pub is_a_edges: u64,
    /// Reference sets with at least one active concept member.
    pub refsets: u64,
    /// Active attribute relationships (every inferred relationship that is not is-a).
    pub attributes: u64,
    /// Active reference set member rows kept with their fields.
    pub member_rows: u64,
    /// Alternate identifiers.
    pub identifiers: u64,
    /// Distinct words in the designation index.
    pub words: u64,
    /// The designation languages present (RF2 `languageCode`), sorted.
    pub languages: Vec<String>,
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
    let relationships = read_relationships(&release, &ordinals)?;
    let designations = read_designations(&release, &ordinals)?;
    let (refsets, acceptabilities) = read_acceptabilities(&release, &designations)?;
    let memberships = read_memberships(&release, &ordinals)?;
    let member_tables = read_member_tables(&release, &ordinals)?;
    let identifiers = read_identifiers(&release, &ordinals)?;
    let attribute_graph = relationships.graph(ordinal_of(concepts.len(), "concepts")?)?;

    let store_path = out.join(STORE_FILE);
    let version_uri = edition.version_uri();
    let mut builder = StoreBuilder::create(&store_path, "http://snomed.info/sct", &version_uri)?;
    write_vocabularies(&mut builder, &refsets, &relationships.attribute_types)?;
    let is_a_edges = write_concepts(&mut builder, &concepts, &ordinals, &relationships)?;
    let designation_count = write_designations(&mut builder, &designations)?;
    for ((ordinal, index), memberships) in &acceptabilities {
        for (refset, acceptability) in memberships {
            builder.acceptability(*ordinal, *index, *refset, *acceptability)?;
        }
    }
    let hierarchy = build_hierarchy(&concepts, &relationships.is_a)?;
    let hierarchy_path = out.join(HIERARCHY_FILE);
    let mut graph_bytes = Vec::new();
    hierarchy.write_to(&mut graph_bytes)?;
    std::fs::write(&hierarchy_path, &graph_bytes).map_err(io_error(&hierarchy_path))?;
    let (text_bytes, words) = build_text(&designations, &acceptabilities)?;
    let mut languages: Vec<String> = designations
        .iter()
        .map(|placed| placed.record.language.clone())
        .collect();
    languages.sort();
    languages.dedup();
    let text_path = out.join(TEXT_FILE);
    std::fs::write(&text_path, &text_bytes).map_err(io_error(&text_path))?;
    let mut member_bytes = Vec::new();
    memberships.write_to(&mut member_bytes)?;
    let refsets_path = out.join(REFSETS_FILE);
    std::fs::write(&refsets_path, &member_bytes).map_err(io_error(&refsets_path))?;
    write_ecl_files(out, &attribute_graph, &member_tables, &identifiers)?;
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
        "hierarchy": HIERARCHY_FILE,
        "text": TEXT_FILE,
        "refsets": REFSETS_FILE,
        "attributes": ATTRIBUTES_FILE,
        "members": MEMBERS_FILE,
        "identifiers": IDENTIFIERS_FILE,
        "concepts": concepts.len(),
        "designations": designation_count,
        "isAEdges": is_a_edges,
        "referenceSets": memberships.len(),
        "memberships": memberships.total(),
        "attributeRows": attribute_graph.edges(),
        "memberRows": member_tables.total(),
        "alternateIdentifiers": identifiers.len(),
        "words": words,
        "languages": languages,
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
        hierarchy: hierarchy_path,
        text: text_path,
        manifest: manifest_path,
        concepts: u64::try_from(concepts.len()).unwrap_or(u64::MAX),
        designations: designation_count,
        is_a_edges,
        refsets: u64::try_from(memberships.len()).unwrap_or(u64::MAX),
        attributes: u64::try_from(attribute_graph.edges()).unwrap_or(u64::MAX),
        member_rows: member_tables.total(),
        identifiers: u64::try_from(identifiers.len()).unwrap_or(u64::MAX),
        words,
        languages,
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

/// The active inferred relationships: is-a edges as (child, parent) ordinals,
/// sorted, and every other attribute as a property value per (source, type).
struct Relationships {
    is_a: Vec<(Ordinal, Ordinal)>,
    /// Attribute type SCTIDs, sorted; the position is the key ordinal offset.
    attribute_types: Vec<ConceptId>,
    /// Per source ordinal, per attribute type, the values in file order.
    attributes: BTreeMap<(Ordinal, ConceptId), Vec<record::PropertyValue>>,
    /// Every attribute row with its role group, for the graph.
    edges: Vec<(Ordinal, u32, ConceptId, attributes::Value)>,
}

impl Relationships {
    /// The attribute graph: the rows keyed by the sorted type list.
    fn graph(&self, nodes: u32) -> Result<Attributes, Error> {
        let types: Vec<u64> = self.attribute_types.iter().map(|t| t.value()).collect();
        let mut edges = Vec::with_capacity(self.edges.len());
        for (source, group, type_id, value) in &self.edges {
            let kind = self
                .attribute_types
                .binary_search(type_id)
                .map_err(|_| Error::TooMany("attribute types"))?;
            edges.push(attributes::Edge {
                source: *source,
                group: *group,
                kind: ordinal_of(kind, "attribute types")?,
                value: value.clone(),
            });
        }
        Ok(Attributes::build(nodes, types, edges)?)
    }
}

fn read_relationships(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<Relationships, Error> {
    let mut is_a = Vec::new();
    let mut attributes: BTreeMap<(Ordinal, ConceptId), Vec<record::PropertyValue>> =
        BTreeMap::new();
    let mut edges = Vec::new();
    let lookup = |relationship: &str, concept: ConceptId| {
        ordinals
            .get(&concept)
            .copied()
            .ok_or_else(|| Error::UnknownConcept {
                relationship: relationship.to_owned(),
                concept,
            })
    };
    for file in release.of_type(&ContentType::Relationship) {
        for relationship in Rows::<_, Relationship>::open(&file.path)? {
            let relationship = relationship?;
            if !relationship.base.active {
                continue;
            }
            let id = relationship.id.to_string();
            let source = lookup(&id, relationship.source_id)?;
            let destination = lookup(&id, relationship.destination_id)?;
            if relationship.type_id == constants::IS_A {
                is_a.push((source, destination));
            } else {
                attributes
                    .entry((source, relationship.type_id))
                    .or_default()
                    .push(record::PropertyValue::Concept(destination));
                edges.push((
                    source,
                    relationship.relationship_group,
                    relationship.type_id,
                    attributes::Value::Concept(destination),
                ));
            }
        }
    }
    for file in release.of_type(&ContentType::RelationshipConcreteValues) {
        for relationship in Rows::<_, ConcreteRelationship>::open(&file.path)? {
            let relationship = relationship?;
            if !relationship.base.active {
                continue;
            }
            let id = relationship.id.to_string();
            let source = lookup(&id, relationship.source_id)?;
            // NOTE: RF2 spells a numeric concrete value with a `#` prefix and a
            // string one in quotes; the reader strips both and keeps the kind.
            let (value, edge) = match relationship.value {
                ConcreteValue::Number(number) => (
                    record::PropertyValue::Decimal(number.clone()),
                    attributes::Value::Number(number),
                ),
                ConcreteValue::String(text) => (
                    record::PropertyValue::String(text.clone()),
                    attributes::Value::String(text),
                ),
            };
            attributes
                .entry((source, relationship.type_id))
                .or_default()
                .push(value);
            edges.push((
                source,
                relationship.relationship_group,
                relationship.type_id,
                edge,
            ));
        }
    }
    is_a.sort_unstable();
    is_a.dedup();
    let mut attribute_types: Vec<ConceptId> = attributes.keys().map(|(_, t)| *t).collect();
    attribute_types.sort_unstable();
    attribute_types.dedup();
    Ok(Relationships {
        is_a,
        attribute_types,
        attributes,
        edges,
    })
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

/// The active concept members of every reference set that references
/// concepts: the simple, association, attribute value, and map reference
/// sets, whatever their content; the language reference sets reference
/// descriptions and are read as acceptabilities instead.
fn read_memberships(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<Memberships, Error> {
    let mut memberships = Memberships::new();
    for file in release.refsets().filter(|f| f.name.summary != "Language") {
        let ContentType::Refset(kinds) = &file.name.content_type else {
            continue;
        };
        for member in Members::open(&file.path, kinds)? {
            let member = member?;
            if !member.active {
                continue;
            }
            // NOTE: a member referencing a description or a relationship is not a
            // concept membership; that is "absent" here, not a defect of the file.
            let Ok(concept) = ConceptId::try_from(member.referenced_component_id) else {
                continue;
            };
            if let Some(ordinal) = ordinals.get(&concept) {
                memberships.insert(member.refset_id.concept().value(), *ordinal);
            }
        }
    }
    Ok(memberships)
}

/// The `YYYYMMDD` of an effective time as a number.
fn compact_time(time: EffectiveTime) -> u32 {
    time.compact().parse().unwrap_or_default()
}

/// The fields and rows of one reference set before they become a table.
type PendingTable = (Vec<(String, refsets::FieldKind)>, Vec<MemberRow>);

/// The active members of every concept-referencing reference set, with their
/// fields; the language reference sets live in the store as acceptability and
/// the OWL axiom reference sets carry text no ECL filter reads.
fn read_member_tables(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<RefsetMembers, Error> {
    let mut tables: BTreeMap<u64, PendingTable> = BTreeMap::new();
    for file in release
        .refsets()
        .filter(|f| f.name.summary != "Language" && !f.name.summary.starts_with("OWL"))
    {
        let ContentType::Refset(kinds) = &file.name.content_type else {
            continue;
        };
        for member in Members::open(&file.path, kinds)? {
            let member = member?;
            if !member.active {
                continue;
            }
            let Ok(concept) = ConceptId::try_from(member.referenced_component_id) else {
                continue;
            };
            let Some(&ordinal) = ordinals.get(&concept) else {
                continue;
            };
            let entry = tables
                .entry(member.refset_id.concept().value())
                .or_insert_with(|| {
                    let fields = member
                        .fields
                        .iter()
                        .zip(kinds)
                        .map(|((name, _), kind)| {
                            (
                                name.clone(),
                                match kind {
                                    FieldKind::Component => refsets::FieldKind::Component,
                                    FieldKind::Integer => refsets::FieldKind::Integer,
                                    FieldKind::String => refsets::FieldKind::String,
                                },
                            )
                        })
                        .collect();
                    (fields, Vec::new())
                });
            let values = member
                .fields
                .into_iter()
                .map(|(_, value)| match value {
                    FieldValue::Component(id) => ConceptId::try_from(id)
                        .ok()
                        .and_then(|c| ordinals.get(&c))
                        .map_or(refsets::FieldValue::Component(id.value()), |o| {
                            refsets::FieldValue::Concept(*o)
                        }),
                    FieldValue::Integer(value) => refsets::FieldValue::Integer(value),
                    FieldValue::String(text) => refsets::FieldValue::String(text),
                })
                .collect();
            entry.1.push(MemberRow {
                concept: ordinal,
                effective_time: compact_time(member.effective_time),
                module: member.module_id.concept().value(),
                values,
            });
        }
    }
    let mut members = RefsetMembers::new();
    for (refset, (fields, rows)) in tables {
        members.insert(refset, &fields, rows)?;
    }
    Ok(members)
}

/// The active alternate identifiers of the release's concepts.
fn read_identifiers(
    release: &Release,
    ordinals: &BTreeMap<ConceptId, Ordinal>,
) -> Result<Identifiers, Error> {
    let mut entries = Vec::new();
    for file in release.of_type(&ContentType::Identifier) {
        for identifier in Rows::<_, AlternateIdentifier>::open(&file.path)? {
            let identifier = identifier?;
            if !identifier.base.active {
                continue;
            }
            let Ok(concept) = ConceptId::try_from(identifier.referenced_component_id) else {
                continue;
            };
            if let Some(&ordinal) = ordinals.get(&concept) {
                entries.push((
                    identifier.identifier_scheme_id.value(),
                    identifier.alternate_identifier,
                    ordinal,
                ));
            }
        }
    }
    Ok(Identifiers::new(entries))
}

fn write_vocabularies(
    builder: &mut StoreBuilder,
    refsets: &[RefsetId],
    attribute_types: &[ConceptId],
) -> Result<(), Error> {
    for (i, key) in PROPERTY_KEYS.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::PropertyKeys,
            ordinal_of(i, "property keys")?,
            key,
        )?;
    }
    for (i, attribute) in attribute_types.iter().enumerate() {
        builder.vocabulary(
            Vocabulary::PropertyKeys,
            ordinal_of(PROPERTY_KEYS.len().saturating_add(i), "property keys")?,
            &attribute.to_string(),
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
    relationships: &Relationships,
) -> Result<u64, Error> {
    let mut parents: BTreeMap<Ordinal, Vec<record::PropertyValue>> = BTreeMap::new();
    for (child, parent) in &relationships.is_a {
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
        for ((_, attribute), values) in relationships
            .attributes
            .range((ordinal, ConceptId::published(0))..)
            .take_while(|((source, _), _)| *source == ordinal)
        {
            let position = relationships
                .attribute_types
                .binary_search(attribute)
                .map_err(|_| Error::TooMany("attribute types"))?;
            let key = ordinal_of(
                PROPERTY_KEYS.len().saturating_add(position),
                "property keys",
            )?;
            builder.properties(ordinal, key, values)?;
        }
    }
    Ok(u64::try_from(relationships.is_a.len()).unwrap_or(u64::MAX))
}

fn write_designations(builder: &mut StoreBuilder, designations: &[Placed]) -> Result<u64, Error> {
    for placed in designations {
        builder.designation(placed.ordinal, placed.index, &placed.record)?;
    }
    Ok(u64::try_from(designations.len()).unwrap_or(u64::MAX))
}

/// Writes the files the ECL evaluator reads beside the store.
fn write_ecl_files(
    out: &Path,
    attributes: &Attributes,
    members: &RefsetMembers,
    identifiers: &Identifiers,
) -> Result<(), Error> {
    let mut bytes = Vec::new();
    attributes.write_to(&mut bytes)?;
    let path = out.join(ATTRIBUTES_FILE);
    std::fs::write(&path, &bytes).map_err(io_error(&path))?;
    bytes.clear();
    members.write_to(&mut bytes)?;
    let path = out.join(MEMBERS_FILE);
    std::fs::write(&path, &bytes).map_err(io_error(&path))?;
    bytes.clear();
    identifiers.write_to(&mut bytes)?;
    let path = out.join(IDENTIFIERS_FILE);
    std::fs::write(&path, &bytes).map_err(io_error(&path))?;
    Ok(())
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
