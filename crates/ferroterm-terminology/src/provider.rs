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

use ferroterm_graph::subsumption::Outcome;

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

/// Who a code system version is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The canonical system URI (`CodeSystem.url`).
    pub url: String,
    /// The version string (`CodeSystem.version`).
    pub version: String,
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

/// The status of a concept, for `inactive` and `abstract` on the wire.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    /// Whether the concept is active.
    pub active: bool,
    /// Why it is inactive, as the system states it.
    pub inactive_reason: Option<String>,
    /// Whether the concept is abstract (`notSelectable`).
    pub abstract_concept: bool,
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
}

/// A typed property value (`Parameters.parameter.part.value[x]` of `$lookup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// A `code`.
    Code(String),
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
            Self::Code(text) | Self::String(text) | Self::DateTime(text) | Self::Decimal(text) => {
                text.clone()
            }
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
}

/// A failure inside a provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
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
    /// An implicit value set URI of this system is malformed.
    #[error("implicit value set `{url}` is malformed: {reason}")]
    MalformedImplicitValueSet {
        /// The URI.
        url: String,
        /// Why.
        reason: String,
    },
}

/// The hierarchy of a system whose `hierarchyMeaning` is `is-a`.
pub trait Hierarchy: fmt::Debug {
    /// The direct parents.
    fn parents(&self, concept: Concept) -> ConceptSet;
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

    /// Finds a code; `None` when the system has no such code.
    fn locate(&self, code: &str) -> Option<Located>;

    /// The code of a concept.
    fn code(&self, concept: Concept) -> Option<String>;

    /// The display in `language`, or the system's default display.
    fn display(&self, concept: Concept, language: Option<&str>) -> Option<String>;

    /// The formal definition, if the system has one.
    fn definition(&self, _concept: Concept) -> Option<String> {
        None
    }

    /// Active or inactive, abstract or not.
    fn status(&self, concept: Concept) -> Status;

    /// Every designation, optionally only those in `language`.
    fn designations(&self, concept: Concept, language: Option<&str>) -> Vec<Designation>;

    /// Every property.
    fn properties(&self, concept: Concept) -> Vec<Property>;

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
    fn search(&self, text: &str, language: Option<&str>) -> ConceptSet;

    /// The compose an implicit value set URI of this system denotes, when the
    /// system defines implicit value sets and `url` is one of them.
    ///
    /// `None` when the URI is not an implicit value set of this system; the
    /// error when it is malformed. The default declares none.
    fn implicit_value_set(&self, _url: &str) -> Option<Result<Compose, ProviderError>> {
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
}
