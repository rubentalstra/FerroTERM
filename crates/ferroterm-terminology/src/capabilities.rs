//! `TerminologyCapabilities` from the loaded providers' declarations
//! (<https://hl7.org/fhir/R4B/terminologycapabilities.html>,
//! <https://hl7.org/fhir/R5/terminologycapabilities.html>).
//!
//! The neutral [`Summary`] is read off the registry once; each FHIR version
//! renders it into its generated type, and the R5-only `codeSystem.content`
//! is a generated difference, not a hand-written conditional.

use ferroterm_fhir::{r4b, r5};

use crate::filter::FilterOperator;
use crate::provider::{Capability, ContentMode};
use crate::registry::Registry;

/// One filter a version supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterSupport {
    /// The filter code.
    pub code: String,
    /// The operators.
    pub operators: Vec<FilterOperator>,
}

/// One code system version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSummary {
    /// The version string.
    pub code: String,
    /// Whether it is the default.
    pub is_default: bool,
    /// Whether compositional grammar is supported.
    pub compositional: bool,
    /// The designation languages.
    pub languages: Vec<String>,
    /// The filters, generic ones first.
    pub filters: Vec<FilterSupport>,
    /// The `$lookup` properties.
    pub properties: Vec<String>,
}

/// One code system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemSummary {
    /// The system URI.
    pub url: String,
    /// The versions, sorted.
    pub versions: Vec<VersionSummary>,
    /// The content mode of the served versions.
    pub content: ContentMode,
    /// Whether subsumption is supported.
    pub subsumption: bool,
}

/// What the server can do, version-neutral.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// The code systems, sorted by URI.
    pub systems: Vec<SystemSummary>,
}

/// The generic filters every provider answers on `concept` and `code`.
const GENERIC_OPERATORS: [FilterOperator; 5] = [
    FilterOperator::Equal,
    FilterOperator::In,
    FilterOperator::NotIn,
    FilterOperator::Regex,
    FilterOperator::Exists,
];

/// The hierarchy operators a provider with subsumption answers.
const HIERARCHY_OPERATORS: [FilterOperator; 6] = [
    FilterOperator::IsA,
    FilterOperator::DescendentOf,
    FilterOperator::IsNotA,
    FilterOperator::Generalizes,
    FilterOperator::ChildOf,
    FilterOperator::DescendentLeaf,
];

impl Summary {
    /// Reads the summary off `registry`.
    #[must_use]
    pub fn of(registry: &Registry) -> Self {
        let mut systems = Vec::new();
        for url in registry.systems() {
            let default = registry.default_version(url);
            let mut versions = Vec::new();
            let mut content = ContentMode::NotPresent;
            let mut subsumption = false;
            for provider in registry.versions(url) {
                let identity = provider.identity();
                let declaration = provider.declaration();
                content = declaration.content;
                subsumption |= declaration.capabilities.contains(&Capability::Subsumption);
                let mut operators = GENERIC_OPERATORS.to_vec();
                if declaration.capabilities.contains(&Capability::Subsumption) {
                    operators.extend(HIERARCHY_OPERATORS);
                }
                let mut filters = vec![FilterSupport {
                    code: String::from("concept"),
                    operators,
                }];
                filters.extend(declaration.filters.iter().map(|filter| FilterSupport {
                    code: filter.code.clone(),
                    operators: filter.operators.clone(),
                }));
                versions.push(VersionSummary {
                    code: identity.version.clone(),
                    is_default: default == Some(identity.version.as_str()),
                    compositional: declaration.compositional,
                    languages: declaration.languages.clone(),
                    filters,
                    properties: declaration
                        .properties
                        .iter()
                        .map(|property| property.code.clone())
                        .collect(),
                });
            }
            systems.push(SystemSummary {
                url: url.to_owned(),
                versions,
                content,
                subsumption,
            });
        }
        Self { systems }
    }

    /// The R4B resource; `date` is the statement's `dateTime`.
    #[must_use]
    pub fn to_r4b(&self, date: &str) -> r4b::terminology_capabilities::TerminologyCapabilities {
        use r4b::terminology_capabilities::{
            TerminologyCapabilities, TerminologyCapabilitiesCodeSystem,
            TerminologyCapabilitiesCodeSystemVersion,
            TerminologyCapabilitiesCodeSystemVersionFilter, TerminologyCapabilitiesExpansion,
        };
        TerminologyCapabilities {
            status: "active".into(),
            date: date.into(),
            kind: "instance".into(),
            code_system: self
                .systems
                .iter()
                .map(|system| TerminologyCapabilitiesCodeSystem {
                    uri: Some(system.url.as_str().into()),
                    version: system
                        .versions
                        .iter()
                        .map(|version| TerminologyCapabilitiesCodeSystemVersion {
                            code: Some(version.code.as_str().into()),
                            is_default: Some(version.is_default.into()),
                            compositional: Some(version.compositional.into()),
                            language: version
                                .languages
                                .iter()
                                .map(|l| l.as_str().into())
                                .collect(),
                            filter: version
                                .filters
                                .iter()
                                .map(|filter| TerminologyCapabilitiesCodeSystemVersionFilter {
                                    code: filter.code.as_str().into(),
                                    op: filter
                                        .operators
                                        .iter()
                                        .map(|op| op.code().into())
                                        .collect(),
                                    ..Default::default()
                                })
                                .collect(),
                            property: version
                                .properties
                                .iter()
                                .map(|p| p.as_str().into())
                                .collect(),
                            ..Default::default()
                        })
                        .collect(),
                    subsumption: Some(system.subsumption.into()),
                    ..Default::default()
                })
                .collect(),
            expansion: Some(TerminologyCapabilitiesExpansion {
                hierarchical: Some(false.into()),
                paging: Some(true.into()),
                incomplete: Some(false.into()),
                text_filter: Some(TEXT_FILTER.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// The R5 resource; `date` is the statement's `dateTime`.
    #[must_use]
    pub fn to_r5(&self, date: &str) -> r5::terminology_capabilities::TerminologyCapabilities {
        use r5::terminology_capabilities::{
            TerminologyCapabilities, TerminologyCapabilitiesCodeSystem,
            TerminologyCapabilitiesCodeSystemVersion,
            TerminologyCapabilitiesCodeSystemVersionFilter, TerminologyCapabilitiesExpansion,
        };
        TerminologyCapabilities {
            status: "active".into(),
            date: date.into(),
            kind: "instance".into(),
            code_system: self
                .systems
                .iter()
                .map(|system| TerminologyCapabilitiesCodeSystem {
                    uri: Some(system.url.as_str().into()),
                    version: system
                        .versions
                        .iter()
                        .map(|version| TerminologyCapabilitiesCodeSystemVersion {
                            code: Some(version.code.as_str().into()),
                            is_default: Some(version.is_default.into()),
                            compositional: Some(version.compositional.into()),
                            language: version
                                .languages
                                .iter()
                                .map(|l| l.as_str().into())
                                .collect(),
                            filter: version
                                .filters
                                .iter()
                                .map(|filter| TerminologyCapabilitiesCodeSystemVersionFilter {
                                    code: filter.code.as_str().into(),
                                    op: filter
                                        .operators
                                        .iter()
                                        .map(|op| op.code().into())
                                        .collect(),
                                    ..Default::default()
                                })
                                .collect(),
                            property: version
                                .properties
                                .iter()
                                .map(|p| p.as_str().into())
                                .collect(),
                            ..Default::default()
                        })
                        .collect(),
                    content: system.content.code().into(),
                    subsumption: Some(system.subsumption.into()),
                    ..Default::default()
                })
                .collect(),
            expansion: Some(TerminologyCapabilitiesExpansion {
                hierarchical: Some(false.into()),
                paging: Some(true.into()),
                incomplete: Some(false.into()),
                text_filter: Some(TEXT_FILTER.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// `expansion.textFilter`: how the `filter` parameter matches.
const TEXT_FILTER: &str = "Each word of the filter is a prefix that must match the start of a word \
in one designation of the concept, in any order. Matching ignores case and diacritics. No wild cards.";
