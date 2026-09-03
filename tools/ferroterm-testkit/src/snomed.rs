//! A synthetic SNOMED CT edition as an artifact directory.
//!
//! Written with the store, graph, and text writers the way `ferroterm-build`
//! lays it out: invented concepts in an invented namespace with valid check
//! digits, and no SNOMED content.

use std::path::Path;

use concept_graph::attributes::{self, Attributes};
use concept_graph::closure::Closure;
use concept_graph::csr::Csr;
use concept_graph::identifiers::Identifiers;
use concept_graph::members::Memberships;
use concept_graph::ordinal::Ordinal;
use concept_graph::persist::Hierarchy;
use concept_graph::refsets::{self, RefsetMembers};
use concept_store::builder::{PreferredRule, StoreBuilder};
use concept_store::record::{Concept, Designation, PropertyValue};
use concept_store::store::Vocabulary;
use concept_store::tables;
use designation_index::index::{IndexBuilder, Input};
use rf2::constants;
use rf2::id::with_check_digit;

const NAMESPACE: &str = "1234567";
/// The edition URI of the synthetic edition.
pub const EDITION: &str = "http://snomed.info/sct/91234567105";
/// The edition version URI (the `version` the provider serves).
pub const VERSION: &str = "http://snomed.info/sct/91234567105/version/20260101";
/// The GB English language reference set (a published SCTID, metadata only).
pub const GB_REFSET: &str = "900000000000508004";
/// The Dutch language reference set (a published SCTID, metadata only).
pub const NL_REFSET: &str = "31000146106";

/// A concept identifier in the invented namespace.
#[must_use]
pub fn sctid(item: u32) -> String {
    with_check_digit(&format!("{item}{NAMESPACE}10"))
}

/// The root concept.
pub const TOP: u32 = 0;
/// The animal, under the root.
pub const ANIMAL: u32 = 1;
/// The cat, under the animal; defined; has fur and four legs.
pub const CAT: u32 = 2;
/// The dog, under the animal; defined.
pub const DOG: u32 = 3;
/// The inactive fish, unclassified.
pub const FISH: u32 = 4;
/// The fur, target of the covering attribute.
pub const FUR: u32 = 5;
/// The covering attribute type.
pub const COVERING: u32 = 6;
/// The leg-count attribute type (concrete values).
pub const LEGS: u32 = 7;
/// A reference set concept whose members are the cat and the dog.
pub const PETS: u32 = 8;
/// A map reference set (`mapGroup`, `mapTarget`) whose members are the cat and the dog.
pub const CODES_MAP: u32 = 9;
/// The historical association reference set root (a published SCTID, metadata only).
pub const HISTORICAL: u32 = 10;
/// The SAME AS association reference set (a published SCTID); the fish is SAME AS the cat.
pub const SAME_AS: u32 = 11;
/// An identifier scheme whose alias is `ZOO`; the cat is `ZOO#cat-1`, the dog `ZOO#dog-1`.
pub const SCHEME: u32 = 12;
/// The published SCTID of the historical association reference set root.
pub const HISTORICAL_SCTID: &str = "900000000000522004";
/// The published SCTID of the SAME AS association reference set.
pub const SAME_AS_SCTID: &str = "900000000000527005";

/// The item number behind each ordinal, so `sctid(item(CAT))` is the cat's code.
#[must_use]
pub fn item(ordinal: u32) -> u32 {
    ordinal + 1
}

/// (refset ordinal, acceptability ordinal).
type LanguageMemberships = Vec<(u32, u32)>;
/// (term, language, use ordinal, active, memberships).
type DesignationRow = (&'static str, &'static str, u32, bool, LanguageMemberships);

struct Row {
    ordinal: u32,
    /// A published code instead of the invented one.
    code: Option<&'static str>,
    active: bool,
    defined: bool,
    designations: Vec<DesignationRow>,
}

/// A small index as a `u32` ordinal (the fixture has a handful of rows).
fn ord(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// A failure to write the fixture.
#[derive(Debug)]
pub enum FixtureError {
    /// The store could not be written.
    Store(concept_store::builder::BuildError),
    /// The hierarchy could not be built or serialized.
    Graph(String),
    /// The text index could not be built or serialized.
    Text(String),
    /// The manifest could not be written.
    Io(std::io::Error),
}

impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "store: {e}"),
            Self::Graph(e) => write!(f, "graph: {e}"),
            Self::Text(e) => write!(f, "text: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for FixtureError {}

impl From<concept_store::builder::BuildError> for FixtureError {
    fn from(e: concept_store::builder::BuildError) -> Self {
        Self::Store(e)
    }
}

impl From<std::io::Error> for FixtureError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Writes the edition under `dir` (`store.redb`, `hierarchy.bin`, `text.bin`, `refsets.bin`, `attributes.bin`, `members.bin`, `identifiers.bin`, and `manifest.json`).
///
/// # Errors
///
/// Returns [`FixtureError`] when a writer fails; the fixture is fixed content,
/// so a failure means the writers or the directory are at fault.
#[expect(
    clippy::too_many_lines,
    reason = "one synthetic edition, read top to bottom"
)]
pub fn write(dir: &Path) -> Result<(), FixtureError> {
    let module = sctid(99);
    let fsn = 0;
    let syn = 1;
    let (gb, nl) = (0, 1);
    let (preferred, acceptable) = (0, 1);
    let rows = [
        Row {
            ordinal: TOP,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Living thing (synthetic)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                ("Living thing", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: ANIMAL,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                ("Animal (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Animal", "en", syn, true, vec![(gb, preferred)]),
                ("Dier", "nl", syn, true, vec![(nl, preferred)]),
            ],
        },
        Row {
            ordinal: CAT,
            code: None,
            active: true,
            defined: true,
            designations: vec![
                ("Cat (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Cat", "en", syn, true, vec![(gb, preferred)]),
                ("Kat", "nl", syn, true, vec![(nl, preferred)]),
                ("Poes", "nl", syn, true, vec![(nl, acceptable)]),
                ("Moggy", "en", syn, false, vec![]),
            ],
        },
        Row {
            ordinal: DOG,
            code: None,
            active: true,
            defined: true,
            designations: vec![
                ("Dog (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Dog", "en", syn, true, vec![(gb, preferred)]),
                ("Hond", "nl", syn, true, vec![(nl, preferred)]),
            ],
        },
        Row {
            ordinal: FISH,
            code: None,
            active: false,
            defined: false,
            designations: vec![
                ("Fish (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Fish", "en", syn, true, vec![]),
            ],
        },
        Row {
            ordinal: FUR,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                ("Fur (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Fur", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: COVERING,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Has covering (attribute)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                ("Has covering", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: LEGS,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Leg count (attribute)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                ("Leg count", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: PETS,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Pets reference set (foundation metadata concept)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                ("Pets reference set", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: CODES_MAP,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Codes map reference set (foundation metadata concept)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                (
                    "Codes map reference set",
                    "en",
                    syn,
                    true,
                    vec![(gb, preferred)],
                ),
            ],
        },
        Row {
            ordinal: HISTORICAL,
            code: Some(HISTORICAL_SCTID),
            active: true,
            defined: false,
            designations: vec![
                (
                    "Historical association reference set (foundation metadata concept)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                (
                    "Historical association reference set",
                    "en",
                    syn,
                    true,
                    vec![(gb, preferred)],
                ),
            ],
        },
        Row {
            ordinal: SAME_AS,
            code: Some(SAME_AS_SCTID),
            active: true,
            defined: false,
            designations: vec![
                (
                    "SAME AS association reference set (foundation metadata concept)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                (
                    "SAME AS association reference set",
                    "en",
                    syn,
                    true,
                    vec![(gb, preferred)],
                ),
            ],
        },
        Row {
            ordinal: SCHEME,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Zoo code system (identifier scheme)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                ("Zoo code system", "en", syn, true, vec![(gb, preferred)]),
                ("ZOO", "en", syn, true, vec![(gb, acceptable)]),
            ],
        },
    ];
    let is_a = [
        (ANIMAL, TOP),
        (CAT, ANIMAL),
        (DOG, ANIMAL),
        (FUR, TOP),
        (COVERING, TOP),
        (LEGS, TOP),
        (PETS, TOP),
        (CODES_MAP, TOP),
        (HISTORICAL, TOP),
        (SAME_AS, HISTORICAL),
        (SCHEME, TOP),
    ];
    let code_of = |row: &Row| {
        row.code
            .map_or_else(|| sctid(item(row.ordinal)), str::to_owned)
    };

    let mut builder =
        StoreBuilder::create(&dir.join("store.redb"), "http://snomed.info/sct", VERSION)?;
    for (i, key) in ["parent", "definitionStatus", "module"].iter().enumerate() {
        builder.vocabulary(Vocabulary::PropertyKeys, ord(i), key)?;
    }
    // Attribute types after the fixed keys, sorted by SCTID as the build does.
    let mut attribute_types = [(COVERING, sctid(item(COVERING))), (LEGS, sctid(item(LEGS)))];
    attribute_types.sort_by(|a, b| a.1.cmp(&b.1));
    for (i, (_, code)) in attribute_types.iter().enumerate() {
        builder.vocabulary(Vocabulary::PropertyKeys, 3 + ord(i), code)?;
    }
    let attribute_key = |ordinal: u32| {
        3 + ord(attribute_types
            .iter()
            .position(|(o, _)| *o == ordinal)
            .unwrap_or_default())
    };
    for (i, use_id) in [
        constants::FULLY_SPECIFIED_NAME,
        constants::SYNONYM,
        constants::DEFINITION,
    ]
    .iter()
    .enumerate()
    {
        builder.vocabulary(Vocabulary::DesignationUses, ord(i), &use_id.to_string())?;
    }
    builder.vocabulary(
        Vocabulary::Acceptabilities,
        0,
        &constants::PREFERRED.to_string(),
    )?;
    builder.vocabulary(
        Vocabulary::Acceptabilities,
        1,
        &constants::ACCEPTABLE.to_string(),
    )?;
    builder.vocabulary(Vocabulary::LanguageRefsets, gb, GB_REFSET)?;
    builder.vocabulary(Vocabulary::LanguageRefsets, nl, NL_REFSET)?;

    let mut text = IndexBuilder::new();
    for row in &rows {
        let ordinal = Ordinal::new(row.ordinal);
        builder.concept(
            ordinal,
            &Concept {
                code: code_of(row),
                active: row.active,
                effective_time: Some(String::from("20260101")),
                module: None,
            },
        )?;
        let parents: Vec<PropertyValue> = is_a
            .iter()
            .filter(|(child, _)| *child == row.ordinal)
            .map(|(_, parent)| PropertyValue::Concept(Ordinal::new(*parent)))
            .collect();
        if !parents.is_empty() {
            builder.properties(ordinal, 0, &parents)?;
        }
        let status = if row.defined {
            constants::DEFINED
        } else {
            constants::PRIMITIVE
        };
        builder.properties(ordinal, 1, &[PropertyValue::Code(status.to_string())])?;
        builder.properties(ordinal, 2, &[PropertyValue::Code(module.clone())])?;
        for (index, (term, language, use_ordinal, active, memberships)) in
            row.designations.iter().enumerate()
        {
            let index = ord(index);
            builder.designation(
                ordinal,
                index,
                &Designation {
                    id: None,
                    term: (*term).to_owned(),
                    language: (*language).to_owned(),
                    use_ordinal: *use_ordinal,
                    active: *active,
                },
            )?;
            for (refset, acceptability) in memberships {
                builder.acceptability(ordinal, index, *refset, *acceptability)?;
            }
            let refsets: Vec<u32> = memberships.iter().map(|(r, _)| *r).collect();
            text.add(&Input {
                concept: ordinal,
                index,
                term,
                language,
                use_ordinal: *use_ordinal,
                active: *active,
                refsets: &refsets,
            })
            .map_err(|e| FixtureError::Text(e.to_string()))?;
        }
    }
    // The cat and the dog have covering fur and four legs; the cat states both
    // in one role group, the dog in two.
    for animal in [CAT, DOG] {
        builder.properties(
            Ordinal::new(animal),
            attribute_key(COVERING),
            &[PropertyValue::Concept(Ordinal::new(FUR))],
        )?;
        builder.properties(
            Ordinal::new(animal),
            attribute_key(LEGS),
            &[PropertyValue::Decimal(String::from("4"))],
        )?;
    }
    let graph_error = |e: &dyn std::fmt::Display| FixtureError::Graph(e.to_string());
    let attribute_types = {
        let mut types: Vec<u64> = [COVERING, LEGS]
            .iter()
            .map(|o| sctid(item(*o)).parse().unwrap_or_default())
            .collect();
        types.sort_unstable();
        types
    };
    let kind = |ordinal: u32| -> u32 {
        let code: u64 = sctid(item(ordinal)).parse().unwrap_or_default();
        ord(attribute_types
            .iter()
            .position(|t| *t == code)
            .unwrap_or_default())
    };
    let edge =
        |source: u32, group: u32, attribute: u32, value: attributes::Value| attributes::Edge {
            source: Ordinal::new(source),
            group,
            kind: kind(attribute),
            value,
        };
    let attributes = Attributes::build(
        ord(rows.len()),
        attribute_types.clone(),
        vec![
            edge(
                CAT,
                1,
                COVERING,
                attributes::Value::Concept(Ordinal::new(FUR)),
            ),
            edge(CAT, 1, LEGS, attributes::Value::Number(String::from("4"))),
            edge(
                DOG,
                1,
                COVERING,
                attributes::Value::Concept(Ordinal::new(FUR)),
            ),
            edge(DOG, 2, LEGS, attributes::Value::Number(String::from("4"))),
        ],
    )
    .map_err(|e| graph_error(&e))?;
    let mut attribute_bytes = Vec::new();
    attributes
        .write_to(&mut attribute_bytes)
        .map_err(|e| graph_error(&e))?;
    std::fs::write(dir.join("attributes.bin"), &attribute_bytes)?;

    let csr = Csr::build(
        ord(rows.len()),
        is_a.iter()
            .map(|(c, p)| (Ordinal::new(*c), Ordinal::new(*p))),
    )
    .map_err(|e| FixtureError::Graph(e.to_string()))?;
    let closure = Closure::compute(&csr).map_err(|e| FixtureError::Graph(e.to_string()))?;
    let hierarchy = Hierarchy { is_a: csr, closure };
    let mut graph_bytes = Vec::new();
    hierarchy
        .write_to(&mut graph_bytes)
        .map_err(|e| FixtureError::Graph(e.to_string()))?;
    std::fs::write(dir.join("hierarchy.bin"), &graph_bytes)?;
    let index = text
        .build()
        .map_err(|e| FixtureError::Text(e.to_string()))?;
    let mut text_bytes = Vec::new();
    designation_index::persist::write_to(&index, &mut text_bytes)
        .map_err(|e| FixtureError::Text(e.to_string()))?;
    std::fs::write(dir.join("text.bin"), &text_bytes)?;
    let mut memberships = Memberships::new();
    let refset_id = |ordinal: u32| -> Result<u64, FixtureError> {
        rows.iter()
            .find(|r| r.ordinal == ordinal)
            .map(code_of)
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| FixtureError::Graph(String::from("refset id")))
    };
    let (pets, codes_map, same_as) = (refset_id(PETS)?, refset_id(CODES_MAP)?, refset_id(SAME_AS)?);
    memberships.insert(pets, Ordinal::new(CAT));
    memberships.insert(pets, Ordinal::new(DOG));
    memberships.insert(codes_map, Ordinal::new(CAT));
    memberships.insert(codes_map, Ordinal::new(DOG));
    memberships.insert(same_as, Ordinal::new(FISH));
    let module_id: u64 = module.parse().unwrap_or_default();
    let member = |concept: u32, values: Vec<refsets::FieldValue>| refsets::MemberRow {
        concept: Ordinal::new(concept),
        effective_time: 20_260_101,
        module: module_id,
        values,
    };
    let mut tables = RefsetMembers::new();
    tables
        .insert(pets, &[], vec![member(CAT, vec![]), member(DOG, vec![])])
        .map_err(|e| graph_error(&e))?;
    tables
        .insert(
            codes_map,
            &[
                (String::from("mapGroup"), refsets::FieldKind::Integer),
                (String::from("mapTarget"), refsets::FieldKind::String),
            ],
            vec![
                member(
                    CAT,
                    vec![
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::String(String::from("C01")),
                    ],
                ),
                member(
                    DOG,
                    vec![
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::String(String::from("D01")),
                    ],
                ),
            ],
        )
        .map_err(|e| graph_error(&e))?;
    tables
        .insert(
            same_as,
            &[(
                String::from("targetComponentId"),
                refsets::FieldKind::Component,
            )],
            vec![member(
                FISH,
                vec![refsets::FieldValue::Concept(Ordinal::new(CAT))],
            )],
        )
        .map_err(|e| graph_error(&e))?;
    let mut table_bytes = Vec::new();
    tables
        .write_to(&mut table_bytes)
        .map_err(|e| graph_error(&e))?;
    std::fs::write(dir.join("members.bin"), &table_bytes)?;
    let scheme: u64 = sctid(item(SCHEME)).parse().unwrap_or_default();
    let identifiers = Identifiers::new(vec![
        (scheme, String::from("cat-1"), Ordinal::new(CAT)),
        (scheme, String::from("dog-1"), Ordinal::new(DOG)),
    ]);
    let mut identifier_bytes = Vec::new();
    identifiers
        .write_to(&mut identifier_bytes)
        .map_err(|e| graph_error(&e))?;
    std::fs::write(dir.join("identifiers.bin"), &identifier_bytes)?;
    let mut member_bytes = Vec::new();
    memberships
        .write_to(&mut member_bytes)
        .map_err(|e| FixtureError::Graph(e.to_string()))?;
    std::fs::write(dir.join("refsets.bin"), &member_bytes)?;
    builder.finish(&PreferredRule { preferred })?;

    let manifest = serde_json::json!({
        "manifest": 2,
        "system": "http://snomed.info/sct",
        "edition": EDITION,
        "version": VERSION,
        "releaseDate": "20260101",
        "store": "store.redb",
        "storeLayout": tables::LAYOUT_VERSION,
        "hierarchy": "hierarchy.bin",
        "text": "text.bin",
        "refsets": "refsets.bin",
        "attributes": "attributes.bin",
        "members": "members.bin",
        "identifiers": "identifiers.bin",
        "concepts": rows.len(),
        "languages": ["en", "nl"],
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| FixtureError::Io(std::io::Error::other(e)))?;
    std::fs::write(dir.join("manifest.json"), text)?;
    Ok(())
}
