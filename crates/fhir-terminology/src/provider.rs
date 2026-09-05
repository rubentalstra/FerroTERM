//! The code system provider seam.
//!
//! Every code system reaches the operations through [`CodeSystemProvider`]:
//! an opaque concept handle, the FHIR `CodeSystem` metadata as its capability
//! declaration (<https://hl7.org/fhir/R4B/codesystem.html>), and the reads the
//! operations need. Storage stays behind the provider; the compose layer,
//! paging, and dedup live once above it.

use std::collections::BTreeSet;
use std::fmt;

use roaring::RoaringBitmap;

use concept_graph::subsumption::Outcome;

use crate::compose::Compose;
use crate::filter::{Filter, FilterOperator};

/// An opaque handle to one concept of one code system version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Concept(u32);

impl Concept {
    /// A handle over a dense index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The dense index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// A set of concept handles, in sorted order.
pub type ConceptSet = RoaringBitmap;

/// Every ordinal below `concepts`, the selection a system's whole content is.
///
/// A range fills roaring's containers whole
/// (<https://docs.rs/roaring/0.11.5/roaring/struct.RoaringBitmap.html#method.insert_range>),
/// which an insert per ordinal does not.
#[must_use]
pub fn every(concepts: u32) -> ConceptSet {
    let mut set = ConceptSet::new();
    set.insert_range(0..concepts);
    set
}

/// Who a code system version is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The canonical system URI (`CodeSystem.url`).
    pub url: String,
    /// The version string (`CodeSystem.version`).
    pub version: String,
    /// The computer-friendly name (`CodeSystem.name`).
    pub name: Option<String>,
    /// A human title (`CodeSystem.title`).
    pub title: Option<String>,
    /// Whether a version is needed to interpret a code (`CodeSystem.versionNeeded`).
    pub version_needed: bool,
}

/// `CodeSystem.content` (<https://hl7.org/fhir/R4B/codesystem-content-mode.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentMode {
    /// `not-present`: the server holds the content and serves it.
    NotPresent,
    /// `example`.
    Example,
    /// `fragment`.
    Fragment,
    /// `complete`.
    Complete,
    /// `supplement`.
    Supplement,
}

impl ContentMode {
    /// The FHIR code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotPresent => "not-present",
            Self::Example => "example",
            Self::Fragment => "fragment",
            Self::Complete => "complete",
            Self::Supplement => "supplement",
        }
    }
}

/// `CodeSystem.hierarchyMeaning` (<https://hl7.org/fhir/R4B/codesystem-hierarchy-meaning.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyMeaning {
    /// `grouped-by`.
    GroupedBy,
    /// `is-a`: subsumption.
    IsA,
    /// `part-of`.
    PartOf,
    /// `classified-with`.
    ClassifiedWith,
}

impl HierarchyMeaning {
    /// The FHIR code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::GroupedBy => "grouped-by",
            Self::IsA => "is-a",
            Self::PartOf => "part-of",
            Self::ClassifiedWith => "classified-with",
        }
    }
}

/// `CodeSystem.property.type` (<https://hl7.org/fhir/R4B/concept-property-type.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyKind {
    /// `code`.
    Code,
    /// `Coding`.
    Coding,
    /// `string`.
    String,
    /// `integer`.
    Integer,
    /// `boolean`.
    Boolean,
    /// `dateTime`.
    DateTime,
    /// `decimal`.
    Decimal,
}

impl PropertyKind {
    /// The FHIR code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Coding => "Coding",
            Self::String => "string",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::DateTime => "dateTime",
            Self::Decimal => "decimal",
        }
    }
}

/// One declared property (`CodeSystem.property`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDefinition {
    /// The property code.
    pub code: String,
    /// The formal identity of the property, if any.
    pub uri: Option<String>,
    /// A description.
    pub description: Option<String>,
    /// The value type.
    pub kind: PropertyKind,
}

/// One declared filter (`CodeSystem.filter`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterDefinition {
    /// The filter code (the property a filter names).
    pub code: String,
    /// A description.
    pub description: Option<String>,
    /// The operators the filter accepts.
    pub operators: Vec<FilterOperator>,
    /// What the filter value is (`CodeSystem.filter.value`).
    pub value: String,
}

/// An optional capability a provider declares rather than every system assumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// Subsumption and the hierarchy filters; `hierarchy()` returns `Some`.
    Subsumption,
    /// Enumeration of every concept, for expansion without a filter.
    Enumeration,
    /// Implicit value sets parsed from the system URI.
    ImplicitValueSets,
    /// Implicit concept maps parsed from the system URI.
    ImplicitConceptMaps,
    /// Alternate or normalized codes resolve through `locate`.
    NormalizedCodes,
}

/// What a provider declares about its code system (the `TerminologyCapabilities` input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// `CodeSystem.content`.
    pub content: ContentMode,
    /// `CodeSystem.caseSensitive`.
    pub case_sensitive: bool,
    /// `CodeSystem.hierarchyMeaning`, when the system has a hierarchy.
    pub hierarchy_meaning: Option<HierarchyMeaning>,
    /// `CodeSystem.compositional`.
    pub compositional: bool,
    /// The designation languages the system carries (BCP 47).
    pub languages: Vec<String>,
    /// The declared properties.
    pub properties: Vec<PropertyDefinition>,
    /// The declared filters, beyond the generic ones every provider answers.
    pub filters: Vec<FilterDefinition>,
    /// The optional capabilities.
    pub capabilities: BTreeSet<Capability>,
}

/// A located code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The concept.
    pub concept: Concept,
    /// The code as the system spells it, which differs from the input for a
    /// case-insensitive or normalized match.
    pub code: String,
}

/// The standing of a code system as a resource: its publication status and
/// the terminology ecosystem's notes on referencing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// `CodeSystem.status` (`draft`, `active`, `retired`, `unknown`).
    pub status: String,
    /// `CodeSystem.experimental`.
    pub experimental: bool,
    /// The `structuredefinition-standards-status` extension's code, when set.
    pub standards_status: Option<String>,
}

impl Default for Standing {
    fn default() -> Self {
        Self {
            status: String::from("active"),
            experimental: false,
            standards_status: None,
        }
    }
}

/// The metadata an implicit value set carries beside its compose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImplicitMetadata {
    /// `ValueSet.version`.
    pub version: Option<String>,
    /// `ValueSet.name`.
    pub name: Option<String>,
    /// `ValueSet.title`.
    pub title: Option<String>,
    /// `ValueSet.experimental`.
    pub experimental: Option<bool>,
    /// `ValueSet.date`.
    pub date: Option<String>,
}

/// The status of a concept, for `inactive` and `abstract` on the wire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`standards_status` is the name of the FHIR extension it carries"
)]
pub struct Status {
    /// The concept's `structuredefinition-standards-status` code, when set
    /// (`deprecated`, `withdrawn`, ...): a note beside an active concept.
    pub standards_status: Option<String>,
    /// Whether the concept is active.
    pub active: bool,
    /// Why it is inactive, as the system states it.
    pub inactive_reason: Option<String>,
    /// Whether the concept is abstract (`notSelectable`).
    pub abstract_concept: bool,
    /// Whether the concept has no code of its own and is addressed by its URI
    /// (an ICD-11 grouper); `$lookup` answers it `notSelectable` as a property
    /// and no `abstract`, expansions and validation keep `abstract_concept`.
    pub codeless: bool,
}

/// The `use` of a designation (`ValueSet.expansion.contains.designation.use`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignationUse {
    /// The system of the use code.
    pub system: String,
    /// The use code.
    pub code: String,
    /// The display of the use code.
    pub display: Option<String>,
}

/// One designation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Designation {
    /// The BCP 47 language.
    pub language: Option<String>,
    /// The use.
    pub use_: Option<DesignationUse>,
    /// The text.
    pub value: String,
    /// The designation's `structuredefinition-standards-status` code, when
    /// set: a `withdrawn` or `deprecated` designation is no longer a correct
    /// display.
    pub standards_status: Option<String>,
}

/// A typed property value (`Parameters.parameter.part.value[x]` of `$lookup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// A `code`.
    Code(String),
    /// A `uri`.
    Uri(String),
    /// A `Coding`.
    Coding {
        /// The system.
        system: String,
        /// The code.
        code: String,
        /// The display.
        display: Option<String>,
    },
    /// A `string`.
    String(String),
    /// An `integer`.
    Integer(i64),
    /// A `boolean`.
    Boolean(bool),
    /// A `dateTime`, in its lexical form.
    DateTime(String),
    /// A `decimal`, in its lexical form.
    Decimal(String),
}

impl PropertyValue {
    /// The value as text, the form filters compare against.
    #[must_use]
    pub fn as_text(&self) -> String {
        match self {
            Self::Code(text)
            | Self::Uri(text)
            | Self::String(text)
            | Self::DateTime(text)
            | Self::Decimal(text) => text.clone(),
            Self::Coding { code, .. } => code.clone(),
            Self::Integer(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
        }
    }
}

/// One property of a concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// The property code.
    pub code: String,
    /// The value.
    pub value: PropertyValue,
    /// A human description of the value (`$lookup` `property.description`).
    pub description: Option<String>,
    /// The parts of a structured value (`$lookup` `property.subproperty`).
    pub subproperties: Vec<Subproperty>,
}

impl Default for Property {
    fn default() -> Self {
        Self {
            code: String::new(),
            value: PropertyValue::String(String::new()),
            description: None,
            subproperties: Vec::new(),
        }
    }
}

/// One part of a structured property value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subproperty {
    /// The part's code.
    pub code: String,
    /// The part's value.
    pub value: PropertyValue,
    /// A human description of the part.
    pub description: Option<String>,
}

/// One concept a system names in place of an inactive one.
///
/// SNOMED CT states these in its historical association reference sets, so a
/// translation of a retired concept answers with its successor
/// (<https://hl7.org/fhir/R4B/snomedct.html>).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Successor {
    /// The code that stands in for the inactive one.
    pub code: String,
    /// Its display.
    pub display: Option<String>,
    /// The relationship the association asserts.
    pub relationship: crate::conceptmap::model::Relationship,
    /// The canonical of the implicit concept map the association forms.
    pub map: String,
}

/// A failure inside a provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The system cannot decide the relationship asked of it.
    #[error("cannot determine: {0}")]
    CannotDetermine(String),
    /// The system cannot enumerate its concepts (a grammar-defined system).
    #[error("the code system cannot enumerate its concepts")]
    NotEnumerable,
    /// The provider does not answer this filter.
    #[error("filter `{property}` with operator `{operator}` is not supported")]
    UnsupportedFilter {
        /// The filter property.
        property: String,
        /// The operator code.
        operator: String,
    },
    /// The filter value is not what the operator expects.
    #[error("filter `{property}` value `{value}` is invalid: {reason}")]
    InvalidFilterValue {
        /// The filter property.
        property: String,
        /// The offending value.
        value: String,
        /// Why.
        reason: String,
    },
    /// A regular expression does not compile.
    #[error("invalid regular expression")]
    Regex(#[from] regex::Error),
    /// A filter names a code the system does not have.
    #[error("unknown code `{0}`")]
    UnknownCode(String),
    /// The code is malformed for the code system's grammar, and the system
    /// can say why (a postcoordination value on no axis of its stem).
    #[error("code `{code}` is invalid: {reason}")]
    InvalidCode {
        /// The code as given.
        code: String,
        /// Why it is not a code.
        reason: String,
    },
    /// The storage behind the provider failed; the cause is the substrate's error.
    #[error("the code system storage failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    /// The code system's content is not present or is an example, so a code
    /// cannot be validated or enumerated against it.
    #[error("code system `{system}` has content `{content}`; its codes cannot be checked")]
    IncompleteContent {
        /// The system.
        system: String,
        /// The `content` mode.
        content: &'static str,
    },
    /// An implicit value set URI of this system is malformed.
    #[error("implicit value set `{url}` is malformed: {reason}")]
    MalformedImplicitValueSet {
        /// The URI.
        url: String,
        /// Why.
        reason: String,
    },
    /// An implicit concept map URI of this system is malformed.
    #[error("implicit concept map `{url}` is malformed: {reason}")]
    MalformedImplicitConceptMap {
        /// The URI.
        url: String,
        /// Why.
        reason: String,
    },
    /// An implicit concept map URI names a map this system does not hold.
    #[error("implicit concept map `{url}` names no map this code system holds")]
    UnknownImplicitConceptMap {
        /// The URI.
        url: String,
    },
    /// An implicit URI of this system names a version of it that this provider
    /// is not, so another loaded version of the system may answer it.
    #[error("unknown version `{version}` of code system `{url}`")]
    UnservedImplicitVersion {
        /// The system.
        url: String,
        /// The version the implicit URI names.
        version: String,
    },
}

/// The hierarchy of a system whose `hierarchyMeaning` is `is-a`.
pub trait Hierarchy: fmt::Debug {
    /// The direct parents.
    fn parents(&self, concept: Concept) -> ConceptSet;
    /// Whether any direct parent of `concept` lies in `set`.
    ///
    /// This is the root test of a nested expansion, asked once per member of
    /// the selection, so the default's set per call is worth overriding wherever
    /// the adjacency is already a list.
    fn any_parent_in(&self, concept: Concept, set: &ConceptSet) -> bool {
        self.parents(concept)
            .iter()
            .any(|parent| set.contains(parent))
    }
    /// The direct children.
    fn children(&self, concept: Concept) -> ConceptSet;
    /// Every ancestor, excluding the concept.
    fn ancestors(&self, concept: Concept) -> ConceptSet;
    /// Every descendant, excluding the concept.
    fn descendants(&self, concept: Concept) -> ConceptSet;
    /// Whether `a` subsumes `b`, in the FHIR `$subsumes` vocabulary.
    fn subsumes(&self, a: Concept, b: Concept) -> Outcome {
        if a == b {
            Outcome::Equivalent
        } else if self.descendants(a).contains(b.index()) {
            Outcome::Subsumes
        } else if self.descendants(b).contains(a.index()) {
            Outcome::SubsumedBy
        } else {
            Outcome::NotSubsumed
        }
    }
}

/// One code system version behind the seam.
pub trait CodeSystemProvider: fmt::Debug + Send + Sync {
    /// Who this version is.
    fn identity(&self) -> &Identity;

    /// What it declares.
    fn declaration(&self) -> &Declaration;

    /// Finds a code; `Ok(None)` when the system has no such code.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails; a code that
    /// is absent is never an error.
    fn locate(&self, code: &str) -> Result<Option<Located>, ProviderError>;

    /// The code of a concept.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn code(&self, concept: Concept) -> Result<Option<String>, ProviderError>;

    /// The display in `language`, or the system's default display.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn display(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Option<String>, ProviderError>;

    /// The formal definition, if the system has one.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn definition(&self, _concept: Concept) -> Result<Option<String>, ProviderError> {
        Ok(None)
    }

    /// The language of the system's own displays (`CodeSystem.language`),
    /// when the system states one.
    fn language(&self) -> Option<&str> {
        None
    }

    /// The system's standing as a resource; active and unremarkable unless the
    /// system states otherwise.
    fn standing(&self) -> Standing {
        Standing::default()
    }

    /// Active or inactive, abstract or not.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn status(&self, concept: Concept) -> Result<Status, ProviderError>;

    /// Every inactive concept, for `activeOnly` and `compose.inactive = false`
    /// over a large selection.
    ///
    /// The default scans every concept's status; a provider with a cheaper
    /// answer overrides.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotEnumerable`] when the system cannot
    /// enumerate, and [`ProviderError::Storage`] when the substrate fails.
    fn inactive(&self) -> Result<ConceptSet, ProviderError> {
        let mut inactive = ConceptSet::new();
        for index in self.all()? {
            if !self.status(Concept::new(index))?.active {
                inactive.insert(index);
            }
        }
        Ok(inactive)
    }

    /// The concepts an expansion for use outside a user interface leaves out:
    /// the abstract (`notSelectable`) groupers, for `excludeNotForUI`
    /// (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>).
    ///
    /// The default scans every concept's status; a provider with a cheaper
    /// answer overrides.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotEnumerable`] when the system cannot
    /// enumerate, and [`ProviderError::Storage`] when the substrate fails.
    fn not_for_ui(&self) -> Result<ConceptSet, ProviderError> {
        let mut abstract_concepts = ConceptSet::new();
        for index in self.all()? {
            if self.status(Concept::new(index))?.abstract_concept {
                abstract_concepts.insert(index);
            }
        }
        Ok(abstract_concepts)
    }

    /// Whether `concept` is a post-coordinated expression the system composed
    /// on request, for `excludePostCoordinated`; a system without a grammar
    /// has none.
    fn is_postcoordinated(&self, _concept: Concept) -> bool {
        false
    }

    /// Every designation, optionally only those in `language`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn designations(
        &self,
        concept: Concept,
        language: Option<&str>,
    ) -> Result<Vec<Designation>, ProviderError>;

    /// Every property.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn properties(&self, concept: Concept) -> Result<Vec<Property>, ProviderError>;

    /// The hierarchy, for a system that declares subsumption.
    fn hierarchy(&self) -> Option<&dyn Hierarchy> {
        None
    }

    /// Every concept, for a system that can enumerate.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotEnumerable`] for a grammar-defined system.
    fn all(&self) -> Result<ConceptSet, ProviderError>;

    /// The concepts with a designation whose words start with the words of `text`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn search(&self, text: &str, language: Option<&str>) -> Result<ConceptSet, ProviderError>;

    /// The compose an implicit value set URI of this system denotes, when the
    /// system defines implicit value sets and `url` is one of them.
    ///
    /// `None` when the URI is not an implicit value set of this system; the
    /// error when it is malformed. The default declares none.
    fn implicit_value_set(&self, _url: &str) -> Option<Result<Compose, ProviderError>> {
        None
    }

    /// The metadata of the implicit value set `url` denotes: the fields the
    /// returned `ValueSet` carries beside its compose. The default carries
    /// none; a system whose implicit sets have a version or a name of their
    /// own says so.
    fn implicit_metadata(&self, _url: &str) -> ImplicitMetadata {
        ImplicitMetadata::default()
    }

    /// The concepts this system names in place of an inactive one, from its
    /// own historical associations. The default declares none.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Storage`] when the substrate fails.
    fn successors(&self, _concept: Concept) -> Result<Vec<Successor>, ProviderError> {
        Ok(Vec::new())
    }

    /// The `ConceptMap` an implicit concept map URI of this system denotes,
    /// when the system defines implicit concept maps and `url` is one of them.
    ///
    /// `None` when the URI is not an implicit concept map of this system; the
    /// error when it is malformed or names a map the system does not hold.
    /// The default declares none.
    fn implicit_concept_map(
        &self,
        _url: &str,
    ) -> Option<Result<crate::conceptmap::model::ConceptMapModel, ProviderError>> {
        None
    }

    /// The concepts a filter selects.
    ///
    /// The default answers the generic operators on `concept` and `code`, the
    /// hierarchy operators when the system declares subsumption, and `=`,
    /// `in`, `not-in`, `regex`, `exists` over declared properties by scanning
    /// every concept; a provider overrides for its own filters and indexes.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::UnsupportedFilter`] for an operator or property
    /// the system does not answer, and the value errors of the operators.
    fn filter(&self, filter: &Filter) -> Result<ConceptSet, ProviderError> {
        crate::filter::evaluate(self, filter)
    }

    /// The concepts every filter of one include selects, all of them.
    ///
    /// The default intersects [`CodeSystemProvider::filter`] per filter; a
    /// system whose filters only make sense together (a registry flag that
    /// bounds an otherwise unbounded grammar) overrides.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`CodeSystemProvider::filter`], and
    /// [`ProviderError::NotEnumerable`] when the filters do not bound the set.
    fn filter_all(&self, filters: &[Filter]) -> Result<ConceptSet, ProviderError> {
        let Some((first, rest)) = filters.split_first() else {
            return self.all();
        };
        let mut set = self.filter(first)?;
        for filter in rest {
            set &= self.filter(filter)?;
        }
        Ok(set)
    }

    /// Whether `concept` satisfies `filter`, without enumerating the set.
    ///
    /// The default evaluates the filter and tests membership; a grammar system
    /// answers from the concept alone.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`CodeSystemProvider::filter`].
    fn filter_matches(&self, concept: Concept, filter: &Filter) -> Result<bool, ProviderError> {
        Ok(self.filter(filter)?.contains(concept.index()))
    }

    /// The subsumption of `a` over `b` when the system decides it without a
    /// materialized hierarchy (a grammar whose parameters narrow a code).
    ///
    /// `None` leaves the answer to [`CodeSystemProvider::hierarchy`].
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::CannotDetermine`] when the system cannot say.
    fn subsumes(&self, _a: Concept, _b: Concept) -> Result<Option<Outcome>, ProviderError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentMode, HierarchyMeaning, PropertyKind};

    // The three enums below cross the wire: `content` and `hierarchyMeaning`
    // on a CodeSystem, and `type` on a property definition. Each variant's code
    // is fixed by the specification, so each is pinned rather than left to
    // whichever variant a provider happens to use.

    #[test]
    fn a_content_mode_carries_the_code_the_specification_defines() {
        // <https://hl7.org/fhir/R4B/valueset-codesystem-content-mode.html>
        assert_eq!(ContentMode::NotPresent.code(), "not-present");
        assert_eq!(ContentMode::Example.code(), "example");
        assert_eq!(ContentMode::Fragment.code(), "fragment");
        assert_eq!(ContentMode::Complete.code(), "complete");
        assert_eq!(ContentMode::Supplement.code(), "supplement");
    }

    #[test]
    fn a_hierarchy_meaning_carries_the_code_the_specification_defines() {
        // <https://hl7.org/fhir/R4B/valueset-codesystem-hierarchy-meaning.html>
        assert_eq!(HierarchyMeaning::GroupedBy.code(), "grouped-by");
        assert_eq!(HierarchyMeaning::IsA.code(), "is-a");
        assert_eq!(HierarchyMeaning::PartOf.code(), "part-of");
        assert_eq!(HierarchyMeaning::ClassifiedWith.code(), "classified-with");
    }

    #[test]
    fn a_property_kind_carries_the_code_the_specification_defines() {
        // <https://hl7.org/fhir/R4B/valueset-concept-property-type.html>;
        // `Coding` is the one code of the set that is not lower case.
        assert_eq!(PropertyKind::Code.code(), "code");
        assert_eq!(PropertyKind::Coding.code(), "Coding");
        assert_eq!(PropertyKind::String.code(), "string");
        assert_eq!(PropertyKind::Integer.code(), "integer");
        assert_eq!(PropertyKind::Boolean.code(), "boolean");
        assert_eq!(PropertyKind::DateTime.code(), "dateTime");
        assert_eq!(PropertyKind::Decimal.code(), "decimal");
    }
}
