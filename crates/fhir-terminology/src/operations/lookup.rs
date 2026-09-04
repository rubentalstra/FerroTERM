//! `CodeSystem/$lookup` in the terms every served FHIR version shares.
//!
//! The input is the union of the parameters the versions declare
//! (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>,
//! <https://hl7.org/fhir/R5/codesystem-operation-lookup.html>), the outcome the
//! provider's own designations and properties; the wire layer of each version
//! maps them into that version's generated contract.

use super::{CodingRef, Invocation, OperationError, locate, resolve};
use crate::language;
use crate::provider::{
    CodeSystemProvider, Concept, Designation, DesignationUse, Property, PropertyValue,
};
use crate::registry::Registry;

/// The `$lookup` parameters `property` never repeats: they are output
/// parameters of their own.
const NAMED_ELSEWHERE: [&str; 6] = ["url", "system", "name", "version", "display", "designation"];

/// The `property` value that asks for every property.
const EVERY_PROPERTY: &str = "*";

/// The designation use of the display, from HL7's terminology maintenance
/// vocabulary (<https://terminology.hl7.org/CodeSystem-hl7TermMaintInfra.html>).
const PREFERRED_FOR_LANGUAGE: (&str, &str, &str) = (
    "http://terminology.hl7.org/CodeSystem/hl7TermMaintInfra",
    "preferredForLanguage",
    "Preferred For Language",
);

/// The input of `$lookup`: `code` with `system` (and `version`), or `coding`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LookupInput {
    /// The code.
    pub code: Option<String>,
    /// The code system URI.
    pub system: Option<String>,
    /// The code system version.
    pub version: Option<String>,
    /// The coding, instead of `code` and `system`.
    pub coding: Option<CodingRef>,
    /// The language of the display (a BCP 47 range list).
    pub display_language: Option<String>,
    /// The properties asked for; empty or `*` is every one.
    pub properties: Vec<String>,
    /// The supplements to apply (R5 `useSupplement`), by canonical.
    pub use_supplement: Vec<String>,
}

/// The outcome of `$lookup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupOutcome {
    /// The code as the system spells it (`code`, the ecosystem's output).
    pub code: String,
    /// The code system URI (`system`, the ecosystem's output).
    pub system: String,
    /// Whether the concept is abstract, `notSelectable` (`abstract`, the
    /// ecosystem's output).
    pub abstract_concept: bool,
    /// The code system's name, else its title, else its URI.
    pub name: String,
    /// The version served.
    pub version: Option<String>,
    /// The display in the chosen language.
    pub display: String,
    /// `definition`: the meaning of the concept, when the system states one.
    pub definition: Option<String>,
    /// The designations, when asked for, filtered by the `lang.x` properties
    /// asked for; the display is among them with its language.
    pub designations: Vec<Designation>,
    /// The properties asked for, `definition` among them when the concept has one.
    pub properties: Vec<Property>,
}

/// What the `property` parameters ask for.
struct Asked<'a> {
    /// Every property, designations included: nothing named, or `*`.
    all: bool,
    /// The property codes named.
    names: Vec<&'a str>,
    /// The designation languages named through `lang.X`.
    languages: Vec<&'a str>,
}

impl<'a> Asked<'a> {
    fn from(properties: &'a [String]) -> Self {
        let names: Vec<&str> = properties.iter().map(String::as_str).collect();
        Self {
            all: names.is_empty() || names.contains(&EVERY_PROPERTY),
            languages: names
                .iter()
                .filter_map(|p| p.strip_prefix("lang."))
                .collect(),
            names,
        }
    }

    fn property(&self, code: &str) -> bool {
        self.all || self.names.contains(&code)
    }

    // NOTE: the R4B `property` parameter lists `designation` and `lang.X` among
    // the properties a client asks for, so naming other properties only leaves
    // designations out (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>).
    fn designations(&self) -> bool {
        self.all || self.names.contains(&"designation") || !self.languages.is_empty()
    }

    fn language(&self, designation: &Designation) -> bool {
        self.languages.is_empty()
            || designation
                .language
                .as_deref()
                .is_some_and(|l| self.languages.iter().any(|w| l.eq_ignore_ascii_case(w)))
    }
}

/// Runs `$lookup`.
///
/// # Errors
///
/// Returns [`OperationError`] for a missing or contradictory system or code,
/// an unknown system, version, or code, or a provider failure.
pub fn lookup(
    registry: &Registry,
    invocation: &Invocation,
    input: &LookupInput,
) -> Result<LookupOutcome, OperationError> {
    language::check(input.display_language.as_deref())?;
    if matches!(invocation, Invocation::Instance(_)) {
        return Err(OperationError::NotSupported(String::from(
            "`CodeSystem/$lookup` is declared at the type level only",
        )));
    }
    let (system, version, code) = match (&input.coding, input.code.as_deref()) {
        (Some(_), Some(_)) => {
            return Err(OperationError::Invalid(String::from(
                "provide either `system` and `code` or `coding`, not both",
            )));
        }
        (Some(coding), None) => {
            let code = coding.code.as_deref().ok_or_else(|| {
                OperationError::Required(String::from("`coding.code` is required"))
            })?;
            (
                coding.system.as_deref(),
                coding.version.as_deref().or(input.version.as_deref()),
                code,
            )
        }
        (None, Some(code)) => (input.system.as_deref(), input.version.as_deref(), code),
        (None, None) => {
            return Err(OperationError::Required(String::from(
                "a code is required: `code` with `system`, or `coding`",
            )));
        }
    };
    // NOTE: the R4B definition says "If a code is provided, a system must be
    // provided"; the shared resolver reports the missing system.
    let layered = if input.use_supplement.is_empty() {
        None
    } else {
        Some(registry.with_supplements(&input.use_supplement)?)
    };
    let registry = layered.as_ref().unwrap_or(registry);
    let resolved = resolve(registry, invocation, system, version)?;
    let provider = &resolved.provider;
    let located = locate(provider, code)?;
    let concept = located.concept;
    let identity = provider.identity();
    let language = language::for_provider(provider.as_ref(), input.display_language.as_deref());
    let display = provider
        .display(concept, language.as_deref())?
        .unwrap_or_else(|| located.code.clone());
    let asked = Asked::from(&input.properties);
    let designations = if asked.designations() {
        let mut designations = provider.designations(concept, None)?;
        if !designations.iter().any(|d| d.value == display) {
            designations.push(display_designation(
                provider.as_ref(),
                language.as_deref(),
                &display,
            ));
        }
        designations.retain(|d| asked.language(d));
        designations
    } else {
        Vec::new()
    };
    let properties = properties(provider.as_ref(), concept, &asked)?;
    // NOTE: the ecosystem's icd-11 `lookup-mms-no-code` answers a codeless grouper
    // `notSelectable` as a property and no `abstract`; `simple-lookup-2` wants `abstract`.
    let status = provider.status(concept)?;
    let abstract_concept = status.abstract_concept && !status.codeless;
    Ok(LookupOutcome {
        code: String::from(code),
        system: identity.url.clone(),
        abstract_concept,
        name: identity
            .name
            .clone()
            .or_else(|| identity.title.clone())
            .unwrap_or_else(|| identity.url.clone()),
        version: Some(identity.version.clone()).filter(|v| !v.is_empty()),
        display,
        definition: provider.definition(concept)?,
        designations,
        properties,
    })
}

/// The display as a designation: in the language chosen for it, else the
/// system's own, marked preferred for that language.
fn display_designation(
    provider: &dyn CodeSystemProvider,
    language: Option<&str>,
    display: &str,
) -> Designation {
    let (system, code, use_display) = PREFERRED_FOR_LANGUAGE;
    Designation {
        standards_status: None,
        language: language
            .map(str::to_owned)
            .or_else(|| provider.language().map(str::to_owned)),
        use_: Some(DesignationUse {
            system: String::from(system),
            code: String::from(code),
            display: Some(String::from(use_display)),
        }),
        value: display.to_owned(),
    }
}

/// The properties asked for: `definition` first when the concept has one,
/// then every provider property that is not an output parameter of its own.
fn properties(
    provider: &dyn CodeSystemProvider,
    concept: Concept,
    asked: &Asked<'_>,
) -> Result<Vec<Property>, OperationError> {
    let mut properties = Vec::new();
    if asked.property("definition")
        && let Some(definition) = provider.definition(concept)?
    {
        properties.push(Property {
            code: String::from("definition"),
            value: PropertyValue::String(definition),
            ..Property::default()
        });
    }
    for property in provider.properties(concept)? {
        if NAMED_ELSEWHERE.contains(&property.code.as_str()) || !asked.property(&property.code) {
            continue;
        }
        properties.push(property);
    }
    Ok(properties)
}
