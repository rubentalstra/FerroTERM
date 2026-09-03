//! Code system supplements applied to a provider's designations and properties
//! (`CodeSystem.content = supplement`, <https://hl7.org/fhir/R4B/codesystem.html#supplements>).

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::provider::{
    CodeSystemProvider, Concept, ConceptSet, Declaration, Designation, Hierarchy, Identity,
    Located, Property, ProviderError, Status,
};

/// What a supplement adds to one concept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Additions {
    /// Extra designations.
    pub designations: Vec<Designation>,
    /// Extra properties.
    pub properties: Vec<Property>,
}

/// A supplement: additions keyed by the supplemented system's codes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Supplement {
    /// The supplement's own canonical URL.
    pub url: String,
    /// The supplement's version.
    pub version: Option<String>,
    /// The additions per code.
    pub concepts: BTreeMap<String, Additions>,
}

impl Supplement {
    /// The supplement a `CodeSystem` with `content = supplement` describes:
    /// its designations and properties keyed by the supplemented codes.
    #[must_use]
    pub fn from_code_system(model: &crate::fhir_codesystem::model::CodeSystemModel) -> Self {
        let mut concepts = BTreeMap::new();
        for entry in &model.concepts {
            concepts.insert(
                entry.code.clone(),
                Additions {
                    designations: entry.designations.clone(),
                    properties: entry.properties.clone(),
                },
            );
        }
        Self {
            url: model.url.clone(),
            version: Some(model.version.clone()).filter(|v| !v.is_empty()),
            concepts,
        }
    }
}

/// A provider with supplements layered on.
#[derive(Debug)]
pub struct Supplemented {
    inner: Arc<dyn CodeSystemProvider>,
    supplements: Vec<Supplement>,
    declaration: Declaration,
}

impl Supplemented {
    /// Layers `supplements` over `inner`; their languages join the declaration.
    #[must_use]
    pub fn new(inner: Arc<dyn CodeSystemProvider>, supplements: Vec<Supplement>) -> Self {
        let mut declaration = inner.declaration().clone();
        for supplement in &supplements {
            for additions in supplement.concepts.values() {
                for designation in &additions.designations {
                    if let Some(language) = &designation.language
                        && !declaration.languages.contains(language)
                    {
                        declaration.languages.push(language.clone());
                    }
                }
            }
        }
        declaration.languages.sort();
        Self {
            inner,
            supplements,
            declaration,
        }
    }

    /// The supplements applied.
    #[must_use]
    pub fn supplements(&self) -> &[Supplement] {
        &self.supplements
    }

    fn additions(&self, concept: Concept) -> Result<Vec<&Additions>, ProviderError> {
        let code = self.inner.code(concept)?;
        Ok(self
            .supplements
            .iter()
            .filter_map(|supplement| {
                code.as_deref()
                    .and_then(|code| supplement.concepts.get(code))
            })
            .collect())
    }
}

impl CodeSystemProvider for Supplemented {
    fn identity(&self) -> &Identity {
        self.inner.identity()
    }

    fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    fn locate(&self, code: &str) -> Result<Option<Located>, ProviderError> {
        self.inner.locate(code)
    }

    fn code(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        self.inner.code(concept)
    }

    fn display(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        // NOTE: a supplement adds designations the system lacks, so its term in
        // the requested language beats the system's fallback display
        // (<https://hl7.org/fhir/R4B/codesystem.html#supplements>).
        if let Some(wanted) = language
            && self.inner.designations(concept, Some(wanted))?.is_empty()
            && let Some(added) = self
                .additions(concept)?
                .into_iter()
                .flat_map(|additions| additions.designations.iter())
                .find(|designation| designation.language.as_deref() == Some(wanted))
        {
            return Ok(Some(added.value.clone()));
        }
        if let Some(display) = self.inner.display(concept, language)? {
            return Ok(Some(display));
        }
        Ok(self
            .additions(concept)?
            .into_iter()
            .flat_map(|additions| additions.designations.iter())
            .find(|designation| {
                language.is_none_or(|wanted| designation.language.as_deref() == Some(wanted))
            })
            .map(|designation| designation.value.clone()))
    }

    fn language(&self) -> Option<&str> {
        self.inner.language()
    }

    fn standing(&self) -> crate::provider::Standing {
        self.inner.standing()
    }

    fn definition(&self, concept: Concept) -> Result<Option<String>, ProviderError> {
        self.inner.definition(concept)
    }

    fn status(&self, concept: Concept) -> Result<Status, ProviderError> {
        self.inner.status(concept)
    }

    fn designations(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Vec<Designation>, ProviderError> {
        let mut designations = self.inner.designations(concept, language)?;
        designations.extend(
            self.additions(concept)?
                .into_iter()
                .flat_map(|additions| additions.designations.iter())
                .filter(|designation| {
                    language.is_none_or(|wanted| designation.language.as_deref() == Some(wanted))
                })
                .cloned(),
        );
        Ok(designations)
    }

    fn properties(&self, concept: Concept) -> Result<Vec<Property>, ProviderError> {
        let mut properties = self.inner.properties(concept)?;
        properties.extend(
            self.additions(concept)?
                .into_iter()
                .flat_map(|additions| additions.properties.iter())
                .cloned(),
        );
        Ok(properties)
    }

    fn hierarchy(&self) -> Option<&dyn Hierarchy> {
        self.inner.hierarchy()
    }

    fn all(&self) -> Result<ConceptSet, ProviderError> {
        self.inner.all()
    }

    fn search(&self, text: &str, language: Option<&str>) -> Result<ConceptSet, ProviderError> {
        self.inner.search(text, language)
    }

    fn filter(&self, filter: &crate::filter::Filter) -> Result<ConceptSet, ProviderError> {
        self.inner.filter(filter)
    }
}
