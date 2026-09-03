//! A synthetic SNOMED CT edition as an artifact directory.
//!
//! Written with the store, graph, and text writers the way `ferroterm-build`
//! lays it out: invented concepts in an invented namespace with valid check
//! digits, and no SNOMED content.

use std::path::Path;

use ferroterm_graph::closure::Closure;
use ferroterm_graph::csr::Csr;
use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::persist::Hierarchy;
use ferroterm_rf2::constants;
use ferroterm_rf2::id::with_check_digit;
use ferroterm_store::builder::{PreferredRule, StoreBuilder};
use ferroterm_store::record::{Concept, Designation, PropertyValue};
use ferroterm_store::store::Vocabulary;
use ferroterm_store::tables;
use ferroterm_text::index::{IndexBuilder, Input};

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

/// The item number behind each ordinal, so `sctid(item(CAT))` is the cat's code.
#[must_use]
pub fn item(ordinal: u32) -> u32 {
    ordinal + 1
}

/// (refset ordinal, acceptability ordinal).
type Memberships = Vec<(u32, u32)>;
/// (term, language, use ordinal, active, memberships).
type DesignationRow = (&'static str, &'static str, u32, bool, Memberships);

struct Row {
    ordinal: u32,
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
    Store(ferroterm_store::builder::BuildError),
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

impl From<ferroterm_store::builder::BuildError> for FixtureError {
    fn from(e: ferroterm_store::builder::BuildError) -> Self {
        Self::Store(e)
    }
}

impl From<std::io::Error> for FixtureError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Writes the edition under `dir` (`store.redb`, `hierarchy.bin`, `text.bin`, and `manifest.json`).
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
            active: false,
            defined: false,
            designations: vec![
                ("Fish (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Fish", "en", syn, true, vec![]),
            ],
        },
        Row {
            ordinal: FUR,
            active: true,
            defined: false,
            designations: vec![
                ("Fur (synthetic)", "en", fsn, true, vec![(gb, preferred)]),
                ("Fur", "en", syn, true, vec![(gb, preferred)]),
            ],
        },
        Row {
            ordinal: COVERING,
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
    ];
    let is_a = [
        (ANIMAL, TOP),
        (CAT, ANIMAL),
        (DOG, ANIMAL),
        (FUR, TOP),
        (COVERING, TOP),
        (LEGS, TOP),
    ];

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
                code: sctid(item(row.ordinal)),
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
    // The cat has covering fur and four legs.
    builder.properties(
        Ordinal::new(CAT),
        attribute_key(COVERING),
        &[PropertyValue::Concept(Ordinal::new(FUR))],
    )?;
    builder.properties(
        Ordinal::new(CAT),
        attribute_key(LEGS),
        &[PropertyValue::Decimal(String::from("4"))],
    )?;

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
    ferroterm_text::persist::write_to(&index, &mut text_bytes)
        .map_err(|e| FixtureError::Text(e.to_string()))?;
    std::fs::write(dir.join("text.bin"), &text_bytes)?;
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
        "concepts": rows.len(),
        "languages": ["en", "nl"],
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| FixtureError::Io(std::io::Error::other(e)))?;
    std::fs::write(dir.join("manifest.json"), text)?;
    Ok(())
}
