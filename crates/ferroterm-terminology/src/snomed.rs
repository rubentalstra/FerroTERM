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
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ferroterm_graph::csr::{Csr, CsrError};
use ferroterm_graph::members::{MembersError, Memberships};
use ferroterm_graph::ordinal::Ordinal;
use ferroterm_graph::persist::Hierarchy as GraphHierarchy;
use ferroterm_rf2::constants;
use ferroterm_rf2::id::ConceptId;
use ferroterm_store::record;
use ferroterm_store::store::{Store, StoreError, Vocabulary};
use ferroterm_store::tables;
use ferroterm_text::index::{Query, TextIndex};
use serde::Deserialize;

use crate::compose::{Compose, ConceptRef, Include, SystemRef};
use crate::filter::{Filter, FilterOperator};
use crate::provider::{
    Capability, CodeSystemProvider, Concept, ConceptSet, ContentMode, Declaration, Designation,
    DesignationUse, FilterDefinition, Hierarchy, HierarchyMeaning, Identity, Located, Property,
    PropertyDefinition, PropertyKind, PropertyValue, ProviderError, Status,
};

/// The SNOMED CT system URI.
pub const SYSTEM: &str = "http://snomed.info/sct";
/// The manifest file inside an artifact directory.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The manifest version this provider reads: the store beside the hierarchy
/// and the designation index as their own files.
pub const MANIFEST_VERSION: u32 = 2;

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
    Hierarchy(#[from] ferroterm_graph::persist::PersistError),
    /// The designation index file does not read.
    #[error("cannot read the designation index file")]
    Text(#[from] ferroterm_text::persist::PersistError),
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
    /// The inactive concepts, read once on the first request that needs them.
    inactive: OnceLock<ConceptSet>,
    identity: Identity,
    declaration: Declaration,
    keys: Keys,
    edition: String,
    default_language: String,
    concepts: u32,
}

impl std::fmt::Debug for SnomedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnomedProvider")
            .field("version", &self.identity.version)
            .field("concepts", &self.concepts)
            .field("default_language", &self.default_language)
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
    /// tag used when a request names none.
    ///
    /// # Errors
    ///
    /// Returns [`OpenError`] when the manifest, the store, or a side file does not
    /// read, or the artifact is not a SNOMED CT edition.
    pub fn open(dir: &Path, default_language: &str) -> Result<Self, OpenError> {
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
        let store = Store::open(&dir.join(&manifest.store))?;
        let read = |name: &str| {
            let path = dir.join(name);
            std::fs::read(&path).map_err(|source| OpenError::Io { path, source })
        };
        let graph_bytes = read(&manifest.hierarchy)?;
        let graph = GraphHierarchy::read_from(&mut graph_bytes.as_slice())?;
        let children = graph.is_a.transpose()?;
        let text_bytes = read(&manifest.text)?;
        let text = ferroterm_text::persist::read_from(&mut text_bytes.as_slice())?;
        let memberships = match &manifest.refsets {
            Some(name) => Memberships::read_from(&mut read(name)?.as_slice())?,
            None => Memberships::new(),
        };
        let concepts = store
            .meta(tables::META_CONCEPTS)?
            .ok_or(OpenError::MissingMeta(tables::META_CONCEPTS))?;
        let concepts: u32 = concepts
            .parse()
            .map_err(|_| OpenError::ConceptCount(concepts.clone()))?;
        let keys = Self::resolve_keys(&store)?;
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
            inactive: OnceLock::new(),
            identity: Identity {
                url: SYSTEM.to_owned(),
                version: manifest.version,
                title: Some(String::from("SNOMED CT")),
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
                compositional: true,
                languages: manifest.languages,
                properties,
                // NOTE: the FHIR SNOMED CT page defines `concept is-a` and `concept in`
                // (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties").
                filters: vec![FilterDefinition {
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
                }],
                capabilities: BTreeSet::from([
                    Capability::Subsumption,
                    Capability::Enumeration,
                    Capability::ImplicitValueSets,
                ]),
            },
            keys,
            edition: manifest.edition,
            default_language: primary_subtag(default_language),
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

    /// The preferred synonym of `concept` in `language`, by the first language
    /// reference set (in store order) whose preferred synonym is in that
    /// language.
    fn preferred_in(
        &self,
        ordinal: Ordinal,
        language: &str,
    ) -> Result<Option<record::Designation>, ProviderError> {
        for (refset, _) in &self.keys.refsets {
            if let Some(designation) = self
                .store
                .preferred(ordinal, *refset, self.keys.synonym)
                .map_err(storage)?
                && primary_subtag(&designation.language) == language
            {
                return Ok(Some(designation));
            }
        }
        Ok(None)
    }

    /// The display for `language` (or the default), by the SNOMED rule: the
    /// preferred term of the language reference set; then, our own fallback
    /// order (no spec governs it): an active synonym in the language, the
    /// preferred term in the default language, the FSN, any designation.
    fn choose_display(
        &self,
        ordinal: Ordinal,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        let wanted = language.map_or_else(|| self.default_language.clone(), primary_subtag);
        if let Some(preferred) = self.preferred_in(ordinal, &wanted)? {
            return Ok(Some(preferred.term));
        }
        let designations = self.store.designations(ordinal).map_err(storage)?;
        if let Some(synonym) = designations.iter().find(|d| {
            d.active && d.use_ordinal == self.keys.synonym && primary_subtag(&d.language) == wanted
        }) {
            return Ok(Some(synonym.term.clone()));
        }
        if wanted != self.default_language
            && let Some(preferred) = self.preferred_in(ordinal, &self.default_language)?
        {
            return Ok(Some(preferred.term));
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
                    return Err(ProviderError::UnknownCode(refset));
                }
                Ok(Filter {
                    property: String::from("concept"),
                    op: FilterOperator::In,
                    value: refset,
                })
            }
            "ecl" => Err(malformed(String::from(
                "ECL expressions are not evaluated yet; `isa/`, `refset`, and `refset/` are",
            ))),
            _ => Err(malformed(format!("`{form}` is not a `fhir_vs` form"))),
        }
    }

    fn codes(
        &self,
        set: impl IntoIterator<Item = u32>,
    ) -> Result<Vec<PropertyValue>, ProviderError> {
        let mut out = Vec::new();
        for index in set {
            if let Some(concept) = self.store.concept(Ordinal::new(index)).map_err(storage)? {
                out.push(PropertyValue::Code(concept.code));
            }
        }
        Ok(out)
    }
}

impl CodeSystemProvider for SnomedProvider {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn declaration(&self) -> &Declaration {
        &self.declaration
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

    fn code(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(self
            .store
            .concept(Self::ordinal(concept))
            .map_err(storage)?
            .map(|c| c.code))
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
            active: record.is_some_and(|c| c.active),
            inactive_reason: None,
            abstract_concept: false,
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

    /// `concept in [sctid]` is reference set membership
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties"); every
    /// other filter is the generic evaluation over the closure and the store.
    fn filter(&self, filter: &Filter) -> Result<ConceptSet, ProviderError> {
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
            let refset =
                ConceptId::parse(value).map_err(|_| ProviderError::InvalidFilterValue {
                    property: filter.property.clone(),
                    value: value.to_owned(),
                    reason: String::from("not an SCTID"),
                })?;
            let members = self
                .memberships
                .members(refset.value())
                .ok_or_else(|| ProviderError::UnknownCode(value.to_owned()))?;
            set |= members;
        }
        Ok(set)
    }

    /// The implicit value sets of the FHIR SNOMED CT page
    /// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets"):
    /// `?fhir_vs`, `?fhir_vs=isa/[sctid]`, `?fhir_vs=refset`, and
    /// `?fhir_vs=refset/[sctid]`, on the bare system URI or on this edition's
    /// edition or version URI. `ecl/` waits for the evaluator.
    fn implicit_value_set(&self, url: &str) -> Option<Result<Compose, ProviderError>> {
        let (base, form) = implicit_parts(url)?;
        let malformed = |reason: String| ProviderError::MalformedImplicitValueSet {
            url: url.to_owned(),
            reason,
        };
        let version = if base == SYSTEM {
            None
        } else if base == self.edition || base == self.identity.version {
            Some(self.identity.version.clone())
        } else {
            return Some(Err(malformed(format!(
                "`{base}` is not the served edition `{}`",
                self.identity.version
            ))));
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
                let mut concepts = Vec::new();
                for refset in self.memberships.refsets() {
                    if let Ok(Some(located)) = self.locate(&refset.to_string()) {
                        concepts.push(ConceptRef {
                            code: located.code,
                            display: None,
                        });
                    }
                }
                if concepts.is_empty() {
                    return Some(Err(malformed(String::from(
                        "the edition has no reference sets with concept members",
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

    fn all(&self) -> Result<ConceptSet, ProviderError> {
        Ok((0..self.concepts).collect())
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

/// The base (the system, edition, or version URI) and the `fhir_vs` form of
/// an implicit value set URL, when `url` has the shape.
fn implicit_parts(url: &str) -> Option<(&str, &str)> {
    let (base, query) = url.split_once('?')?;
    let form = match query.strip_prefix("fhir_vs")? {
        "" => "",
        rest => rest.strip_prefix('=')?,
    };
    (base == SYSTEM || base.starts_with("http://snomed.info/sct/")).then_some((base, form))
}

#[cfg(test)]
mod tests {
    use super::{implicit_parts, primary_subtag};

    #[test]
    fn the_primary_subtag_is_the_language() {
        assert_eq!(primary_subtag("en-GB"), "en");
        assert_eq!(primary_subtag("nl"), "nl");
        assert_eq!(primary_subtag("EN_us"), "en");
    }

    #[test]
    fn implicit_urls_split_into_base_and_form() {
        assert_eq!(
            implicit_parts("http://snomed.info/sct?fhir_vs"),
            Some(("http://snomed.info/sct", ""))
        );
        assert_eq!(
            implicit_parts("http://snomed.info/sct/11000146104/version/20260630?fhir_vs=isa/1"),
            Some((
                "http://snomed.info/sct/11000146104/version/20260630",
                "isa/1"
            ))
        );
        assert_eq!(implicit_parts("http://snomed.info/sct?fhir_cm=1"), None);
        assert_eq!(implicit_parts("http://loinc.org/vs"), None);
        assert_eq!(implicit_parts("http://snomed.info/sct?fhir_vsx"), None);
    }
}
