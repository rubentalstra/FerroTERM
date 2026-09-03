//! The LOINC pipeline: read the release, number everything, write the
//! artifacts (<https://hl7.org/fhir/R4B/loinc.html> for what the provider
//! needs of them).
//!
//! Terms, parts, answer lists, and answers are the concepts, in that order,
//! each group sorted by code; the multiaxial hierarchy is the graph; every
//! `Loinc.csv` column is a property under its own name; the long common
//! name, short name, consumer name, part names, answer texts, and the
//! linguistic variants are the designations, indexed for search. No FHIR or
//! LOINC specification governs the artifact layout: our own design, shared
//! with the SNOMED CT build.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use ferroterm_graph::closure::{Closure, ClosureError};
use ferroterm_graph::csr::{Csr, CsrError};
use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::persist::Hierarchy;
use ferroterm_loinc::release::{Release, ReleaseError};
use ferroterm_loinc::{answer, part, term, variant};
use ferroterm_store::builder::{BuildError, PreferredRule, StoreBuilder};
use ferroterm_store::record::{Concept, Designation, PropertyValue};
use ferroterm_store::store::Vocabulary;
use ferroterm_store::tables;
use ferroterm_text::index::{IndexBuilder, Input};
use serde_json::json;

use crate::pipeline::{HIERARCHY_FILE, MANIFEST_FILE, MANIFEST_VERSION, STORE_FILE, TEXT_FILE};

/// The system URI.
pub const SYSTEM: &str = "http://loinc.org";

/// The designation uses, by ordinal: the `DESIGNATION_USES` vocabulary.
pub const DESIGNATION_USES: [&str; 5] = [
    "LONG_COMMON_NAME",
    "SHORTNAME",
    "CONSUMER_NAME",
    "DISPLAY",
    "LinguisticVariantDisplayName",
];

/// The property keys the build adds beside the `Loinc.csv` columns.
pub const COPYRIGHT_KEY: &str = "copyright";
/// The answer lists a term is linked to (`LL…` codes).
pub const ANSWER_LIST_KEY: &str = "answer-list";
/// The answers of an answer list (`LA…` codes).
pub const ANSWERS_KEY: &str = "answers";
/// The kind of code: `term`, `part`, `answer-list`, or `answer`.
pub const KIND_KEY: &str = "kind";

/// What the build wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The LOINC version recorded.
    pub version: String,
    /// The store file.
    pub store: PathBuf,
    /// Terms written.
    pub terms: u64,
    /// Parts written.
    pub parts: u64,
    /// Answer lists written.
    pub answer_lists: u64,
    /// Designations written.
    pub designations: u64,
    /// Words indexed.
    pub words: u64,
}

/// A failure of the LOINC build.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The release does not read.
    #[error(transparent)]
    Release(#[from] ReleaseError),
    /// The release names no version and none was given.
    #[error("the release names no version; pass `--loinc-version`")]
    NoVersion,
    /// The store cannot be written.
    #[error(transparent)]
    Store(#[from] BuildError),
    /// The hierarchy cannot be built.
    #[error(transparent)]
    Csr(#[from] CsrError),
    /// The hierarchy has a cycle.
    #[error(transparent)]
    Closure(#[from] ClosureError),
    /// The hierarchy cannot be written.
    #[error(transparent)]
    Graph(#[from] ferroterm_graph::persist::PersistError),
    /// The text index cannot be built or written.
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

/// The version in a release name such as `Loinc_2.82` or `Loinc_2.82.zip`.
#[must_use]
pub fn version_from_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix("Loinc_")?;
    let rest = rest.strip_suffix(".zip").unwrap_or(rest);
    (!rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .then(|| rest.to_owned())
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

/// One designation to write and index.
struct Placed {
    ordinal: Ordinal,
    index: u32,
    record: Designation,
}

/// The numbered concepts of the release.
struct Numbered {
    ordinals: BTreeMap<String, Ordinal>,
    concepts: Vec<(Ordinal, Concept, &'static str)>,
}

fn number(
    terms: &term::Terms,
    parts: &[part::Part],
    lists: &BTreeMap<String, answer::AnswerList>,
) -> Result<Numbered, Error> {
    let mut codes: Vec<(String, bool, &'static str)> = Vec::new();
    let mut term_codes: Vec<_> = terms
        .rows
        .iter()
        .map(|t| (t.code.clone(), t.active(), "term"))
        .collect();
    term_codes.sort();
    codes.extend(term_codes);
    let mut part_codes: Vec<_> = parts
        .iter()
        .map(|p| {
            (
                p.code.clone(),
                !p.status.eq_ignore_ascii_case(term::DEPRECATED),
                "part",
            )
        })
        .collect();
    part_codes.sort();
    codes.extend(part_codes);
    for list in lists.values() {
        codes.push((list.code.clone(), true, "answer-list"));
    }
    let mut answers: Vec<_> = lists
        .values()
        .flat_map(|l| l.answers.iter().map(|a| (a.code.clone(), true, "answer")))
        .collect();
    answers.sort();
    answers.dedup();
    codes.extend(answers);
    let mut ordinals = BTreeMap::new();
    let mut concepts = Vec::with_capacity(codes.len());
    for (code, active, kind) in codes {
        if ordinals.contains_key(&code) {
            continue;
        }
        let ordinal = ordinal(concepts.len())?;
        ordinals.insert(code.to_ascii_uppercase(), ordinal);
        concepts.push((
            ordinal,
            Concept {
                code,
                active,
                effective_time: None,
                module: None,
            },
            kind,
        ));
    }
    Ok(Numbered { ordinals, concepts })
}

/// Builds the artifacts for the release under `root` into `out`.
///
/// # Errors
///
/// Returns [`Error`] when the release does not read, no version is known, the
/// hierarchy has a cycle, or an artifact cannot be written.
#[expect(
    clippy::too_many_lines,
    reason = "one pass per release table, read top to bottom"
)]
pub fn build(root: &Path, version: Option<&str>, out: &Path) -> Result<Report, Error> {
    let release = Release::open(root)?;
    let terms = term::read(&release)?;
    let parts = part::read_parts(&release)?;
    let edges = part::read_hierarchy(&release)?;
    let lists = answer::read(&release)?;
    let variants = variant::read(&release)?;
    let version = match version {
        Some(v) => v.to_owned(),
        None => terms
            .rows
            .iter()
            .filter_map(|t| t.fields.get("VersionLastChanged"))
            .max()
            .cloned()
            .ok_or(Error::NoVersion)?,
    };
    std::fs::create_dir_all(out).map_err(io_error(out))?;
    let numbered = number(&terms, &parts, &lists)?;
    let store_path = out.join(STORE_FILE);
    let mut builder = StoreBuilder::create(&store_path, SYSTEM, &version)?;
    for (i, name) in DESIGNATION_USES.iter().enumerate() {
        builder.vocabulary(Vocabulary::DesignationUses, ordinal(i)?.index(), name)?;
    }
    let mut keys: BTreeMap<String, u32> = BTreeMap::new();
    let mut key_names: Vec<String> = terms.columns.clone();
    key_names.extend([COPYRIGHT_KEY, ANSWER_LIST_KEY, ANSWERS_KEY, KIND_KEY].map(str::to_owned));
    for name in &key_names {
        let key = ordinal(keys.len())?.index();
        keys.insert(name.clone(), key);
        builder.vocabulary(Vocabulary::PropertyKeys, key, name)?;
    }
    let key = |name: &str| keys.get(name).copied().ok_or(Error::TooMany);
    let mut placed: Vec<Placed> = Vec::new();
    let mut place =
        |ordinal: Ordinal, index: &mut u32, term: &str, language: &str, use_ordinal: u32| {
            if term.is_empty() {
                return;
            }
            placed.push(Placed {
                ordinal,
                index: *index,
                record: Designation {
                    id: None,
                    term: term.to_owned(),
                    language: language.to_owned(),
                    use_ordinal,
                    active: true,
                },
            });
            *index += 1;
        };
    for (ordinal, concept, kind) in &numbered.concepts {
        builder.concept(*ordinal, concept)?;
        builder.properties(
            *ordinal,
            key(KIND_KEY)?,
            &[PropertyValue::Code((*kind).to_owned())],
        )?;
    }
    let mut answer_lists_of: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for list in lists.values() {
        for term_code in &list.terms {
            answer_lists_of
                .entry(term_code.as_str())
                .or_default()
                .push(list.code.clone());
        }
    }
    for term in &terms.rows {
        let Some(&ordinal) = numbered.ordinals.get(&term.code.to_ascii_uppercase()) else {
            continue;
        };
        let mut index = 0;
        place(ordinal, &mut index, &term.long_common_name, "en", 0);
        place(ordinal, &mut index, &term.short_name, "en", 1);
        place(ordinal, &mut index, &term.consumer_name, "en", 2);
        for variant in &variants {
            if let Some(translation) = variant.terms.get(&term.code) {
                if let Some(long) = &translation.long_common_name {
                    place(ordinal, &mut index, long, &variant.language, 0);
                }
                if let Some(short) = &translation.short_name {
                    place(ordinal, &mut index, short, &variant.language, 1);
                }
                if let Some(display) = &translation.display_name {
                    place(ordinal, &mut index, display, &variant.language, 4);
                }
            }
        }
        for (column, value) in &term.fields {
            builder.properties(
                ordinal,
                key(column)?,
                &[PropertyValue::String(value.clone())],
            )?;
        }
        let copyright = if term.external_copyright.is_some() {
            "3rdParty"
        } else {
            "LOINC"
        };
        builder.properties(
            ordinal,
            key(COPYRIGHT_KEY)?,
            &[PropertyValue::Code(copyright.to_owned())],
        )?;
        if let Some(lists) = answer_lists_of.get(term.code.as_str()) {
            let values: Vec<PropertyValue> = lists
                .iter()
                .map(|l| PropertyValue::Code(l.clone()))
                .collect();
            builder.properties(ordinal, key(ANSWER_LIST_KEY)?, &values)?;
        }
    }
    for part in &parts {
        let Some(&ordinal) = numbered.ordinals.get(&part.code.to_ascii_uppercase()) else {
            continue;
        };
        let mut index = 0;
        let name = if part.display_name.is_empty() {
            &part.name
        } else {
            &part.display_name
        };
        place(ordinal, &mut index, name, "en", 3);
        builder.properties(
            ordinal,
            key("STATUS")?,
            &[PropertyValue::String(part.status.clone())],
        )?;
        builder.properties(
            ordinal,
            key(COPYRIGHT_KEY)?,
            &[PropertyValue::Code(String::from("LOINC"))],
        )?;
    }
    for list in lists.values() {
        let Some(&ordinal) = numbered.ordinals.get(&list.code.to_ascii_uppercase()) else {
            continue;
        };
        let mut index = 0;
        place(ordinal, &mut index, &list.name, "en", 3);
        let values: Vec<PropertyValue> = list
            .answers
            .iter()
            .map(|a| PropertyValue::Code(a.code.clone()))
            .collect();
        builder.properties(ordinal, key(ANSWERS_KEY)?, &values)?;
        builder.properties(
            ordinal,
            key(COPYRIGHT_KEY)?,
            &[PropertyValue::Code(String::from("LOINC"))],
        )?;
        for answer in &list.answers {
            let Some(&answer_ordinal) = numbered.ordinals.get(&answer.code.to_ascii_uppercase())
            else {
                continue;
            };
            let mut index = 0;
            place(answer_ordinal, &mut index, &answer.display, "en", 3);
        }
    }
    for p in &placed {
        builder.designation(p.ordinal, p.index, &p.record)?;
    }
    let count = ordinal(numbered.concepts.len())?.index();
    let mut is_a = Vec::new();
    for edge in &edges {
        if let (Some(child), Some(parent)) = (
            numbered.ordinals.get(&edge.code.to_ascii_uppercase()),
            edge.parent
                .as_ref()
                .and_then(|p| numbered.ordinals.get(&p.to_ascii_uppercase())),
        ) {
            is_a.push((*child, *parent));
        }
    }
    let csr = Csr::build(count, is_a)?;
    let closure = Closure::compute(&csr)?;
    let hierarchy = Hierarchy { is_a: csr, closure };
    let hierarchy_path = out.join(HIERARCHY_FILE);
    let mut graph_bytes = Vec::new();
    hierarchy.write_to(&mut graph_bytes)?;
    std::fs::write(&hierarchy_path, &graph_bytes).map_err(io_error(&hierarchy_path))?;
    let mut index = IndexBuilder::new();
    for p in &placed {
        index.add(&Input {
            concept: p.ordinal,
            index: p.index,
            term: &p.record.term,
            // NOTE: the index keys designations by primary language subtag, the way
            // the providers query it; the store keeps the full BCP 47 tag.
            language: p
                .record
                .language
                .split('-')
                .next()
                .unwrap_or(&p.record.language),
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
    builder.finish(&PreferredRule { preferred: 0 })?;
    let mut languages: Vec<String> = variants.iter().map(|v| v.language.clone()).collect();
    languages.push(String::from("en"));
    languages.sort();
    languages.dedup();
    let manifest = json!({
        "manifest": MANIFEST_VERSION,
        "system": SYSTEM,
        "edition": SYSTEM,
        "version": version,
        "store": STORE_FILE,
        "storeLayout": tables::LAYOUT_VERSION,
        "hierarchy": HIERARCHY_FILE,
        "text": TEXT_FILE,
        "concepts": numbered.concepts.len(),
        "designations": placed.len(),
        "words": index.words(),
        "languages": languages,
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
        terms: u64::try_from(terms.rows.len()).unwrap_or(u64::MAX),
        parts: u64::try_from(parts.len()).unwrap_or(u64::MAX),
        answer_lists: u64::try_from(lists.len()).unwrap_or(u64::MAX),
        designations: u64::try_from(placed.len()).unwrap_or(u64::MAX),
        words: u64::try_from(index.words()).unwrap_or(u64::MAX),
    })
}
