//! A synthetic SNOMED CT edition as an artifact directory, and a refset-only
//! RF2 package to layer onto one.
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
/// The REPLACED BY association reference set (a published SCTID); the fish is REPLACED BY the dog.
pub const REPLACED_BY: u32 = 13;
/// The POSSIBLY EQUIVALENT TO association reference set (a published SCTID); the fish points at the dog.
pub const POSSIBLY_EQUIVALENT_TO: u32 = 14;
/// The ALTERNATIVE association reference set (a published SCTID); the fish points at the cat.
pub const ALTERNATIVE: u32 = 15;
/// The Module Dependency reference set (a published SCTID); its member is the
/// edition module, the way a real edition states what it was built on.
pub const MODULE_DEPENDENCY: u32 = 16;
/// The edition module, a concept and the member of the Module Dependency set.
pub const MODULE_CONCEPT: u32 = 17;
/// The published SCTID of the Module Dependency reference set.
pub const MODULE_DEPENDENCY_SCTID: &str = "900000000000534007";
/// The published SCTID of the historical association reference set root.
pub const HISTORICAL_SCTID: &str = "900000000000522004";
/// The published SCTID of the SAME AS association reference set.
pub const SAME_AS_SCTID: &str = "900000000000527005";
/// The published SCTID of the REPLACED BY association reference set.
pub const REPLACED_BY_SCTID: &str = "900000000000526001";
/// The published SCTID of the POSSIBLY EQUIVALENT TO association reference set.
pub const POSSIBLY_EQUIVALENT_TO_SCTID: &str = "900000000000523009";
/// The published SCTID of the ALTERNATIVE association reference set.
pub const ALTERNATIVE_SCTID: &str = "900000000000530003";

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
    // The association reference set concepts share one shape: a published SCTID,
    // the metadata fully specified name, and the bare name as the synonym.
    let association = |ordinal: u32, code: &'static str, name: &'static str| Row {
        ordinal,
        code: Some(code),
        active: true,
        defined: false,
        designations: vec![
            (name, "en", fsn, true, vec![(gb, preferred)]),
            (
                name.trim_end_matches(" (foundation metadata concept)"),
                "en",
                syn,
                true,
                vec![(gb, preferred)],
            ),
        ],
    };
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
        association(
            REPLACED_BY,
            REPLACED_BY_SCTID,
            "REPLACED BY association reference set (foundation metadata concept)",
        ),
        association(
            POSSIBLY_EQUIVALENT_TO,
            POSSIBLY_EQUIVALENT_TO_SCTID,
            "POSSIBLY EQUIVALENT TO association reference set (foundation metadata concept)",
        ),
        association(
            ALTERNATIVE,
            ALTERNATIVE_SCTID,
            "ALTERNATIVE association reference set (foundation metadata concept)",
        ),
        association(
            MODULE_DEPENDENCY,
            MODULE_DEPENDENCY_SCTID,
            "Module dependency reference set (foundation metadata concept)",
        ),
        Row {
            ordinal: MODULE_CONCEPT,
            code: None,
            active: true,
            defined: false,
            designations: vec![
                (
                    "Synthetic edition module (core metadata concept)",
                    "en",
                    fsn,
                    true,
                    vec![(gb, preferred)],
                ),
                (
                    "Synthetic edition module",
                    "en",
                    syn,
                    true,
                    vec![(gb, preferred)],
                ),
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
        (REPLACED_BY, HISTORICAL),
        (POSSIBLY_EQUIVALENT_TO, HISTORICAL),
        (ALTERNATIVE, HISTORICAL),
        (MODULE_DEPENDENCY, TOP),
        (MODULE_CONCEPT, TOP),
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
    let replaced_by = refset_id(REPLACED_BY)?;
    let possibly_equivalent_to = refset_id(POSSIBLY_EQUIVALENT_TO)?;
    let alternative = refset_id(ALTERNATIVE)?;
    memberships.insert(pets, Ordinal::new(CAT));
    memberships.insert(pets, Ordinal::new(DOG));
    memberships.insert(codes_map, Ordinal::new(CAT));
    memberships.insert(codes_map, Ordinal::new(DOG));
    memberships.insert(same_as, Ordinal::new(FISH));
    memberships.insert(replaced_by, Ordinal::new(FISH));
    memberships.insert(possibly_equivalent_to, Ordinal::new(FISH));
    memberships.insert(alternative, Ordinal::new(FISH));
    let module_dependency = refset_id(MODULE_DEPENDENCY)?;
    memberships.insert(module_dependency, Ordinal::new(MODULE_CONCEPT));
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
    // The Module Dependency reference set states which module an edition was
    // built on, in the columns RF2 gives it.
    tables
        .insert(
            module_dependency,
            &[
                (
                    String::from("sourceEffectiveTime"),
                    refsets::FieldKind::Integer,
                ),
                (
                    String::from("targetEffectiveTime"),
                    refsets::FieldKind::Integer,
                ),
            ],
            vec![member(
                MODULE_CONCEPT,
                vec![
                    refsets::FieldValue::Integer(20_260_101),
                    refsets::FieldValue::Integer(20_260_101),
                ],
            )],
        )
        .map_err(|e| graph_error(&e))?;
    tables
        .insert(
            codes_map,
            &[
                (String::from("mapGroup"), refsets::FieldKind::Integer),
                (String::from("mapPriority"), refsets::FieldKind::Integer),
                (String::from("mapRule"), refsets::FieldKind::String),
                (String::from("mapAdvice"), refsets::FieldKind::String),
                (String::from("mapTarget"), refsets::FieldKind::String),
                (String::from("correlationId"), refsets::FieldKind::Component),
            ],
            vec![
                member(
                    CAT,
                    vec![
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::String(String::from("TRUE")),
                        refsets::FieldValue::String(String::from("ALWAYS C01")),
                        refsets::FieldValue::String(String::from("C01")),
                        refsets::FieldValue::Component(447_561_005),
                    ],
                ),
                member(
                    DOG,
                    vec![
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::Integer(1),
                        refsets::FieldValue::String(String::from("TRUE")),
                        refsets::FieldValue::String(String::from("ALWAYS D01")),
                        refsets::FieldValue::String(String::from("D01")),
                        refsets::FieldValue::Component(447_561_005),
                    ],
                ),
            ],
        )
        .map_err(|e| graph_error(&e))?;
    let target_component = [(
        String::from("targetComponentId"),
        refsets::FieldKind::Component,
    )];
    for (refset, target) in [
        (same_as, CAT),
        (replaced_by, DOG),
        (possibly_equivalent_to, DOG),
        (alternative, CAT),
    ] {
        tables
            .insert(
                refset,
                &target_component,
                vec![member(
                    FISH,
                    vec![refsets::FieldValue::Concept(Ordinal::new(target))],
                )],
            )
            .map_err(|e| graph_error(&e))?;
    }
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
/// The item number of the refset-only package's module concept.
pub const PACKAGE_MODULE: u32 = 200;
/// The item number of the refset-only package's reference set concept.
pub const PACKAGE_REFSET: u32 = 201;
/// The release date the fixture writes, as an RF2 `effectiveTime`.
pub const DATE: &str = "20260101";

/// Case insensitive (`900000000000448009`), the case significance every
/// fixture description carries.
const CASE_INSENSITIVE: &str = "900000000000448009";

/// A description identifier in the invented namespace.
fn description_id(item: u32) -> String {
    with_check_digit(&format!("{item}{NAMESPACE}11"))
}

/// A reference set member identifier (RF2 spells it as a UUID).
fn member_id(item: u32) -> String {
    format!("00000000-0000-4000-8000-{item:012}")
}

/// What a refset-only package depends on and holds.
#[derive(Debug, Clone, Copy)]
pub struct Package<'a> {
    /// The edition module the package's module dependency row names.
    pub depends_on: &'a str,
    /// The version that row asks of that module (`YYYYMMDD`).
    pub target: &'a str,
    /// The edition concepts the package's simple reference set holds.
    pub members: &'a [String],
}

/// Writes one RF2 file: a tab-separated header and rows, CRLF-terminated.
fn write_rf2(
    dir: &Path,
    relative: &str,
    header: &[&str],
    rows: &[Vec<String>],
) -> std::io::Result<()> {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = header.join("\t");
    text.push_str("\r\n");
    for row in rows {
        text.push_str(&row.join("\t"));
        text.push_str("\r\n");
    }
    std::fs::write(&path, text)
}

/// Writes a refset-only RF2 Snapshot package under `dir`, the shape SNOMED
/// International publishes for a package dependent on an edition.
///
/// The package carries its own module and one simple reference set, both as
/// concepts with a fully specified name and a synonym, the language reference
/// set rows those descriptions need, the reference set's members over the
/// edition's concepts, and the module dependency row stating what it needs of
/// the edition. Its file names carry the package's own name the way a
/// derivative package does (`der2_Refset_ICNPSimpleSnapshot`), so no reader
/// may match a file summary exactly.
///
/// # Errors
///
/// Returns [`FixtureError`] when a file cannot be written; the content is
/// fixed, so a failure means the directory is at fault.
pub fn write_refset_package(dir: &Path, package: &Package<'_>) -> Result<(), FixtureError> {
    write_package_terminology(dir)?;
    write_package_refsets(dir, package)?;
    Ok(())
}

/// The package's two concepts and their descriptions.
fn write_package_terminology(dir: &Path) -> Result<(), FixtureError> {
    let module = sctid(PACKAGE_MODULE);
    let refset = sctid(PACKAGE_REFSET);
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<String>>();
    let fsn = constants::FULLY_SPECIFIED_NAME.to_string();
    let synonym = constants::SYNONYM.to_string();
    write_rf2(
        dir,
        &format!("Snapshot/Terminology/sct2_Concept_ZooSnapshot_XX{NAMESPACE}_{DATE}.txt"),
        &[
            "id",
            "effectiveTime",
            "active",
            "moduleId",
            "definitionStatusId",
        ],
        &[
            s(&[
                &module,
                DATE,
                "1",
                &module,
                &constants::PRIMITIVE.to_string(),
            ]),
            s(&[
                &refset,
                DATE,
                "1",
                &module,
                &constants::PRIMITIVE.to_string(),
            ]),
        ],
    )?;
    let describe = |item: u32, concept: &str, kind: &str, term: &str| {
        s(&[
            &description_id(item),
            DATE,
            "1",
            &module,
            concept,
            "en",
            kind,
            term,
            CASE_INSENSITIVE,
        ])
    };
    write_rf2(
        dir,
        &format!("Snapshot/Terminology/sct2_Description_ZooSnapshot-en_XX{NAMESPACE}_{DATE}.txt"),
        &[
            "id",
            "effectiveTime",
            "active",
            "moduleId",
            "conceptId",
            "languageCode",
            "typeId",
            "term",
            "caseSignificanceId",
        ],
        // The four descriptions are items 200 to 203: the module's fully
        // specified name and synonym, then the reference set's.
        &[
            describe(
                200,
                &module,
                &fsn,
                "Zoo nursing module (core metadata concept)",
            ),
            describe(201, &module, &synonym, "Zoo nursing module"),
            describe(
                202,
                &refset,
                &fsn,
                "Zoo nursing reference set (foundation metadata concept)",
            ),
            describe(203, &refset, &synonym, "Zoo nursing reference set"),
        ],
    )?;
    Ok(())
}

/// The package's language reference set, simple reference set, and module
/// dependency rows.
fn write_package_refsets(dir: &Path, package: &Package<'_>) -> Result<(), FixtureError> {
    let module = sctid(PACKAGE_MODULE);
    let refset = sctid(PACKAGE_REFSET);
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<String>>();
    let accept = |item: u32| {
        s(&[
            &member_id(item),
            DATE,
            "1",
            &module,
            GB_REFSET,
            &description_id(item),
            &constants::PREFERRED.to_string(),
        ])
    };
    write_rf2(
        dir,
        &format!(
            "Snapshot/Refset/Language/der2_cRefset_ZooLanguageSnapshot-en_XX{NAMESPACE}_{DATE}.txt"
        ),
        &[
            "id",
            "effectiveTime",
            "active",
            "moduleId",
            "refsetId",
            "referencedComponentId",
            "acceptabilityId",
        ],
        &[accept(200), accept(201), accept(202), accept(203)],
    )?;
    let members: Vec<Vec<String>> = package
        .members
        .iter()
        .enumerate()
        .map(|(i, member)| {
            s(&[
                &member_id(300 + ord(i)),
                DATE,
                "1",
                &module,
                &refset,
                member,
            ])
        })
        .collect();
    write_rf2(
        dir,
        &format!("Snapshot/Refset/Content/der2_Refset_ZooSimpleSnapshot_XX{NAMESPACE}_{DATE}.txt"),
        &[
            "id",
            "effectiveTime",
            "active",
            "moduleId",
            "refsetId",
            "referencedComponentId",
        ],
        &members,
    )?;
    write_rf2(
        dir,
        &format!(
            "Snapshot/Refset/Metadata/der2_ssRefset_ZooModuleDependencySnapshot_XX{NAMESPACE}_{DATE}.txt"
        ),
        &[
            "id",
            "effectiveTime",
            "active",
            "moduleId",
            "refsetId",
            "referencedComponentId",
            "sourceEffectiveTime",
            "targetEffectiveTime",
        ],
        &[s(&[
            &member_id(400),
            DATE,
            "1",
            &module,
            &constants::MODULE_DEPENDENCY_REFSET.to_string(),
            package.depends_on,
            DATE,
            package.target,
        ])],
    )?;
    Ok(())
}
