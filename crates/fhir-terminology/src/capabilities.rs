//! `TerminologyCapabilities` from the loaded providers' declarations
//! (<https://hl7.org/fhir/R4B/terminologycapabilities.html>,
//! <https://hl7.org/fhir/R5/terminologycapabilities.html>).
//!
//! The neutral [`Summary`] is read off the registry once; each FHIR version
//! renders it into its generated type, and the R5-only `codeSystem.content`
//! is a generated difference, not a hand-written conditional.

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
}

// NOTE: R5 (5.0.0) and the R6 ballot (6.0.0-ballot5) declare the same
// TerminologyCapabilities elements this render fills, except that R6 makes
// `codeSystem.content` optional and renames `version.code` to `version.value`
// (<https://hl7.org/fhir/6.0.0-ballot5/terminologycapabilities.html>), so one
// macro with a flavour arm produces both methods.
macro_rules! r5_family_capabilities {
    ($module:ident, $name:ident, $flavour:ident) => {
        impl Summary {
            /// The resource of the version; `date` is the statement's `dateTime`.
            #[must_use]
            pub fn $name(
                &self,
                date: &str,
            ) -> fhir_types::$module::terminology_capabilities::TerminologyCapabilities {
                use fhir_types::$module::terminology_capabilities::{
                    TerminologyCapabilities, TerminologyCapabilitiesCodeSystem,
                    TerminologyCapabilitiesCodeSystemVersion,
                    TerminologyCapabilitiesCodeSystemVersionFilter, TerminologyCapabilitiesExpansion,
                    TerminologyCapabilitiesExpansionParameter,
                };
                let version_entry = |version: &VersionSummary| {
                    let mut entry = TerminologyCapabilitiesCodeSystemVersion {
                        is_default: Some(version.is_default.into()),
                        compositional: Some(version.compositional.into()),
                        language: common_languages(&version.languages)
                            .into_iter()
                            .map(Into::into)
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
                    };
                    r5_family_capabilities!(@version_code $flavour, entry, version.code.as_str().into());
                    entry
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
                            version: system.versions.iter().map(version_entry).collect(),
                            content: r5_family_capabilities!(@content $flavour, system.content.code().into()),
                            subsumption: Some(system.subsumption.into()),
                            ..Default::default()
                        })
                        .collect(),
                    expansion: Some(TerminologyCapabilitiesExpansion {
                        hierarchical: Some(false.into()),
                        paging: Some(true.into()),
                        incomplete: Some(false.into()),
                        text_filter: Some(TEXT_FILTER.into()),
                        parameter: EXPANSION_PARAMETERS
                            .iter()
                            .map(|name| TerminologyCapabilitiesExpansionParameter {
                                name: (*name).into(),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            }
        }
    };
    (@content r5, $value:expr) => {
        $value
    };
    (@content r6, $value:expr) => {
        Some($value)
    };
    (@version_code r5, $entry:ident, $value:expr) => {
        $entry.code = Some($value);
    };
    (@version_code r6, $entry:ident, $value:expr) => {
        $entry.value = Some($value);
    };
}

r5_family_capabilities!(r5, to_r5, r5);
r5_family_capabilities!(r6, to_r6, r6);

// NOTE: R4 (4.0.1) and R4B (4.3.0) declare the same TerminologyCapabilities
// elements this render fills, so one macro produces both methods.
macro_rules! terminology_capabilities {
    ($module:ident, $name:ident) => {
        impl Summary {
            /// The resource of the version; `date` is the statement's `dateTime`.
            #[must_use]
            pub fn $name(
                &self,
                date: &str,
            ) -> fhir_types::$module::terminology_capabilities::TerminologyCapabilities {
                use fhir_types::$module::terminology_capabilities::{
                    TerminologyCapabilities, TerminologyCapabilitiesCodeSystem,
                    TerminologyCapabilitiesCodeSystemVersion,
                    TerminologyCapabilitiesCodeSystemVersionFilter,
                    TerminologyCapabilitiesExpansion, TerminologyCapabilitiesExpansionParameter,
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
                                    language: common_languages(&version.languages)
                                        .into_iter()
                                        .map(Into::into)
                                        .collect(),
                                    filter: version
                                        .filters
                                        .iter()
                                        .map(|filter| {
                                            TerminologyCapabilitiesCodeSystemVersionFilter {
                                                code: filter.code.as_str().into(),
                                                op: filter
                                                    .operators
                                                    .iter()
                                                    .map(|op| op.code().into())
                                                    .collect(),
                                                ..Default::default()
                                            }
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
                        parameter: EXPANSION_PARAMETERS
                            .iter()
                            .map(|name| TerminologyCapabilitiesExpansionParameter {
                                name: (*name).into(),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            }
        }
    };
}

terminology_capabilities!(r4, to_r4);
terminology_capabilities!(r4b, to_r4b);

/// The `$expand` parameters the server evaluates, for
/// `expansion.parameter`; `tx-resource` is the terminology ecosystem's
/// (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
pub const EXPANSION_PARAMETERS: [&str; 17] = [
    "activeOnly",
    "check-system-version",
    "count",
    "designation",
    "displayLanguage",
    "exclude-system",
    "excludeNested",
    "excludeNotForUI",
    "excludePostCoordinated",
    "filter",
    "force-system-version",
    "includeDefinition",
    "includeDesignations",
    "offset",
    "property",
    "system-version",
    "tx-resource",
];

/// `expansion.textFilter`: how the `filter` parameter matches.
const TEXT_FILTER: &str = "Each word of the filter is a prefix that must match the start of a word \
in one designation of the concept, in any order. Matching ignores case and diacritics. No wild cards.";

/// The codes of the FHIR `CommonLanguages` value set
/// (<http://hl7.org/fhir/ValueSet/languages>, R4B 4.3.0).
const COMMON_LANGUAGES: [&str; 56] = [
    "ar", "bn", "cs", "da", "de", "de-AT", "de-CH", "de-DE", "el", "en", "en-AU", "en-CA", "en-GB",
    "en-IN", "en-NZ", "en-SG", "en-US", "es", "es-AR", "es-ES", "es-UY", "fi", "fr", "fr-BE",
    "fr-CH", "fr-FR", "fy", "fy-NL", "hi", "hr", "it", "it-CH", "it-IT", "ja", "ko", "nl", "nl-BE",
    "nl-NL", "no", "no-NO", "pa", "pl", "pt", "pt-BR", "ru", "ru-RU", "sr", "sr-RS", "sv", "sv-SE",
    "te", "zh", "zh-CN", "zh-HK", "zh-SG", "zh-TW",
];

/// The designation languages as `CommonLanguages` codes: a tag in the value
/// set as it is, another by its primary subtag when that is, the rest left
/// out, without repeats.
///
/// R4B binds `TerminologyCapabilities.codeSystem.version.language` to
/// nothing (<https://hl7.org/fhir/R4B/terminologycapabilities-definitions.html>),
/// but R5 binds it to `CommonLanguages` (required) and the FHIR validator
/// converts the resource to R5 before reading it, so a tag outside the set
/// fails the terminology test runner. `$lookup` designations keep every tag.
#[must_use]
pub fn common_languages(tags: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for tag in tags {
        let primary = tag.split(['-', '_']).next().unwrap_or(tag);
        let code = COMMON_LANGUAGES
            .iter()
            .find(|c| c.eq_ignore_ascii_case(tag))
            .or_else(|| {
                COMMON_LANGUAGES
                    .iter()
                    .find(|c| c.eq_ignore_ascii_case(primary))
            });
        if let Some(code) = code
            && !out.contains(code)
        {
            out.push(code);
        }
    }
    out
}

#[cfg(test)]
mod common_language_tests {
    use super::common_languages;

    #[test]
    fn common_languages_fold_to_the_value_set() {
        let tags: Vec<String> = ["en", "nl-NL", "ar-JO", "cs-CZ", "et-EE", "EN-gb", "zh-CN"]
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        assert_eq!(
            common_languages(&tags),
            ["en", "nl-NL", "ar", "cs", "en-GB", "zh-CN"],
            "ar-JO and cs-CZ fold to their primary subtag; et has no code"
        );
    }
}
