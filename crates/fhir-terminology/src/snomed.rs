//! The SNOMED CT provider: one built edition version read from its artifact
//! directory (`store.redb`, `hierarchy.bin`, `text.bin`, and `manifest.json`).
//!
//! Identity is the SNOMED CT URI standard: the system `http://snomed.info/sct`
//! and the edition version URI as `version`
//! (<https://hl7.org/fhir/R4B/snomedct.html>). Display is the preferred term of
//! a language reference set for the requested language; the FHIR-defined
//! properties `inactive`, `sufficientlyDefined`, `moduleId`, `parent`, `child`,
//! and every concept-model attribute keyed by its concept id come from the
//! store. Reads are point reads and bitmap lookups; nothing walks the graph
//! per request.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

mod conceptmap;
mod ecl;

use concept_graph::attributes::{Attributes, AttributesError};
use concept_graph::csr::{Csr, CsrError};
use concept_graph::identifiers::{Identifiers, IdentifiersError};
use concept_graph::members::{MembersError, Memberships};
use concept_graph::ordinal::Ordinal;
use concept_graph::persist::Hierarchy as GraphHierarchy;
use concept_graph::refsets::{RefsetMembers, RefsetsError};
use concept_store::record;
use concept_store::store::{Store, StoreError, Vocabulary};
use concept_store::tables;
use designation_index::index::{Query, TextIndex};
use rf2::constants;
use rf2::id::ConceptId;
use roaring::RoaringBitmap;
use sct_ecl::ast::ExpressionConstraint;
use sct_ecl::eval::EvalError;
use serde::Deserialize;

use crate::compose::{Compose, ConceptRef, Include, SystemRef};
use crate::filter::{Filter, FilterOperator};
use crate::provider::{
    Capability, CodeSystemProvider, Compositional, Concept, ConceptSet, ContentMode, Declaration,
    Designation, DesignationUse, FilterDefinition, Hierarchy, HierarchyMeaning, Identity, Located,
    Property, PropertyDefinition, PropertyKind, PropertyValue, ProviderError, Status,
};

/// The SNOMED CT system URI.
pub const SYSTEM: &str = "http://snomed.info/sct";

/// The query key of an implicit value set URI
/// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets").
const FHIR_VS: &str = "fhir_vs";

/// The query key of an implicit concept map URI
/// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
const FHIR_CM: &str = "fhir_cm";

/// The characters SNOMED CT Compositional Grammar spends on the structure of
/// an expression: the definition status prefix (`===`, `<<<`), the `+` between
/// focus concepts, the `:` before a refinement, the `=` of an attribute, the
/// braces of an attribute group, the `,` between attributes, and the `|` of a
/// term (<http://snomed.org/scg>). A concept reference alone carries none of
/// them, so their presence is what separates an expression from an SCTID.
const SCG_OPERATORS: [char; 8] = ['+', ':', '=', '{', '}', ',', '|', '<'];
/// The manifest file inside an artifact directory.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The manifest version this provider reads: the store beside the hierarchy
/// and the designation index as their own files.
pub const MANIFEST_VERSION: u32 = 2;

/// The copyright notice every implicit value set template of the FHIR SNOMED
/// CT page carries, verbatim (<https://hl7.org/fhir/R4B/snomedct.html>,
/// "Implicit Value Sets").
pub const TEMPLATE_COPYRIGHT: &str = "This value set includes content from SNOMED CT, which is copyright \u{a9} 2002+ International Health Terminology Standards Development Organisation (SNOMED International), and distributed by agreement between SNOMED International and HL7. Implementer use of SNOMED CT is not covered by this agreement";

/// The SNOMED CT properties the FHIR SNOMED CT page defines that this server
/// does not generate (<https://hl7.org/fhir/R4B/snomedct.html>, "SNOMED CT
/// Properties").
///
/// Both are the Necessary Normal Form expression of a concept. The page
/// defines the property and not the generation, and the SNOMED CT normal form
/// is built from the proximal primitive supertypes
/// (<https://docs.snomed.org/>, the technical implementation guide's normal
/// form section), which the served index does not materialize. `$lookup`
/// refuses a `property` naming one rather than dropping it.
pub const UNSERVED_PROPERTIES: [&str; 2] = ["normalForm", "normalFormTerse"];

/// The FHIR-defined SNOMED properties this provider serves, in output order
/// (<https://hl7.org/fhir/R4B/snomedct.html>, the properties section).
pub const FHIR_PROPERTIES: [(&str, PropertyKind); 6] = [
    ("inactive", PropertyKind::Boolean),
    ("sufficientlyDefined", PropertyKind::Boolean),
    ("moduleId", PropertyKind::Code),
    ("effectiveTime", PropertyKind::String),
    ("parent", PropertyKind::Code),
    ("child", PropertyKind::Code),
];

/// A failure to open an artifact directory.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The manifest cannot be read.
    #[error("cannot read {path}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The manifest is not the JSON this provider reads.
    #[error("cannot parse {path}")]
    Manifest {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
    /// The manifest names another system.
    #[error("the artifact is for `{0}`, not SNOMED CT")]
    NotSnomed(String),
    /// The store cannot be opened or read.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The manifest is of another layout version.
    #[error("the manifest is version {0}; this server reads version {MANIFEST_VERSION}")]
    ManifestVersion(u32),
    /// The hierarchy file does not read.
    #[error("cannot read the hierarchy file")]
    Hierarchy(#[from] concept_graph::persist::PersistError),
    /// The designation index file does not read.
    #[error("cannot read the designation index file")]
    Text(#[from] designation_index::persist::PersistError),
    /// The child adjacency cannot be derived.
    #[error("cannot transpose the hierarchy")]
    Transpose(#[from] CsrError),
    /// A vocabulary entry the provider relies on is missing.
    #[error("the store's {vocabulary} vocabulary has no `{name}`")]
    MissingVocabulary {
        /// Which vocabulary.
        vocabulary: &'static str,
        /// The missing entry.
        name: String,
    },
    /// The reference set memberships file does not read.
    #[error("cannot read the reference set memberships")]
    Members(#[from] MembersError),
    /// The attribute relationships file does not read.
    #[error("cannot read the attribute relationships")]
    Attributes(#[from] AttributesError),
    /// The reference set member rows file does not read.
    #[error("cannot read the reference set member rows")]
    Refsets(#[from] RefsetsError),
    /// The alternate identifiers file does not read.
    #[error("cannot read the alternate identifiers")]
    Identifiers(#[from] IdentifiersError),
    /// The store's metadata is incomplete.
    #[error("the store's metadata has no `{0}`")]
    MissingMeta(&'static str),
    /// The concept count does not parse.
    #[error("the store's concept count `{0}` is not a number")]
    ConceptCount(String),
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "manifest")]
    layout: u32,
    system: String,
    edition: String,
    version: String,
    store: String,
    hierarchy: String,
    text: String,
    /// The reference set memberships; an artifact built before they were
    /// written has none.
    #[serde(default)]
    refsets: Option<String>,
    /// The attribute relationships, the reference set member rows, and the
    /// alternate identifiers the ECL evaluator reads; an artifact built before
    /// they were written has none.
    #[serde(default)]
    attributes: Option<String>,
    #[serde(default)]
    members: Option<String>,
    #[serde(default)]
    identifiers: Option<String>,
    #[serde(default)]
    languages: Vec<String>,
}

/// The vocabulary ordinals the provider resolves once at open.
#[derive(Debug, Clone)]
struct Keys {
    definition_status: u32,
    module: u32,
    /// Attribute type properties: (key ordinal, SCTID as text), sorted by ordinal.
    attributes: Vec<(u32, String)>,
    fsn: u32,
    synonym: u32,
    definition: u32,
    /// Language reference sets: (ordinal, SCTID as text), sorted by ordinal.
    refsets: Vec<(u32, String)>,
}

/// The hierarchy of the edition, in the seam's vocabulary.
#[derive(Debug)]
struct SnomedHierarchy {
    graph: GraphHierarchy,
    children: Csr,
}

impl Hierarchy for SnomedHierarchy {
    fn parents(&self, concept: Concept) -> ConceptSet {
        self.graph
            .is_a
            .neighbours(Ordinal::new(concept.index()))
            .iter()
            .copied()
            .collect()
    }

    fn any_parent_in(&self, concept: Concept, set: &ConceptSet) -> bool {
        self.graph
            .is_a
            .neighbours(Ordinal::new(concept.index()))
            .iter()
            .any(|parent| set.contains(*parent))
    }

    fn children(&self, concept: Concept) -> ConceptSet {
        self.children
            .neighbours(Ordinal::new(concept.index()))
            .iter()
            .copied()
            .collect()
    }

    fn ancestors(&self, concept: Concept) -> ConceptSet {
        self.graph
            .closure
            .ancestors(Ordinal::new(concept.index()))
            .clone()
    }

    fn descendants(&self, concept: Concept) -> ConceptSet {
        self.graph
            .closure
            .descendants(Ordinal::new(concept.index()))
            .clone()
    }
}

/// One SNOMED CT edition version behind the seam.
pub struct SnomedProvider {
    store: Store,
    hierarchy: SnomedHierarchy,
    text: TextIndex,
    memberships: Memberships,
    attributes: Attributes,
    member_tables: RefsetMembers,
    identifiers: Identifiers,
    roots: OnceLock<RoaringBitmap>,
    leaves: OnceLock<RoaringBitmap>,
    defined: OnceLock<RoaringBitmap>,
    /// Parsed expression constraints by their text, bounded.
    expressions: Mutex<HashMap<String, Arc<ExpressionConstraint>>>,
    /// The inactive concepts, read once on the first request that needs them.
    inactive: OnceLock<ConceptSet>,
    identity: Identity,
    declaration: Declaration,
    keys: Keys,
    edition: String,
    base_language: String,
    concepts: u32,
}

impl std::fmt::Debug for SnomedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnomedProvider")
            .field("version", &self.identity.version)
            .field("concepts", &self.concepts)
            .field("base_language", &self.base_language)
            .finish_non_exhaustive()
    }
}

fn storage(error: StoreError) -> ProviderError {
    ProviderError::Storage(Box::new(error))
}

/// The primary language subtag of a BCP 47 tag (`en-GB` is `en`), lowercased.
fn primary_subtag(language: &str) -> String {
    language
        .split(['-', '_'])
        .next()
        .unwrap_or(language)
        .to_ascii_lowercase()
}

impl SnomedProvider {
    /// Opens the artifact directory `dir`; `default_language` is the BCP 47
    /// tag used when a request names none, when the edition carries it.
    ///
    /// # Errors
    ///
    /// Returns [`OpenError`] when the manifest, the store, or a side file does not
    /// read, or the artifact is not a SNOMED CT edition.
    pub fn open(dir: &Path, default_language: &str) -> Result<Self, OpenError> {
        let manifest = Self::read_manifest(dir)?;
        let store = Store::open(&dir.join(&manifest.store))?;
        let read = |name: &str| {
            let path = dir.join(name);
            std::fs::read(&path).map_err(|source| OpenError::Io { path, source })
        };
        let graph_bytes = read(&manifest.hierarchy)?;
        let graph = GraphHierarchy::read_from(&mut graph_bytes.as_slice())?;
        let children = graph.is_a.transpose()?;
        let text_bytes = read(&manifest.text)?;
        let text = designation_index::persist::read_from(&mut text_bytes.as_slice())?;
        let memberships = match &manifest.refsets {
            Some(name) => Memberships::read_from(&mut read(name)?.as_slice())?,
            None => Memberships::new(),
        };
        let (attributes, member_tables, identifiers) = Self::read_ecl_files(dir, &manifest)?;
        let concepts = store
            .meta(tables::META_CONCEPTS)?
            .ok_or(OpenError::MissingMeta(tables::META_CONCEPTS))?;
        // The error carries the text; a `ParseIntError` adds nothing to it.
        let Ok(concepts) = concepts.parse::<u32>() else {
            return Err(OpenError::ConceptCount(concepts.clone()));
        };
        let keys = Self::resolve_keys(&store)?;
        let base_language = Self::base_language(default_language, &manifest.languages);
        let mut properties: Vec<PropertyDefinition> = FHIR_PROPERTIES
            .iter()
            .map(|(code, kind)| PropertyDefinition {
                code: (*code).to_owned(),
                uri: None,
                description: None,
                kind: *kind,
            })
            .collect();
        properties.extend(keys.attributes.iter().map(|(_, sctid)| PropertyDefinition {
            code: sctid.clone(),
            uri: Some(format!("http://snomed.info/id/{sctid}")),
            description: None,
            kind: PropertyKind::Code,
        }));
        Ok(Self {
            store,
            hierarchy: SnomedHierarchy { graph, children },
            text,
            memberships,
            attributes,
            member_tables,
            identifiers,
            roots: OnceLock::new(),
            leaves: OnceLock::new(),
            defined: OnceLock::new(),
            expressions: Mutex::new(HashMap::new()),
            inactive: OnceLock::new(),
            identity: Identity {
                url: SYSTEM.to_owned(),
                version: manifest.version,
                title: Some(String::from("SNOMED CT")),
                name: None,
                // NOTE: the canonical R4B CodeSystem for SNOMED CT declares
                // versionNeeded = false (<https://hl7.org/fhir/R4B/snomedct.html>).
                version_needed: false,
            },
            declaration: Declaration {
                content: ContentMode::NotPresent,
                // NOTE: caseSensitive = false per the canonical R4B CodeSystem;
                // SNOMED codes are digits, so the flag never changes a lookup.
                case_sensitive: false,
                hierarchy_meaning: Some(HierarchyMeaning::IsA),
                // NOTE: SNOMED CT defines Compositional Grammar
                // (<http://snomed.org/scg>) and this server evaluates no
                // expression, so the grammar is defined and not supported.
                compositional: Compositional::Defined,
                languages: manifest.languages,
                properties,
                // NOTE: the FHIR SNOMED CT page defines `concept is-a` and `concept in`
                // (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties").
                filters: Self::filter_definitions(),
                capabilities: BTreeSet::from([
                    Capability::Subsumption,
                    Capability::Enumeration,
                    Capability::ImplicitValueSets,
                    Capability::ImplicitConceptMaps,
                ]),
            },
            keys,
            edition: manifest.edition,
            base_language,
            concepts,
        })
    }

    /// The edition URI (`http://snomed.info/sct/{module}`), the version URI
    /// without its date.
    #[must_use]
    pub fn edition_uri(&self) -> &str {
        &self.edition
    }

    fn resolve_keys(store: &Store) -> Result<Keys, OpenError> {
        let key = |vocabulary: Vocabulary, what: &'static str, name: &str| {
            store.vocabulary_ordinal(vocabulary, name)?.ok_or_else(|| {
                OpenError::MissingVocabulary {
                    vocabulary: what,
                    name: name.to_owned(),
                }
            })
        };
        // The fixed keys the build writes before the attribute types; `parent`
        // is answered from the hierarchy, not the stored property.
        let fixed = ["parent", "definitionStatus", "module"];
        let definition_status = key(Vocabulary::PropertyKeys, "property key", "definitionStatus")?;
        let module = key(Vocabulary::PropertyKeys, "property key", "module")?;
        let mut attributes = Vec::new();
        let mut ordinal = 0_u32;
        while let Some(name) = store.vocabulary(Vocabulary::PropertyKeys, ordinal)? {
            if !fixed.contains(&name.as_str()) {
                attributes.push((ordinal, name));
            }
            ordinal = ordinal.saturating_add(1);
        }
        let fsn = key(
            Vocabulary::DesignationUses,
            "designation use",
            &constants::FULLY_SPECIFIED_NAME.to_string(),
        )?;
        let synonym = key(
            Vocabulary::DesignationUses,
            "designation use",
            &constants::SYNONYM.to_string(),
        )?;
        let definition = key(
            Vocabulary::DesignationUses,
            "designation use",
            &constants::DEFINITION.to_string(),
        )?;
        let mut refsets = Vec::new();
        let mut ordinal = 0_u32;
        while let Some(name) = store.vocabulary(Vocabulary::LanguageRefsets, ordinal)? {
            refsets.push((ordinal, name));
            ordinal = ordinal.saturating_add(1);
        }
        Ok(Keys {
            definition_status,
            module,
            attributes,
            fsn,
            synonym,
            definition,
            refsets,
        })
    }

    /// The filters of the FHIR SNOMED CT page
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties").
    fn filter_definitions() -> Vec<FilterDefinition> {
        vec![
            FilterDefinition {
                code: String::from("concept"),
                description: Some(String::from(
                    "`is-a`: the concept and its descendants; `descendent-of`: its descendants; `in`: the members of the reference set",
                )),
                operators: vec![
                    FilterOperator::IsA,
                    FilterOperator::DescendentOf,
                    FilterOperator::In,
                ],
                value: String::from("an SCTID"),
            },
            FilterDefinition {
                code: String::from("constraint"),
                description: Some(String::from(
                    "the concepts the SNOMED CT expression constraint selects",
                )),
                operators: vec![FilterOperator::Equal],
                value: String::from("an ECL expression"),
            },
            FilterDefinition {
                code: String::from("expressions"),
                description: Some(String::from(
                    "whether post-coordinated expressions are permitted; only `false` is served",
                )),
                operators: vec![FilterOperator::Equal],
                value: String::from("true or false"),
            },
        ]
    }

    /// The manifest of the artifact under `dir`, checked to be a SNOMED CT
    /// artifact of the layout this build reads.
    fn read_manifest(dir: &Path) -> Result<Manifest, OpenError> {
        let manifest_path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&manifest_path).map_err(|source| OpenError::Io {
            path: manifest_path.clone(),
            source,
        })?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|source| OpenError::Manifest {
                path: manifest_path,
                source,
            })?;
        if manifest.system != SYSTEM {
            return Err(OpenError::NotSnomed(manifest.system));
        }
        if manifest.layout != MANIFEST_VERSION {
            return Err(OpenError::ManifestVersion(manifest.layout));
        }
        Ok(manifest)
    }

    /// The files the ECL evaluator reads, when the artifact has them.
    fn read_ecl_files(
        dir: &Path,
        manifest: &Manifest,
    ) -> Result<(Attributes, RefsetMembers, Identifiers), OpenError> {
        let read = |name: &str| {
            let path = dir.join(name);
            std::fs::read(&path).map_err(|source| OpenError::Io { path, source })
        };
        let attributes = match &manifest.attributes {
            Some(name) => Attributes::read_from(&mut read(name)?.as_slice())?,
            None => Attributes::default(),
        };
        let member_tables = match &manifest.members {
            Some(name) => RefsetMembers::read_from(&mut read(name)?.as_slice())?,
            None => RefsetMembers::new(),
        };
        let identifiers = match &manifest.identifiers {
            Some(name) => Identifiers::read_from(&mut read(name)?.as_slice())?,
            None => Identifiers::default(),
        };
        Ok((attributes, member_tables, identifiers))
    }

    /// The attribute relationships of the edition, with their role groups.
    #[expect(
        clippy::same_name_method,
        reason = "the ECL evaluator's trait exposes the same graph part; this is the crate's own accessor"
    )]
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// The active reference set member rows of the edition, with their fields.
    #[must_use]
    pub fn member_tables(&self) -> &RefsetMembers {
        &self.member_tables
    }

    /// The alternate identifiers of the edition.
    #[expect(
        clippy::same_name_method,
        reason = "the ECL evaluator's trait exposes the same graph part; this is the crate's own accessor"
    )]
    #[must_use]
    pub fn identifiers(&self) -> &Identifiers {
        &self.identifiers
    }

    /// The language reference sets of the edition, as SCTIDs.
    #[must_use]
    pub fn language_refsets(&self) -> Vec<&str> {
        self.keys.refsets.iter().map(|(_, s)| s.as_str()).collect()
    }

    fn ordinal(concept: Concept) -> Ordinal {
        Ordinal::new(concept.index())
    }

    fn use_coding(&self, use_ordinal: u32) -> DesignationUse {
        let (code, display) = if use_ordinal == self.keys.fsn {
            (constants::FULLY_SPECIFIED_NAME, "Fully specified name")
        } else if use_ordinal == self.keys.definition {
            (constants::DEFINITION, "Definition")
        } else {
            (constants::SYNONYM, "Synonym")
        };
        DesignationUse {
            system: SYSTEM.to_owned(),
            code: code.to_string(),
            display: Some(display.to_owned()),
        }
    }

    /// The language this edition's own displays are in: the configured
    /// `default_language` when the edition carries descriptions in it, else
    /// the first language the edition declares.
    ///
    /// No FHIR/SNOMED spec governs this: our own design. An edition that
    /// carries no description in the configured tag would otherwise answer a
    /// display in a language it never states.
    fn base_language(default_language: &str, declared: &[String]) -> String {
        let wanted = primary_subtag(default_language);
        if declared.is_empty() || declared.iter().any(|l| primary_subtag(l) == wanted) {
            return wanted;
        }
        declared
            .first()
            .map_or(wanted, |first| primary_subtag(first))
    }

    /// The preferred synonym of `concept` in `language`, by the first language
    /// reference set (in store order) whose preferred synonym is in that
    /// language.
    fn preferred_in(
        &self,
        ordinal: Ordinal,
        language: &str,
    ) -> Result<Option<String>, ProviderError> {
        let refsets = self.keys.refsets.iter().map(|(refset, _)| *refset);
        self.store
            .display(ordinal, refsets, |found| primary_subtag(found) == language)
            .map_err(storage)
    }

    /// The display for `language` (or the base language), by the SNOMED rule:
    /// the preferred term of the language reference set; then, our own fallback
    /// order (no spec governs it): an active synonym in the language, the
    /// preferred term in the base language, the FSN, any designation.
    fn choose_display(
        &self,
        ordinal: Ordinal,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        let wanted = language.map_or_else(|| self.base_language.clone(), primary_subtag);
        if let Some(term) = self.preferred_in(ordinal, &wanted)? {
            return Ok(Some(term));
        }
        let designations = self.store.designations(ordinal).map_err(storage)?;
        if let Some(synonym) = designations.iter().find(|d| {
            d.active && d.use_ordinal == self.keys.synonym && primary_subtag(&d.language) == wanted
        }) {
            return Ok(Some(synonym.term.clone()));
        }
        if wanted != self.base_language
            && let Some(term) = self.preferred_in(ordinal, &self.base_language)?
        {
            return Ok(Some(term));
        }
        if let Some(fsn) = designations
            .iter()
            .find(|d| d.active && d.use_ordinal == self.keys.fsn)
        {
            return Ok(Some(fsn.term.clone()));
        }
        Ok(designations.first().map(|d| d.term.clone()))
    }

    /// The concept named by `text`, as the store spells it.
    fn sctid_of(&self, url: &str, text: &str) -> Result<String, ProviderError> {
        match self.locate(text)? {
            Some(located) => Ok(located.code),
            None => Err(match ConceptId::parse(text) {
                Ok(_) => ProviderError::UnknownCode(text.to_owned()),
                Err(_) => ProviderError::MalformedImplicitValueSet {
                    url: url.to_owned(),
                    reason: format!("`{text}` is not an SCTID"),
                },
            }),
        }
    }

    /// The preferred term of `code` in the server's default language, else
    /// `code` itself: the templates' "[sctid or preferred description]"
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets").
    fn term_or_code(&self, code: &str) -> String {
        self.locate(code)
            .ok()
            .flatten()
            .and_then(|located| {
                self.choose_display(Self::ordinal(located.concept), None)
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| code.to_owned())
    }

    /// The `version` an implicit URI on `base` resolves to: `None` for the
    /// bare system URI, this edition's version URI for its own edition or
    /// version base.
    ///
    /// A base naming another edition is
    /// [`ProviderError::UnservedImplicitVersion`], never a malformed URI: the
    /// FHIR SNOMED CT page admits any edition version as the base
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets"), so
    /// the caller asks the other loaded editions before the server answers
    /// that it holds no such version.
    pub(crate) fn implicit_version(&self, base: &str) -> Result<Option<String>, ProviderError> {
        if base == SYSTEM {
            Ok(None)
        } else if base == self.edition || base == self.identity.version {
            Ok(Some(self.identity.version.clone()))
        } else {
            Err(ProviderError::UnservedImplicitVersion {
                url: SYSTEM.to_owned(),
                version: base.to_owned(),
            })
        }
    }

    /// The filter behind an `isa/[sctid]` or `refset/[sctid]` form of `url`.
    fn implicit_filter(&self, url: &str, form: &str) -> Result<Filter, ProviderError> {
        let malformed = |reason: String| ProviderError::MalformedImplicitValueSet {
            url: url.to_owned(),
            reason,
        };
        let (kind, argument) = form.split_once('/').unwrap_or((form, ""));
        match kind {
            "isa" => Ok(Filter {
                property: String::from("concept"),
                op: FilterOperator::IsA,
                value: self.sctid_of(url, argument)?,
            }),
            "refset" => {
                let refset = self.sctid_of(url, argument)?;
                if ConceptId::parse(&refset)
                    .ok()
                    .and_then(|id| self.memberships.members(id.value()))
                    .is_none()
                {
                    // NOTE: a language reference set references descriptions, so
                    // "all concept ids in the specified reference set" selects none
                    // (<https://hl7.org/fhir/R4B/snomedct.html>).
                    if self.keys.refsets.iter().any(|(_, id)| *id == refset) {
                        return Err(ProviderError::InvalidFilterValue {
                            property: String::from("concept"),
                            value: refset,
                            reason: String::from(
                                "a language reference set references descriptions, not concepts",
                            ),
                        });
                    }
                    return Err(ProviderError::UnknownCode(refset));
                }
                Ok(Filter {
                    property: String::from("concept"),
                    op: FilterOperator::In,
                    value: refset,
                })
            }
            "ecl" => {
                let text = percent_decode(argument).ok_or_else(|| {
                    malformed(String::from("the expression constraint is not URI-encoded"))
                })?;
                self.expression(&text)
                    .map_err(|error| malformed(error.to_string()))?;
                Ok(Filter {
                    property: String::from("constraint"),
                    op: FilterOperator::Equal,
                    value: text,
                })
            }
            _ => Err(malformed(format!("`{form}` is not a `fhir_vs` form"))),
        }
    }

    /// The parsed form of an expression constraint, from the cache when it
    /// was parsed before (the cache is cleared when it holds 256 entries).
    fn expression(&self, text: &str) -> Result<Arc<ExpressionConstraint>, sct_ecl::ParseError> {
        if let Ok(cache) = self.expressions.lock()
            && let Some(parsed) = cache.get(text)
        {
            return Ok(Arc::clone(parsed));
        }
        let parsed = Arc::new(sct_ecl::parse(text)?);
        if let Ok(mut cache) = self.expressions.lock() {
            if cache.len() >= 256 {
                cache.clear();
            }
            cache.insert(text.to_owned(), Arc::clone(&parsed));
        }
        Ok(parsed)
    }

    /// The concepts an expression constraint selects; malformed text is an
    /// invalid filter value with the parser's position, an identifier the
    /// edition lacks an invalid code.
    fn constraint(&self, text: &str) -> Result<ConceptSet, ProviderError> {
        let invalid = |reason: String| ProviderError::InvalidFilterValue {
            property: String::from("constraint"),
            value: text.to_owned(),
            reason,
        };
        let parsed = self
            .expression(text)
            .map_err(|error| invalid(error.to_string()))?;
        sct_ecl::eval::evaluate(self, &parsed).map_err(|error| match error {
            EvalError::UnknownConcept(id) => ProviderError::InvalidCode {
                code: id.to_string(),
                reason: String::from("not a concept of the edition"),
            },
            EvalError::NotAReferenceSet(id) => ProviderError::InvalidCode {
                code: id.to_string(),
                reason: String::from("not a reference set with concept members in the edition"),
            },
            EvalError::Unsupported(what) => ProviderError::UnsupportedFilter {
                property: String::from("constraint"),
                operator: format!("= ({what})"),
            },
            EvalError::Storage(message) => ProviderError::Storage(message.into()),
            other => invalid(other.to_string()),
        })
    }

    /// The codes of `set`, sliced out of the concept column: a concept with
    /// hundreds of children costs one string per child and no search (#314).
    fn codes(
        &self,
        set: impl IntoIterator<Item = u32>,
    ) -> Result<Vec<PropertyValue>, ProviderError> {
        let ordinals = set.into_iter().map(Ordinal::new);
        Ok(self
            .store
            .codes(ordinals)
            .map_err(storage)?
            .into_iter()
            .flatten()
            .map(PropertyValue::Code)
            .collect())
    }
}

impl CodeSystemProvider for SnomedProvider {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    // NOTE: the edition URI names a distribution, and the service "may default to
    // the most recent version of the named SNOMED CT distribution"
    // (<https://hl7.org/fhir/R4B/snomedct.html>, "Versions").
    fn answers_version(&self, version: &str) -> bool {
        version == self.edition
    }

    fn unserved_properties(&self) -> &[&'static str] {
        &UNSERVED_PROPERTIES
    }

    /// The language of the displays this provider returns, so a
    /// `displayLanguage` the edition carries no description for is answered as
    /// a language the system has no display in
    /// (`OperationDefinition/CodeSystem-validate-code`: `displayLanguage`
    /// "Specifies the language to be used for description when validating the
    /// display property").
    fn language(&self) -> Option<&str> {
        Some(&self.base_language)
    }

    fn locate(&self, code: &str) -> Result<Option<Located>, ProviderError> {
        // NOTE: a string that is not a well-formed SCTID (check digit,
        // partition) is not a code of this system; that is "absent", not an
        // error (<https://hl7.org/fhir/R4B/snomedct.html>, valid code values).
        if ConceptId::parse(code).is_err() {
            return Ok(None);
        }
        Ok(self
            .store
            .ordinal(code)
            .map_err(storage)?
            .map(|ordinal| Located {
                concept: Concept::new(ordinal.index()),
                code: code.to_owned(),
            }))
    }

    // NOTE: SNOMED CT Expressions in Compositional Grammar are valid codes
    // (<https://hl7.org/fhir/R4B/snomedct.html>, "Code"), so an expression is
    // refused for the grammar, not as a concept the edition lacks.
    fn is_expression(&self, code: &str) -> bool {
        code.contains(SCG_OPERATORS)
    }

    fn code(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(self
            .store
            .code(Self::ordinal(concept))
            .map_err(storage)?
            .map(ToOwned::to_owned))
    }

    fn display(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        self.choose_display(Self::ordinal(concept), language)
    }

    fn definition(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(self
            .store
            .designations(Self::ordinal(concept))
            .map_err(storage)?
            .into_iter()
            .find(|d| d.active && d.use_ordinal == self.keys.definition)
            .map(|d| d.term))
    }

    fn status(&self, concept: Concept) -> Result<Status, ProviderError> {
        let record = self
            .store
            .concept(Self::ordinal(concept))
            .map_err(storage)?;
        Ok(Status {
            standards_status: None,
            active: record.is_some_and(|c| c.active),
            inactive_reason: None,
            abstract_concept: false,
            codeless: false,
        })
    }

    fn designations(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Vec<Designation>, ProviderError> {
        let wanted = language.map(primary_subtag);
        Ok(self
            .store
            .designations(Self::ordinal(concept))
            .map_err(storage)?
            .into_iter()
            .filter(|d| {
                wanted
                    .as_deref()
                    .is_none_or(|wanted| primary_subtag(&d.language) == wanted)
            })
            .map(|d| Designation {
                standards_status: None,
                language: Some(d.language.clone()),
                use_: Some(self.use_coding(d.use_ordinal)),
                value: d.term,
            })
            .collect())
    }

    fn properties(&self, concept: Concept) -> Result<Vec<Property>, ProviderError> {
        let ordinal = Self::ordinal(concept);
        let Some(record) = self.store.concept(ordinal).map_err(storage)? else {
            return Ok(Vec::new());
        };
        let stored = self.store.properties(ordinal).map_err(storage)?;
        let of = |key: u32| stored.iter().find(|(k, _)| *k == key).map(|(_, v)| v);
        let mut out = vec![Property {
            code: String::from("inactive"),
            value: PropertyValue::Boolean(!record.active),
            ..Property::default()
        }];
        let defined = of(self.keys.definition_status).is_some_and(|values| {
            values
                .iter()
                .any(|v| matches!(v, record::PropertyValue::Code(c) if *c == constants::DEFINED.to_string()))
        });
        out.push(Property {
            code: String::from("sufficientlyDefined"),
            value: PropertyValue::Boolean(defined),
            ..Property::default()
        });
        if let Some(values) = of(self.keys.module)
            && let Some(record::PropertyValue::Code(module)) = values.first()
        {
            out.push(Property {
                code: String::from("moduleId"),
                value: PropertyValue::Code(module.clone()),
                ..Property::default()
            });
        }
        if let Some(time) = record.effective_time {
            out.push(Property {
                code: String::from("effectiveTime"),
                value: PropertyValue::String(time),
                ..Property::default()
            });
        }
        for value in self.codes(self.hierarchy.parents(concept))? {
            out.push(Property {
                code: String::from("parent"),
                value,
                ..Property::default()
            });
        }
        for value in self.codes(self.hierarchy.children(concept))? {
            out.push(Property {
                code: String::from("child"),
                value,
                ..Property::default()
            });
        }
        for (key, sctid) in &self.keys.attributes {
            let Some(values) = of(*key) else {
                continue;
            };
            for value in values {
                let value = match value {
                    record::PropertyValue::Concept(target) => {
                        match self.store.concept(*target).map_err(storage)? {
                            Some(target) => PropertyValue::Code(target.code),
                            None => continue,
                        }
                    }
                    record::PropertyValue::Code(c) => PropertyValue::Code(c.clone()),
                    record::PropertyValue::String(s) => PropertyValue::String(s.clone()),
                    record::PropertyValue::Integer(i) => PropertyValue::Integer(*i),
                    record::PropertyValue::Boolean(b) => PropertyValue::Boolean(*b),
                    record::PropertyValue::Decimal(d) => PropertyValue::Decimal(d.clone()),
                    record::PropertyValue::DateTime(d) => PropertyValue::DateTime(d.clone()),
                };
                out.push(Property {
                    code: sctid.clone(),
                    value,
                    ..Property::default()
                });
            }
        }
        Ok(out)
    }

    fn hierarchy(&self) -> Option<&dyn Hierarchy> {
        Some(&self.hierarchy)
    }

    /// The inactive concepts, scanned once from the store and kept.
    fn inactive(&self) -> Result<ConceptSet, ProviderError> {
        if let Some(set) = self.inactive.get() {
            return Ok(set.clone());
        }
        let mut set = ConceptSet::new();
        for index in 0..self.concepts {
            if let Some(record) = self.store.concept(Ordinal::new(index)).map_err(storage)?
                && !record.active
            {
                set.insert(index);
            }
        }
        Ok(self.inactive.get_or_init(|| set).clone())
    }

    /// `concept in [sctid]` is reference set membership, `constraint = [ecl]`
    /// the evaluated expression constraint, and `expressions = false` every
    /// concept (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties");
    /// every other filter is the generic evaluation over the closure and the
    /// store. Post-coordination (`expressions = true`) is not served.
    fn filter(&self, filter: &Filter) -> Result<ConceptSet, ProviderError> {
        if filter.op == FilterOperator::Equal {
            match filter.property.as_str() {
                "constraint" => return self.constraint(&filter.value),
                "expressions" => {
                    return match filter.value.trim() {
                        "false" => self.all(),
                        "true" => Err(ProviderError::UnsupportedFilter {
                            property: filter.property.clone(),
                            operator: String::from("= true"),
                        }),
                        other => Err(ProviderError::InvalidFilterValue {
                            property: filter.property.clone(),
                            value: other.to_owned(),
                            reason: String::from("expected `true` or `false`"),
                        }),
                    };
                }
                _ => {}
            }
        }
        if filter.property != "concept" || filter.op != FilterOperator::In {
            return crate::filter::evaluate(self, filter);
        }
        let mut set = ConceptSet::new();
        for value in filter
            .value
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            // The error names the value and the reason; the id error adds nothing.
            let Ok(refset) = ConceptId::parse(value) else {
                return Err(ProviderError::InvalidFilterValue {
                    property: filter.property.clone(),
                    value: value.to_owned(),
                    reason: String::from("not an SCTID"),
                });
            };
            let members = self
                .memberships
                .members(refset.value())
                .ok_or_else(|| ProviderError::UnknownCode(value.to_owned()))?;
            set |= members;
        }
        Ok(set)
    }

    /// A SNOMED CT selection admits the post-coordinated expressions its
    /// grammar builds, and no expansion lists them, which is the case `$expand`
    /// names for `valueset-unclosed`: "unbounded due to the inclusion of
    /// post-coordinated value sets (e.g. SNOMED CT, UCUM)"
    /// (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>, Notes). The
    /// `expressions = false` filter is what keeps them out
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties"), so a
    /// selection stating it is complete.
    fn unclosed(&self, filters: &[Filter]) -> bool {
        !filters.iter().any(|filter| {
            filter.property == "expressions"
                && filter.op == FilterOperator::Equal
                && filter.value.trim() == "false"
        })
    }

    /// The implicit value sets of the FHIR SNOMED CT page
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets"):
    /// `?fhir_vs`, `?fhir_vs=isa/[sctid]`, `?fhir_vs=refset`, and
    /// `?fhir_vs=refset/[sctid]`, on the bare system URI or on this edition's
    /// edition or version URI. `ecl/` waits for the evaluator.
    fn successors(
        &self,
        concept: Concept,
    ) -> Result<Vec<crate::provider::Successor>, ProviderError> {
        conceptmap::successors(self, concept)
    }

    /// The implicit concept maps of the FHIR SNOMED CT page
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps"):
    /// `?fhir_cm=[sctid]` names a map reference set of this edition, on the
    /// bare system URI or on this edition's edition or version URI.
    fn implicit_concept_map(
        &self,
        url: &str,
        selection: crate::provider::MapSelection<'_>,
    ) -> Option<Result<crate::conceptmap::model::ConceptMapModel, ProviderError>> {
        let (base, form) = implicit_parts(url, FHIR_CM)?;
        let malformed = |reason: String| ProviderError::MalformedImplicitConceptMap {
            url: url.to_owned(),
            reason,
        };
        // NOTE: the base must be this edition or the bare system URI; the map itself
        // always states the served version
        // (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
        if let Err(error) = self.implicit_version(base) {
            return Some(Err(error));
        }
        let Some(decoded) = percent_decode(form) else {
            return Some(Err(malformed(format!(
                "`{form}` is not a percent-encoded SCTID"
            ))));
        };
        let Ok(refset) = ConceptId::parse(decoded.trim()) else {
            return Some(Err(malformed(format!("`{decoded}` is not an SCTID"))));
        };
        Some(conceptmap::concept_map(
            self,
            url,
            refset.value(),
            selection,
        ))
    }

    fn implicit_value_set(&self, url: &str) -> Option<Result<Compose, ProviderError>> {
        let (base, form) = implicit_parts(url, FHIR_VS)?;
        let malformed = |reason: String| ProviderError::MalformedImplicitValueSet {
            url: url.to_owned(),
            reason,
        };
        let version = match self.implicit_version(base) {
            Ok(version) => version,
            Err(error) => return Some(Err(error)),
        };
        let system = SystemRef {
            url: SYSTEM.to_owned(),
            version,
        };
        let include = match form {
            "" => Include {
                system: Some(system),
                ..Include::default()
            },
            "refset" => {
                // NOTE: the set is "all concept ids that correspond to reference sets
                // that are explicitly defined in the specified SNOMED CT edition",
                // with no category excluded (<https://hl7.org/fhir/R4B/snomedct.html>).
                let mut defined: BTreeSet<u64> = self.memberships.refsets().collect();
                defined.extend(
                    self.keys
                        .refsets
                        .iter()
                        .filter_map(|(_, id)| ConceptId::parse(id).ok())
                        .map(ConceptId::value),
                );
                let mut concepts = Vec::new();
                for refset in defined {
                    if let Ok(Some(located)) = self.locate(&refset.to_string()) {
                        concepts.push(ConceptRef {
                            deprecated: false,
                            code: located.code,
                            display: None,
                        });
                    }
                }
                if concepts.is_empty() {
                    return Some(Err(malformed(String::from(
                        "the edition defines no reference sets",
                    ))));
                }
                Include {
                    system: Some(system),
                    concepts,
                    ..Include::default()
                }
            }
            other => {
                let filter = match self.implicit_filter(url, other) {
                    Ok(filter) => filter,
                    Err(error) => return Some(Err(error)),
                };
                Include {
                    system: Some(system),
                    filters: vec![filter],
                    ..Include::default()
                }
            }
        };
        Some(Ok(Compose {
            include: vec![include],
            ..Compose::default()
        }))
    }

    /// The template fields of an implicit value set of the FHIR SNOMED CT page
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets").
    ///
    /// The page prints a template per form and says "the content of the
    /// resource must conform to the template provided". It prints none for the
    /// bare `?fhir_vs`, so that form carries only the fields every template
    /// shares: the edition version and the copyright.
    fn implicit_metadata(&self, url: &str) -> crate::provider::ImplicitMetadata {
        let Some((_, form)) = implicit_parts(url, FHIR_VS) else {
            return crate::provider::ImplicitMetadata::default();
        };
        let (name, description) = match form.split_once('/').unwrap_or((form, "")) {
            ("isa", sctid) => (
                Some(format!("SNOMED CT Concept {sctid} and descendants")),
                Some(format!(
                    "All SNOMED CT concepts for {}",
                    self.term_or_code(sctid)
                )),
            ),
            ("refset", "") => (
                Some(String::from("SNOMED CT Reference Sets")),
                Some(String::from(
                    "All SNOMED CT concepts associated with a reference set",
                )),
            ),
            ("refset", sctid) => (
                Some(format!("SNOMED CT Reference Set {sctid}")),
                Some(format!(
                    "All SNOMED CT concepts in the reference set {}",
                    self.term_or_code(sctid)
                )),
            ),
            ("ecl", ecl) => {
                let ecl = percent_decode(ecl).unwrap_or_else(|| ecl.to_owned());
                (
                    Some(format!("SNOMED CT Concepts matching {ecl}")),
                    Some(format!(
                        "All SNOMED CT concepts that match the expression constraint {ecl}"
                    )),
                )
            }
            _ => (None, None),
        };
        crate::provider::ImplicitMetadata {
            version: Some(self.identity.version.clone()),
            name,
            title: None,
            experimental: None,
            date: None,
            description,
            copyright: Some(String::from(TEMPLATE_COPYRIGHT)),
        }
    }

    fn all(&self) -> Result<ConceptSet, ProviderError> {
        Ok(crate::provider::every(self.concepts))
    }

    fn search(&self, text: &str, language: Option<&str>) -> Result<ConceptSet, ProviderError> {
        let query = Query {
            text: text.to_owned(),
            language: language.map(primary_subtag),
            ..Query::default()
        };
        let mut concepts = ConceptSet::new();
        for designation in self.text.matches(&query) {
            if let Some(entry) = self.text.entry(designation) {
                concepts.insert(entry.concept.index());
            }
        }
        Ok(concepts)
    }
}

/// Decodes the percent-encoding of a URI component
/// (<https://www.rfc-editor.org/rfc/rfc3986#section-2.1>); `None` for a
/// stray `%` or bytes that are not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        if byte == b'%' {
            let hex = bytes.get(i.checked_add(1)?..i.checked_add(3)?)?;
            let text = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(text, 16).ok()?);
            i = i.checked_add(3)?;
        } else {
            out.push(byte);
            i = i.checked_add(1)?;
        }
    }
    String::from_utf8(out).ok()
}

/// The base (the system, edition, or version URI) and the form of an implicit
/// URI whose query key is `key` (`fhir_vs` or `fhir_cm`), when `url` has the
/// shape (<https://hl7.org/fhir/R4B/snomedct.html>).
fn implicit_parts<'a>(url: &'a str, key: &str) -> Option<(&'a str, &'a str)> {
    let (base, query) = url.split_once('?')?;
    let form = match query.strip_prefix(key)? {
        "" => "",
        rest => rest.strip_prefix('=')?,
    };
    (base == SYSTEM || base.starts_with("http://snomed.info/sct/")).then_some((base, form))
}

#[cfg(test)]
mod tests {
    use super::{FHIR_CM, FHIR_VS, implicit_parts, percent_decode, primary_subtag};

    #[test]
    fn percent_encoding_decodes_and_a_stray_percent_is_refused() {
        assert_eq!(
            percent_decode("%3C%3C%20404684003%20%7Cfinding%7C").as_deref(),
            Some("<< 404684003 |finding|")
        );
        assert_eq!(percent_decode("<< 1").as_deref(), Some("<< 1"));
        assert_eq!(percent_decode("%3"), None);
        assert_eq!(percent_decode("%zz"), None);
    }

    #[test]
    fn the_primary_subtag_is_the_language() {
        assert_eq!(primary_subtag("en-GB"), "en");
        assert_eq!(primary_subtag("nl"), "nl");
        assert_eq!(primary_subtag("EN_us"), "en");
    }

    #[test]
    fn implicit_urls_split_into_base_and_form() {
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_vs", FHIR_VS),
            Some(("http://snomed.info/sct", ""))
        );
        assert_eq!(
            implicit_parts(
                "http://snomed.info/sct/11000146104/version/20260630?fhir_vs=isa/1",
                FHIR_VS
            ),
            Some((
                "http://snomed.info/sct/11000146104/version/20260630",
                "isa/1"
            ))
        );
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_cm=1", FHIR_CM),
            Some(("http://snomed.info/sct", "1"))
        );
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_cm=1", FHIR_VS),
            None,
            "one key never reads the other's URI"
        );
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_vs", FHIR_CM),
            None
        );
        assert_eq!(implicit_parts("http://loinc.org/vs", FHIR_VS), None);
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_vsx", FHIR_VS),
            None
        );
    }
}
