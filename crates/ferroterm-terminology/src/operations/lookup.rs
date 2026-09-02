//! `CodeSystem/$lookup` on R4B
//! (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>).
//!
//! Type-level only: the R4B definition declares `instance = false`. The
//! client provides a system and a code, as `system` + `code` or as `coding`.
//! The response carries `name`, the served `version`, the `display` for the
//! requested language, every designation (language stated), and the
//! properties: all the provider declares when `property` is empty, else the
//! requested ones.

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::code_system_lookup::{
    CodeSystemLookupRequest, CodeSystemLookupResponse, CodeSystemLookupResponseDesignation,
    CodeSystemLookupResponseProperty,
};
use ferroterm_fhir::r4b::parameters::ParametersParameterValue;

use super::{
    Invocation, OperationError, code_text, coding_parts, locate, resolve, string_text, uri_text,
};
use crate::provider::{Designation, PropertyValue};
use crate::registry::Registry;

/// The property names R4B lists for every code system, answered outside the
/// `property` group by the named out parameters or by `designation`
/// (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>, `property`).
const NAMED_ELSEWHERE: [&str; 6] = ["url", "system", "name", "version", "display", "designation"];

/// Runs `$lookup`.
///
/// # Errors
///
/// Returns [`OperationError`] for a missing or contradictory system or code,
/// an unknown system, version, or code, or a provider failure.
pub fn lookup(
    registry: &Registry,
    invocation: &Invocation,
    request: &CodeSystemLookupRequest,
) -> Result<CodeSystemLookupResponse, OperationError> {
    if matches!(invocation, Invocation::Instance(_)) {
        return Err(OperationError::NotSupported(String::from(
            "R4B declares CodeSystem/$lookup at the type level only",
        )));
    }
    let (system, version, code) = match (&request.coding, code_text(request.code.as_ref())) {
        (Some(_), Some(_)) => {
            return Err(OperationError::Invalid(String::from(
                "provide either `system` and `code` or `coding`, not both",
            )));
        }
        (Some(coding), None) => {
            let (system, version, code, _) = coding_parts(coding);
            let code = code.ok_or_else(|| {
                OperationError::Required(String::from("`coding.code` is required"))
            })?;
            (
                system,
                version.or(string_text(request.version.as_ref())),
                code,
            )
        }
        (None, Some(code)) => (
            uri_text(request.system.as_ref()),
            string_text(request.version.as_ref()),
            code,
        ),
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
    let language = code_text(request.display_language.as_ref());
    // NOTE: `display` is 1..1; a concept without any designation is displayed
    // by its code (no spec says what to show then: our own choice).
    let display = provider
        .display(concept, language)?
        .unwrap_or_else(|| located.code.clone());
    let wanted: Vec<&str> = request
        .property
        .iter()
        .filter_map(|p| p.value.as_deref())
        .collect();
    let languages: Vec<&str> = wanted
        .iter()
        .filter_map(|p| p.strip_prefix("lang."))
        .collect();
    let designations: Vec<CodeSystemLookupResponseDesignation> = provider
        .designations(concept, None)?
        .into_iter()
        .filter(|d| {
            languages.is_empty()
                || d.language
                    .as_deref()
                    .is_some_and(|l| languages.iter().any(|w| l.eq_ignore_ascii_case(w)))
        })
        .map(designation)
        .collect();
    let mut properties = Vec::new();
    let all = wanted.is_empty();
    let asked = |name: &str| all || wanted.contains(&name);
    if asked("definition")
        && let Some(definition) = provider.definition(concept)?
    {
        properties.push(CodeSystemLookupResponseProperty {
            code: "definition".into(),
            value: Some(ParametersParameterValue::String(definition.into())),
            description: None,
            subproperty: Vec::new(),
        });
    }
    for property in provider.properties(concept)? {
        if NAMED_ELSEWHERE.contains(&property.code.as_str()) || !asked(&property.code) {
            continue;
        }
        properties.push(CodeSystemLookupResponseProperty {
            code: property.code.as_str().into(),
            value: Some(value(&property.value)),
            description: None,
            subproperty: Vec::new(),
        });
    }
    Ok(CodeSystemLookupResponse {
        name: identity
            .title
            .clone()
            .unwrap_or_else(|| identity.url.clone())
            .into(),
        version: Some(identity.version.as_str().into()),
        display: display.into(),
        designation: designations,
        property: properties,
    })
}

fn designation(d: Designation) -> CodeSystemLookupResponseDesignation {
    CodeSystemLookupResponseDesignation {
        language: d.language.map(|l| l.as_str().into()),
        r#use: d.use_.map(|u| Coding {
            system: Some(u.system.as_str().into()),
            code: Some(u.code.as_str().into()),
            display: u.display.map(|d| d.as_str().into()),
            ..Default::default()
        }),
        value: d.value.into(),
    }
}

/// A property value as the `value[x]` the R4B definition admits
/// (`code`, `Coding`, `string`, `integer`, `boolean`, `dateTime`, `decimal`).
pub(crate) fn value(value: &PropertyValue) -> ParametersParameterValue {
    match value {
        PropertyValue::Code(c) => ParametersParameterValue::Code(c.as_str().into()),
        PropertyValue::Coding {
            system,
            code,
            display,
        } => ParametersParameterValue::Coding(Coding {
            system: Some(system.as_str().into()),
            code: Some(code.as_str().into()),
            display: display.as_ref().map(|d| d.as_str().into()),
            ..Default::default()
        }),
        PropertyValue::String(s) => ParametersParameterValue::String(s.as_str().into()),
        // NOTE: the R4B `integer` is 32-bit; a wider value is carried as a
        // string rather than truncated (no spec covers the overflow).
        PropertyValue::Integer(i) => match i32::try_from(*i) {
            Ok(i) => ParametersParameterValue::Integer(i.into()),
            Err(_) => ParametersParameterValue::String(i.to_string().into()),
        },
        PropertyValue::Boolean(b) => ParametersParameterValue::Boolean((*b).into()),
        PropertyValue::DateTime(d) => ParametersParameterValue::DateTime(d.as_str().into()),
        PropertyValue::Decimal(d) => ParametersParameterValue::Decimal(d.as_str().into()),
    }
}
