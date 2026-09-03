//! The `RxNorm` pipeline: read the release, number the concepts, write the
//! artifacts (<https://hl7.org/fhir/R4B/rxnorm.html> for what the provider
//! needs of them).
//!
//! The codes are the `RXCUI`s that have an atom from the source `RXNORM`
//! (the FHIR page: "only with SAB=RXNORM"), sorted numerically; every atom of
//! those concepts is a designation with its term type as the use, the
//! display atom first; `TTY`, `SAB`, the semantic types when the release has
//! them, and every `RXNORM` attribute (`NDC`, `RXN_AVAILABLE_STRENGTH`, ...)
//! are properties under their own names; the `RXNORM` relationships are typed
//! edges (`REL` and `RELA`) in `relations.bin`, and the atom identifiers sit
//! in `atoms.bin` for the `AUI:` filter form. No FHIR or `RxNorm`
//! specification governs the artifact layout: our own design, shared with the
//! other builds.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::relations::{Relations, RelationsError};
use ferroterm_rrf::row::Atom;
use ferroterm_rrf::{Release, RrfError};
use ferroterm_store::builder::{BuildError, PreferredRule, StoreBuilder};
use ferroterm_store::keys::{KeyTable, KeyTableError};
use ferroterm_store::record::{Concept, Designation, PropertyValue};
use ferroterm_store::store::Vocabulary;
use ferroterm_store::tables;
use ferroterm_text::index::{IndexBuilder, Input};
use serde_json::json;

use crate::pipeline::{MANIFEST_FILE, MANIFEST_VERSION, STORE_FILE, TEXT_FILE};

/// The system URI.
pub const SYSTEM: &str = "http://www.nlm.nih.gov/research/umls/rxnorm";
/// The source whose atoms are the codes.
pub const RXNORM: &str = "RXNORM";
/// The sources whose content the UMLS licence leaves unrestricted.
///
/// The build keeps the atoms of these unless told which others to include
/// (<https://www.nlm.nih.gov/research/umls/rxnorm/docs/prescribe.html>).
pub const UNRESTRICTED_SOURCES: [&str; 2] = ["RXNORM", "MTHSPL"];
/// The typed relationships file beside the store.
pub const RELATIONS_FILE: &str = "relations.bin";
/// The atom table beside the store.
pub const ATOMS_FILE: &str = "atoms.bin";
/// The property that lists a concept's `RXNORM` term types.
pub const TTY_KEY: &str = "TTY";
/// The property that lists the sources with an atom for the concept.
pub const SAB_KEY: &str = "SAB";
/// The property that lists the semantic types (the full release only).
pub const STY_KEY: &str = "STY";
/// The term types whose string is the display, most preferred first
/// (the FHIR page names `SCD` and `SBD`; the rest is our own order for the
/// concepts that have neither).
pub const DISPLAY_TTYS: [&str; 16] = [
    "SCD", "SBD", "GPCK", "BPCK", "SCDG", "SBDG", "SCDF", "SBDF", "SCDC", "SBDC", "IN", "PIN",
    "MIN", "BN", "DF", "DFG",
];

/// What the build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The release date recorded as the version.
    pub version: String,
    /// The store file.
    pub store: PathBuf,
    /// Concepts written.
    pub concepts: u64,
    /// Atoms (designations) written.
    pub atoms: u64,
    /// Typed edges written.
    pub relationships: u64,
    /// Words indexed.
    pub words: u64,
}

/// A failure of the `RxNorm` build.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The release does not read.
    #[error(transparent)]
    Release(#[from] RrfError),
    /// The release names no date and none was given.
    #[error("the release names no date; pass `--rxnorm-version`")]
    NoVersion,
    /// The store cannot be written.
    #[error(transparent)]
    Store(#[from] BuildError),
    /// The relationships cannot be built or written.
    #[error(transparent)]
    Relations(#[from] RelationsError),
    /// The atom table cannot be written.
    #[error(transparent)]
    Atoms(#[from] KeyTableError),
    /// The text index cannot be built.
    #[error(transparent)]
    Text(#[from] ferroterm_text::index::BuildError),
    /// The text index cannot be written.
    #[error(transparent)]
    TextPersist(#[from] ferroterm_text::persist::PersistError),
    /// A file cannot be written.
    #[error("cannot write {path}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// More concepts than an ordinal can number.
    #[error("too many concepts")]
    TooMany,
}

fn ordinal(index: usize) -> Result<Ordinal, Error> {
    u32::try_from(index)
        .map(Ordinal::new)
        .map_err(|_| Error::TooMany)
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// The BCP 47 tag of an `RRF` language code.
fn language(lat: &str) -> String {
    match lat {
        "ENG" => String::from("en"),
        "SPA" => String::from("es"),
        "FRE" => String::from("fr"),
        "GER" => String::from("de"),
        "DUT" => String::from("nl"),
        other => other.to_ascii_lowercase(),
    }
}

/// The atoms of one concept, the display atom first.
fn order_atoms(atoms: &mut [Atom]) {
    let rank = |atom: &Atom| {
        let tty_rank = if atom.sab == RXNORM {
            DISPLAY_TTYS
                .iter()
                .position(|t| *t == atom.tty)
                .unwrap_or(DISPLAY_TTYS.len())
        } else {
            DISPLAY_TTYS.len().saturating_add(1)
        };
        (
            tty_rank,
            atom.sab.clone(),
            atom.tty.clone(),
            atom.name.clone(),
            atom.rxaui,
        )
    };
    atoms.sort_by_cached_key(rank);
}

/// One designation to write and index.
struct Placed {
    ordinal: Ordinal,
    index: u32,
    record: Designation,
}

/// The `RXNORM` attributes of the concepts, by `RXCUI` then attribute name.
type Attributes = BTreeMap<u64, BTreeMap<String, BTreeSet<String>>>;

/// Builds the artifacts for the release under `root` into `out`.
///
/// `sources` names the sources (`SAB`) whose atoms are kept beside the
/// unrestricted ones ([`UNRESTRICTED_SOURCES`]), so a full release under a
/// UMLS licence needs an explicit list to serve the restricted sources it
/// carries.
///
/// # Errors
///
/// Returns [`Error`] when the release does not read, no date is known, or
/// an artifact cannot be written.
#[expect(
    clippy::too_many_lines,
    reason = "one pass per release table, read top to bottom"
)]
pub fn build(
    root: &Path,
    version: Option<&str>,
    sources: &[String],
    out: &Path,
) -> Result<Report, Error> {
    let release = Release::open(root)?;
    let version = version
        .map(str::to_owned)
        .or_else(|| release.version())
        .ok_or(Error::NoVersion)?;
    let kept = |sab: &str| UNRESTRICTED_SOURCES.contains(&sab) || sources.iter().any(|s| s == sab);
    let mut by_concept: BTreeMap<u64, Vec<Atom>> = BTreeMap::new();
    for atom in release.atoms()? {
        let atom = atom?;
        if !kept(&atom.sab) {
            continue;
        }
        by_concept.entry(atom.rxcui).or_default().push(atom);
    }
    by_concept.retain(|_, atoms| atoms.iter().any(|a| a.sab == RXNORM));
    let mut ordinals: BTreeMap<u64, Ordinal> = BTreeMap::new();
    for (i, rxcui) in by_concept.keys().enumerate() {
        ordinals.insert(*rxcui, ordinal(i)?);
    }
    let mut atom_concepts: BTreeMap<u64, u64> = BTreeMap::new();
    for (rxcui, atoms) in &mut by_concept {
        order_atoms(atoms);
        for atom in atoms.iter() {
            atom_concepts.insert(atom.rxaui, *rxcui);
        }
    }
    let mut attributes: Attributes = BTreeMap::new();
    for attribute in release.attributes()? {
        let attribute = attribute?;
        if attribute.sab != RXNORM || !ordinals.contains_key(&attribute.rxcui) {
            continue;
        }
        attributes
            .entry(attribute.rxcui)
            .or_default()
            .entry(attribute.name)
            .or_default()
            .insert(attribute.value);
    }
    let mut semantic_types: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    if let Some(rows) = release.semantic_types()? {
        for row in rows {
            let row = row?;
            if ordinals.contains_key(&row.rxcui) {
                semantic_types
                    .entry(row.rxcui)
                    .or_default()
                    .insert(row.name);
            }
        }
    }
    let mut types: BTreeSet<String> = BTreeSet::new();
    let mut raw_edges: Vec<(Ordinal, String, Ordinal)> = Vec::new();
    for relationship in release.relationships()? {
        let relationship = relationship?;
        if relationship.sab != RXNORM {
            continue;
        }
        let concept = |cui: Option<u64>, aui: Option<u64>| {
            cui.or_else(|| aui.and_then(|a| atom_concepts.get(&a).copied()))
                .and_then(|c| ordinals.get(&c).copied())
        };
        let (Some(first), Some(second)) = (
            concept(relationship.rxcui1, relationship.rxaui1),
            concept(relationship.rxcui2, relationship.rxaui2),
        ) else {
            continue;
        };
        // NOTE: an RRF row states that the second concept has the relationship
        // to the first (the UMLS convention the RxNorm documentation follows).
        types.insert(relationship.rel.clone());
        raw_edges.push((second, relationship.rel, first));
        if let Some(rela) = relationship.rela {
            types.insert(rela.clone());
            raw_edges.push((second, rela, first));
        }
    }
    let types: Vec<String> = types.into_iter().collect();
    let kind_of = |name: &str| {
        types
            .iter()
            .position(|t| t == name)
            .and_then(|i| u32::try_from(i).ok())
            .ok_or(Error::TooMany)
    };
    let mut edges = Vec::with_capacity(raw_edges.len());
    for (source, kind, target) in raw_edges {
        edges.push((source, kind_of(&kind)?, target));
    }
    let count = ordinal(ordinals.len())?.index();
    let relations = Relations::build(count, types, edges)?;
    std::fs::create_dir_all(out).map_err(io_error(out))?;
    let store_path = out.join(STORE_FILE);
    let mut builder = StoreBuilder::create(&store_path, SYSTEM, &version)?;
    let ttys: BTreeSet<&str> = by_concept
        .values()
        .flat_map(|atoms| atoms.iter().map(|a| a.tty.as_str()))
        .collect();
    let mut uses: BTreeMap<&str, u32> = BTreeMap::new();
    for (i, tty) in ttys.iter().enumerate() {
        let use_ordinal = ordinal(i)?.index();
        builder.vocabulary(Vocabulary::DesignationUses, use_ordinal, tty)?;
        uses.insert(tty, use_ordinal);
    }
    let mut key_names: Vec<String> = vec![TTY_KEY.to_owned(), SAB_KEY.to_owned()];
    if !semantic_types.is_empty() {
        key_names.push(STY_KEY.to_owned());
    }
    let attribute_names: BTreeSet<&str> = attributes
        .values()
        .flat_map(|by_name| by_name.keys().map(String::as_str))
        .collect();
    key_names.extend(attribute_names.into_iter().map(str::to_owned));
    let mut keys: BTreeMap<String, u32> = BTreeMap::new();
    for name in &key_names {
        let key = ordinal(keys.len())?.index();
        builder.vocabulary(Vocabulary::PropertyKeys, key, name)?;
        keys.insert(name.clone(), key);
    }
    let key = |name: &str| keys.get(name).copied().ok_or(Error::TooMany);
    let mut placed: Vec<Placed> = Vec::new();
    let mut atom_pairs: Vec<(u64, u32)> = Vec::new();
    let mut sabs: BTreeSet<String> = BTreeSet::new();
    for (rxcui, atoms) in &by_concept {
        let Some(&ordinal) = ordinals.get(rxcui) else {
            continue;
        };
        let active = atoms
            .iter()
            .any(|a| a.sab == RXNORM && !matches!(a.suppress.as_str(), "O" | "Y"));
        builder.concept(
            ordinal,
            &Concept {
                code: rxcui.to_string(),
                active,
                effective_time: None,
                module: None,
            },
        )?;
        let mut index = 0u32;
        let mut concept_ttys: BTreeSet<&str> = BTreeSet::new();
        let mut concept_sabs: BTreeSet<&str> = BTreeSet::new();
        for atom in atoms {
            atom_pairs.push((atom.rxaui, ordinal.index()));
            concept_sabs.insert(&atom.sab);
            if atom.sab == RXNORM {
                concept_ttys.insert(&atom.tty);
            }
            placed.push(Placed {
                ordinal,
                index,
                record: Designation {
                    id: Some(atom.rxaui.to_string()),
                    term: atom.name.clone(),
                    language: language(&atom.language),
                    use_ordinal: uses.get(atom.tty.as_str()).copied().unwrap_or_default(),
                    active: !matches!(atom.suppress.as_str(), "O" | "Y"),
                },
            });
            index = index.saturating_add(1);
        }
        sabs.extend(concept_sabs.iter().map(|s| (*s).to_owned()));
        let codes = |set: &BTreeSet<&str>| -> Vec<PropertyValue> {
            set.iter()
                .map(|s| PropertyValue::Code((*s).to_owned()))
                .collect()
        };
        builder.properties(ordinal, key(TTY_KEY)?, &codes(&concept_ttys))?;
        builder.properties(ordinal, key(SAB_KEY)?, &codes(&concept_sabs))?;
        if let Some(stys) = semantic_types.get(rxcui) {
            let values: Vec<PropertyValue> = stys
                .iter()
                .map(|s| PropertyValue::String(s.clone()))
                .collect();
            builder.properties(ordinal, key(STY_KEY)?, &values)?;
        }
        if let Some(by_name) = attributes.get(rxcui) {
            for (name, values) in by_name {
                let values: Vec<PropertyValue> = values
                    .iter()
                    .map(|v| PropertyValue::String(v.clone()))
                    .collect();
                builder.properties(ordinal, key(name)?, &values)?;
            }
        }
    }
    for p in &placed {
        builder.designation(p.ordinal, p.index, &p.record)?;
    }
    let mut index = IndexBuilder::new();
    for p in &placed {
        index.add(&Input {
            concept: p.ordinal,
            index: p.index,
            term: &p.record.term,
            language: &p.record.language,
            use_ordinal: p.record.use_ordinal,
            active: p.record.active,
            refsets: &[],
        })?;
    }
    let index = index.build()?;
    let mut text_bytes = Vec::new();
    ferroterm_text::persist::write_to(&index, &mut text_bytes)?;
    let text_path = out.join(TEXT_FILE);
    std::fs::write(&text_path, &text_bytes).map_err(io_error(&text_path))?;
    let mut relation_bytes = Vec::new();
    relations.write_to(&mut relation_bytes)?;
    let relations_path = out.join(RELATIONS_FILE);
    std::fs::write(&relations_path, &relation_bytes).map_err(io_error(&relations_path))?;
    let atoms = KeyTable::new(atom_pairs);
    let mut atom_bytes = Vec::new();
    atoms.write_to(&mut atom_bytes)?;
    let atoms_path = out.join(ATOMS_FILE);
    std::fs::write(&atoms_path, &atom_bytes).map_err(io_error(&atoms_path))?;
    // NOTE: the display is the first designation of each concept (the RXNORM
    // atom of the most preferred term type), so the preferred rule names use 0
    // only as a formality.
    builder.finish(&PreferredRule { preferred: 0 })?;
    let manifest = json!({
        "manifest": MANIFEST_VERSION,
        "system": SYSTEM,
        "edition": SYSTEM,
        "version": version,
        "store": STORE_FILE,
        "storeLayout": tables::LAYOUT_VERSION,
        "text": TEXT_FILE,
        "relations": RELATIONS_FILE,
        "atoms": ATOMS_FILE,
        "semanticTypes": !semantic_types.is_empty(),
        "sources": sabs,
        "concepts": ordinals.len(),
        "designations": placed.len(),
        "relationships": relations.edges(),
        "words": index.words(),
        "languages": ["en"],
    });
    let manifest_path = out.join(MANIFEST_FILE);
    let text = serde_json::to_string_pretty(&manifest).map_err(|source| Error::Io {
        path: manifest_path.clone(),
        source: io::Error::other(source),
    })?;
    std::fs::write(&manifest_path, format!("{text}\n")).map_err(io_error(&manifest_path))?;
    Ok(Report {
        version,
        store: store_path,
        concepts: u64::try_from(ordinals.len()).unwrap_or(u64::MAX),
        atoms: u64::try_from(atoms.len()).unwrap_or(u64::MAX),
        relationships: u64::try_from(relations.edges()).unwrap_or(u64::MAX),
        words: u64::try_from(index.words()).unwrap_or(u64::MAX),
    })
}
