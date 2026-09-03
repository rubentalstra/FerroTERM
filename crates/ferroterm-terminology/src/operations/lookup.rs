//! `CodeSystem/$lookup` in the terms every served FHIR version shares.
//!
//! The input is the union of the parameters the versions declare
//! (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>,
//! <https://hl7.org/fhir/R5/codesystem-operation-lookup.html>), the outcome the
//! provider's own designations and properties; the wire layer of each version
//! maps them into that version's generated contract.

use super::{CodingRef, Invocation, OperationError, locate, resolve};
use crate::language;
use crate::provider::{CodeSystemProvider, Concept, Designation, Property, PropertyValue};
use crate::registry::Registry;

/// The `$lookup` parameters `property` never repeats: they are output
/// parameters of their own.
const NAMED_ELSEWHERE: [&str; 6] = ["url", "system", "name", "version", "display", "designation"];

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
    /// The properties asked for; empty is every one.
    pub properties: Vec<String>,
    /// The supplements to apply (R5 `useSupplement`), by canonical.
    pub use_supplement: Vec<String>,
}

/// The outcome of `$lookup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupOutcome {
    /// The code system's name (its title, else its URI).
    pub name: String,
    /// The version served.
    pub version: Option<String>,
    /// The display in the chosen language.
    pub display: String,
    /// The designations, filtered by the `lang.x` properties asked for.
    pub designations: Vec<Designation>,
    /// The properties asked for, `definition` among them when the concept has one.
    pub properties: Vec<Property>,
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
    let resolved = resolve(registry, invocation, system, version)?;
    let provider = &resolved.provider;
    let located = locate(provider, code)?;
    let concept = located.concept;
    let identity = provider.identity();
    let language = language::for_provider(provider.as_ref(), input.display_language.as_deref());
    let display = provider
        .display(concept, language.as_deref())?
        .unwrap_or_else(|| located.code.clone());
    let wanted: Vec<&str> = input.properties.iter().map(String::as_str).collect();
    let languages: Vec<&str> = wanted
        .iter()
        .filter_map(|p| p.strip_prefix("lang."))
        .collect();
    let designations = provider
        .designations(concept, None)?
        .into_iter()
        .filter(|d| {
            languages.is_empty()
                || d.language
                    .as_deref()
                    .is_some_and(|l| languages.iter().any(|w| l.eq_ignore_ascii_case(w)))
        })
        .collect();
    let properties = properties(provider.as_ref(), concept, &wanted)?;
    Ok(LookupOutcome {
        name: identity
            .title
            .clone()
            .unwrap_or_else(|| identity.url.clone()),
        version: Some(identity.version.clone()),
        display,
        designations,
        properties,
    })
}

/// The properties asked for: `definition` first when the concept has one,
/// then every provider property that is not an output parameter of its own.
fn properties(
    provider: &dyn CodeSystemProvider,
    concept: Concept,
    wanted: &[&str],
) -> Result<Vec<Property>, OperationError> {
    let mut properties = Vec::new();
    let all = wanted.is_empty();
    let asked = |name: &str| all || wanted.contains(&name);
    if asked("definition")
        && let Some(definition) = provider.definition(concept)?
    {
        properties.push(Property {
            code: String::from("definition"),
            value: PropertyValue::String(definition),
            ..Property::default()
        });
    }
    for property in provider.properties(concept)? {
        if NAMED_ELSEWHERE.contains(&property.code.as_str()) || !asked(&property.code) {
            continue;
        }
        properties.push(property);
    }
    Ok(properties)
}
