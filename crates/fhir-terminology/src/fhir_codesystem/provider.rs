//! Serving a [`CodeSystemModel`] through the seam.

use std::collections::{BTreeMap, BTreeSet};

use concept_graph::closure::Closure;
use concept_graph::csr::Csr;
use concept_graph::ordinal::Ordinal;
use roaring::RoaringBitmap;

use super::model::{
    CHILD, CodeSystemModel, ConceptEntry, DEPRECATED, INACTIVE, NOT_SELECTABLE, PARENT, STATUS,
};
use crate::provider::{
    Capability, CodeSystemProvider, Concept, ConceptSet, ContentMode, Declaration, Designation,
    Hierarchy, HierarchyMeaning, Identity, Located, Property, PropertyDefinition, PropertyKind,
    PropertyValue, ProviderError, Status,
};
use crate::text_match;

/// A failure to build the provider from a model.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The concept hierarchy has a cycle.
    #[error("the hierarchy of `{url}` is not acyclic")]
    Cycle {
        /// The system.
        url: String,
    },
    /// Too many concepts for a `u32` ordinal.
    #[error("`{url}` has more concepts than the provider can number")]
    TooMany {
        /// The system.
        url: String,
    },
}

/// The hierarchy of a system whose `hierarchyMeaning` is `is-a`.
#[derive(Debug)]
struct ModelHierarchy {
    parents: Csr,
    children: Csr,
    closure: Closure,
}

impl Hierarchy for ModelHierarchy {
    fn parents(&self, concept: Concept) -> ConceptSet {
        self.parents
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
        self.closure
            .ancestors(Ordinal::new(concept.index()))
            .clone()
    }

    fn descendants(&self, concept: Concept) -> ConceptSet {
        self.closure
            .descendants(Ordinal::new(concept.index()))
            .clone()
    }
}

/// A code system served from a `CodeSystem` resource.
#[derive(Debug)]
pub struct FhirCodeSystem {
    identity: Identity,
    declaration: Declaration,
    model: CodeSystemModel,
    /// The comparison key of each code (lowercased when case-insensitive) to its ordinal.
    codes: BTreeMap<String, u32>,
    hierarchy: Option<ModelHierarchy>,
    all: RoaringBitmap,
}

impl FhirCodeSystem {
    /// Builds the provider; the hierarchy exists when `hierarchyMeaning` is
    /// `is-a` and at least one concept names a parent.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] for a cyclic hierarchy or too many concepts.
    pub fn new(mut model: CodeSystemModel) -> Result<Self, BuildError> {
        // NOTE: no FHIR spec fixes an expansion order; concepts are numbered in code
        // order so paging over ordinals is paging over codes, as for every built system.
        model.concepts.sort_by(|a, b| a.code.cmp(&b.code));
        let count = u32::try_from(model.concepts.len()).map_err(|_| BuildError::TooMany {
            url: model.url.clone(),
        })?;
        let key = |code: &str| {
            if model.case_sensitive {
                code.to_owned()
            } else {
                code.to_lowercase()
            }
        };
        let mut codes = BTreeMap::new();
        for (i, concept) in model.concepts.iter().enumerate() {
            codes.insert(key(&concept.code), u32::try_from(i).unwrap_or(u32::MAX));
        }
        let hierarchy = build_hierarchy(&model, count)?;
        let languages = languages_of(&model);
        let properties = with_standard_properties(&model.properties);
        let mut capabilities = BTreeSet::new();
        if hierarchy.is_some() {
            capabilities.insert(Capability::Subsumption);
        }
        if matches!(model.content, ContentMode::Complete | ContentMode::Fragment) {
            capabilities.insert(Capability::Enumeration);
        }
        let identity = Identity {
            url: model.url.clone(),
            version: model.version.clone(),
            name: model.name.clone(),
            title: model.title.clone(),
            version_needed: model.version_needed,
        };
        let declaration = Declaration {
            content: model.content,
            case_sensitive: model.case_sensitive,
            hierarchy_meaning: model.hierarchy_meaning,
            compositional: model.compositional,
            languages,
            properties,
            filters: model.filters.clone(),
            capabilities,
        };
        let mut all = RoaringBitmap::new();
        all.insert_range(0..count);
        Ok(Self {
            identity,
            declaration,
            model,
            codes,
            hierarchy,
            all,
        })
    }

    /// The model this provider serves.
    #[must_use]
    pub fn model(&self) -> &CodeSystemModel {
        &self.model
    }

    fn entry(&self, concept: Concept) -> Option<&ConceptEntry> {
        self.model
            .concepts
            .get(usize::try_from(concept.index()).ok()?)
    }

    /// Refuses a read on a system whose content the resource does not carry.
    ///
    /// `not-present` defines no codes at all and `example` a handful with "no
    /// useful intent" (<https://hl7.org/fhir/R4B/codesystem-content-mode.html>),
    /// so neither can say whether a code is valid.
    fn complete_enough(&self) -> Result<(), ProviderError> {
        match self.model.content {
            ContentMode::NotPresent | ContentMode::Example => {
                Err(ProviderError::IncompleteContent {
                    system: self.model.url.clone(),
                    content: self.model.content.code(),
                })
            }
            ContentMode::Fragment | ContentMode::Complete | ContentMode::Supplement => Ok(()),
        }
    }

    fn status_of(entry: &ConceptEntry) -> Status {
        let mut active = true;
        let mut abstract_concept = false;
        let mut reason = None;
        for property in &entry.properties {
            match (property.code.as_str(), &property.value) {
                (INACTIVE, PropertyValue::Boolean(true)) => {
                    active = false;
                    reason.get_or_insert_with(|| String::from("inactive"));
                }
                (STATUS, PropertyValue::Code(status))
                    if status == "retired" || status == "deprecated" =>
                {
                    active = false;
                    reason.get_or_insert_with(|| status.clone());
                }
                (DEPRECATED, PropertyValue::DateTime(when)) => {
                    active = false;
                    reason.get_or_insert_with(|| format!("deprecated {when}"));
                }
                (NOT_SELECTABLE, PropertyValue::Boolean(true)) => abstract_concept = true,
                _ => {}
            }
        }
        Status {
            standards_status: entry.standards_status.clone(),
            active,
            inactive_reason: reason,
            abstract_concept,
        }
    }
}

impl CodeSystemProvider for FhirCodeSystem {
    fn identity(&self) -> &Identity {
        &self.identity
    }

    fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    fn locate(&self, code: &str) -> Result<Option<Located>, ProviderError> {
        self.complete_enough()?;
        let key = if self.model.case_sensitive {
            code.to_owned()
        } else {
            code.to_lowercase()
        };
        Ok(self.codes.get(&key).map(|&ordinal| Located {
            concept: Concept::new(ordinal),
            code: self
                .model
                .concepts
                .get(usize::try_from(ordinal).unwrap_or(usize::MAX))
                .map_or_else(|| code.to_owned(), |c| c.code.clone()),
        }))
    }

    fn code(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(self.entry(concept).map(|c| c.code.clone()))
    }

    fn display(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        let Some(entry) = self.entry(concept) else {
            return Ok(None);
        };
        if let Some(language) = language
            && let Some(designation) = entry.designations.iter().find(|d| {
                d.language
                    .as_deref()
                    .is_some_and(|l| text_match::same_language(l, language))
            })
        {
            return Ok(Some(designation.value.clone()));
        }
        Ok(entry
            .display
            .clone()
            .or_else(|| entry.designations.first().map(|d| d.value.clone())))
    }

    fn definition(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(self.entry(concept).and_then(|c| c.definition.clone()))
    }

    fn language(&self) -> Option<&str> {
        self.model.language.as_deref()
    }

    fn standing(&self) -> crate::provider::Standing {
        crate::provider::Standing {
            status: self.model.status.clone(),
            experimental: self.model.experimental.unwrap_or(false),
            standards_status: self.model.standards_status.clone(),
        }
    }

    fn status(&self, concept: Concept) -> Result<Status, ProviderError> {
        Ok(self.entry(concept).map(Self::status_of).unwrap_or_default())
    }

    fn designations(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Vec<Designation>, ProviderError> {
        Ok(self
            .entry(concept)
            .map(|c| {
                c.designations
                    .iter()
                    .filter(|d| {
                        language.is_none_or(|wanted| {
                            d.language
                                .as_deref()
                                .is_some_and(|l| text_match::same_language(l, wanted))
                        })
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    fn properties(&self, concept: Concept) -> Result<Vec<Property>, ProviderError> {
        let Some(entry) = self.entry(concept) else {
            return Ok(Vec::new());
        };
        let status = Self::status_of(entry);
        let mut out = vec![Property {
            code: INACTIVE.to_owned(),
            value: PropertyValue::Boolean(!status.active),
            ..Property::default()
        }];
        if status.abstract_concept {
            out.push(Property {
                code: NOT_SELECTABLE.to_owned(),
                value: PropertyValue::Boolean(true),
                ..Property::default()
            });
        }
        for property in &entry.properties {
            if property.code == INACTIVE
                || property.code == NOT_SELECTABLE
                || property.code == PARENT
                || property.code == CHILD
            {
                continue;
            }
            out.push(property.clone());
        }
        if let Some(hierarchy) = &self.hierarchy {
            for parent in hierarchy.parents(concept) {
                if let Some(code) = self.code(Concept::new(parent))? {
                    out.push(Property {
                        code: PARENT.to_owned(),
                        value: PropertyValue::Code(code),
                        ..Property::default()
                    });
                }
            }
            for child in hierarchy.children(concept) {
                if let Some(code) = self.code(Concept::new(child))? {
                    out.push(Property {
                        code: CHILD.to_owned(),
                        value: PropertyValue::Code(code),
                        ..Property::default()
                    });
                }
            }
        }
        Ok(out)
    }

    fn hierarchy(&self) -> Option<&dyn Hierarchy> {
        self.hierarchy.as_ref().map(|h| h as &dyn Hierarchy)
    }

    fn all(&self) -> Result<ConceptSet, ProviderError> {
        if !self
            .declaration
            .capabilities
            .contains(&Capability::Enumeration)
        {
            return Err(ProviderError::NotEnumerable);
        }
        Ok(self.all.clone())
    }

    fn search(&self, text: &str, language: Option<&str>) -> Result<ConceptSet, ProviderError> {
        self.complete_enough()?;
        let words = text_match::query_words(text);
        let mut hits = ConceptSet::new();
        for (i, entry) in self.model.concepts.iter().enumerate() {
            let mut terms: Vec<&str> = Vec::new();
            if language.is_none() {
                terms.extend(entry.display.as_deref());
            }
            terms.extend(
                entry
                    .designations
                    .iter()
                    .filter(|d| {
                        language.is_none_or(|wanted| {
                            d.language
                                .as_deref()
                                .is_some_and(|l| text_match::same_language(l, wanted))
                        })
                    })
                    .map(|d| d.value.as_str()),
            );
            if text_match::matches_all(&words, &terms) {
                hits.insert(u32::try_from(i).unwrap_or(u32::MAX));
            }
        }
        Ok(hits)
    }
}

/// The is-a hierarchy of `model`, when `hierarchyMeaning` is `is-a` and a
/// concept names a parent.
fn build_hierarchy(
    model: &CodeSystemModel,
    count: u32,
) -> Result<Option<ModelHierarchy>, BuildError> {
    let has_parents = model.concepts.iter().any(|c| !c.parents.is_empty());
    // NOTE: no version defines the hierarchy when `hierarchyMeaning` is absent
    // (<https://hl7.org/fhir/R4B/codesystem.html>); the ecosystem suite reads nested
    // concepts as is-a, so only another stated meaning withholds subsumption.
    let is_a = model
        .hierarchy_meaning
        .is_none_or(|meaning| meaning == HierarchyMeaning::IsA);
    if !is_a || !has_parents {
        return Ok(None);
    }
    let by_code: BTreeMap<&str, u32> = model
        .concepts
        .iter()
        .enumerate()
        .map(|(i, c)| (c.code.as_str(), u32::try_from(i).unwrap_or(u32::MAX)))
        .collect();
    let edges: Vec<(Ordinal, Ordinal)> = model
        .concepts
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            let child = Ordinal::new(u32::try_from(i).unwrap_or(u32::MAX));
            c.parents
                .iter()
                .filter_map(|p| by_code.get(p.as_str()).map(|&o| (child, Ordinal::new(o))))
                .collect::<Vec<_>>()
        })
        .collect();
    let too_many = || BuildError::TooMany {
        url: model.url.clone(),
    };
    let parents = Csr::build(count, edges).map_err(|_| too_many())?;
    let closure = Closure::compute(&parents).map_err(|_| BuildError::Cycle {
        url: model.url.clone(),
    })?;
    let children = parents.transpose().map_err(|_| too_many())?;
    Ok(Some(ModelHierarchy {
        parents,
        children,
        closure,
    }))
}

/// The distinct languages of `model`, its own and its designations', sorted.
fn languages_of(model: &CodeSystemModel) -> Vec<String> {
    let mut languages: BTreeSet<String> = BTreeSet::new();
    languages.extend(model.language.clone());
    for concept in &model.concepts {
        for designation in &concept.designations {
            if let Some(language) = &designation.language {
                languages.insert(language.clone());
            }
        }
    }
    languages.into_iter().collect()
}

/// `declared` plus the standard properties every provider answers.
///
/// The URIs are the standard concept properties
/// (<https://hl7.org/fhir/R4B/codesystem-concept-properties.html>).
fn with_standard_properties(declared: &[PropertyDefinition]) -> Vec<PropertyDefinition> {
    let mut properties = declared.to_vec();
    for (code, kind) in [
        (INACTIVE, PropertyKind::Boolean),
        (NOT_SELECTABLE, PropertyKind::Boolean),
        (PARENT, PropertyKind::Code),
        (CHILD, PropertyKind::Code),
    ] {
        if !properties.iter().any(|p| p.code == code) {
            properties.push(PropertyDefinition {
                code: code.to_owned(),
                uri: Some(format!("http://hl7.org/fhir/concept-properties#{code}")),
                description: None,
                kind,
            });
        }
    }
    properties
}
