//! The R4B wire of the operations.
//!
//! The generated R4B request becomes the engine's neutral input, and the
//! neutral outcome the generated R4B response, so `/r4b` emits exactly what
//! the 4.3.0 `OperationDefinition`s declare.

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::code_system_lookup::{
    CodeSystemLookupRequest, CodeSystemLookupResponse, CodeSystemLookupResponseDesignation,
    CodeSystemLookupResponseProperty, CodeSystemLookupResponsePropertySubproperty,
};
use ferroterm_fhir::r4b::operations::code_system_subsumes::{
    CodeSystemSubsumesRequest, CodeSystemSubsumesResponse,
};
use ferroterm_fhir::r4b::operations::code_system_validate_code::{
    CodeSystemValidateCodeRequest, CodeSystemValidateCodeResponse,
};
use ferroterm_fhir::r4b::parameters::ParametersParameterValue;
use ferroterm_graph::subsumption::Outcome;
use ferroterm_terminology::operations::CodingRef;
use ferroterm_terminology::operations::lookup::{LookupInput, LookupOutcome};
use ferroterm_terminology::operations::subsumes::SubsumesInput;
use ferroterm_terminology::operations::validate_code::{ValidateCodeInput, ValidationOutcome};
use ferroterm_terminology::provider::{Designation, PropertyValue};

/// A generated `Coding` as the engine names one.
#[must_use]
pub fn coding_ref(coding: &Coding) -> CodingRef {
    CodingRef {
        system: coding.system.as_ref().and_then(|v| v.value.clone()),
        version: coding.version.as_ref().and_then(|v| v.value.clone()),
        code: coding.code.as_ref().and_then(|v| v.value.clone()),
        display: coding.display.as_ref().and_then(|v| v.value.clone()),
    }
}

/// The `$lookup` request as the engine's input.
#[must_use]
pub fn lookup_input(request: &CodeSystemLookupRequest) -> LookupInput {
    LookupInput {
        code: request.code.as_ref().and_then(|v| v.value.clone()),
        system: request.system.as_ref().and_then(|v| v.value.clone()),
        version: request.version.as_ref().and_then(|v| v.value.clone()),
        coding: request.coding.as_ref().map(coding_ref),
        display_language: request
            .display_language
            .as_ref()
            .and_then(|v| v.value.clone()),
        properties: request
            .property
            .iter()
            .filter_map(|p| p.value.clone())
            .collect(),
        use_supplement: Vec::new(),
    }
}

/// The `$lookup` outcome as the R4B response: `name`, `version`, `display`,
/// `designation`, and `property` with its `subproperty` parts.
#[must_use]
pub fn lookup_response(outcome: LookupOutcome) -> CodeSystemLookupResponse {
    CodeSystemLookupResponse {
        name: outcome.name.into(),
        version: outcome.version.map(Into::into),
        display: outcome.display.into(),
        designation: outcome.designations.into_iter().map(designation).collect(),
        property: outcome
            .properties
            .into_iter()
            .map(|property| CodeSystemLookupResponseProperty {
                code: property.code.into(),
                value: Some(parameter_value(&property.value)),
                description: property.description.map(Into::into),
                subproperty: property
                    .subproperties
                    .iter()
                    .map(|part| CodeSystemLookupResponsePropertySubproperty {
                        code: part.code.as_str().into(),
                        value: parameter_value(&part.value),
                        description: part.description.as_deref().map(Into::into),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn designation(d: Designation) -> CodeSystemLookupResponseDesignation {
    CodeSystemLookupResponseDesignation {
        language: d.language.map(Into::into),
        r#use: d.use_.map(|u| Coding {
            system: Some(u.system.into()),
            code: Some(u.code.into()),
            display: u.display.map(Into::into),
            ..Default::default()
        }),
        value: d.value.into(),
    }
}

/// A property value as a `Parameters.parameter.value[x]`.
#[must_use]
pub fn parameter_value(value: &PropertyValue) -> ParametersParameterValue {
    match value {
        PropertyValue::Code(c) => ParametersParameterValue::Code(c.as_str().into()),
        PropertyValue::Uri(u) => ParametersParameterValue::Uri(u.as_str().into()),
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
        PropertyValue::Integer(i) => match i32::try_from(*i) {
            Ok(i) => ParametersParameterValue::Integer(i.into()),
            Err(_) => ParametersParameterValue::String(i.to_string().into()),
        },
        PropertyValue::Boolean(b) => ParametersParameterValue::Boolean((*b).into()),
        PropertyValue::DateTime(d) => ParametersParameterValue::DateTime(d.as_str().into()),
        PropertyValue::Decimal(d) => ParametersParameterValue::Decimal(d.as_str().into()),
    }
}

/// The `$subsumes` request as the engine's input.
#[must_use]
pub fn subsumes_input(request: &CodeSystemSubsumesRequest) -> SubsumesInput {
    SubsumesInput {
        code_a: request.code_a.as_ref().and_then(|v| v.value.clone()),
        code_b: request.code_b.as_ref().and_then(|v| v.value.clone()),
        system: request.system.as_ref().and_then(|v| v.value.clone()),
        version: request.version.as_ref().and_then(|v| v.value.clone()),
        coding_a: request.coding_a.as_ref().map(coding_ref),
        coding_b: request.coding_b.as_ref().map(coding_ref),
    }
}

/// The `$subsumes` outcome as the R4B response.
#[must_use]
pub fn subsumes_response(outcome: Outcome) -> CodeSystemSubsumesResponse {
    CodeSystemSubsumesResponse {
        outcome: outcome.code().into(),
    }
}

/// The `CodeSystem/$validate-code` request as the engine's input.
#[must_use]
pub fn validate_code_input(request: &CodeSystemValidateCodeRequest) -> ValidateCodeInput {
    ValidateCodeInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        version: request.version.as_ref().and_then(|v| v.value.clone()),
        inline_code_system: request.code_system.is_some(),
        code: request.code.as_ref().and_then(|v| v.value.clone()),
        display: request.display.as_ref().and_then(|v| v.value.clone()),
        coding: request.coding.as_ref().map(coding_ref),
        codeable_concept: request
            .codeable_concept
            .as_ref()
            .map(|concept| concept.coding.iter().map(coding_ref).collect()),
        display_language: request
            .display_language
            .as_ref()
            .and_then(|v| v.value.clone()),
    }
}

/// The `CodeSystem/$validate-code` outcome as the R4B response: `result`,
/// `message`, and `display` (4.3.0 declares no `code`, `system`, or
/// `version` output).
#[must_use]
pub fn validate_code_response(outcome: ValidationOutcome) -> CodeSystemValidateCodeResponse {
    CodeSystemValidateCodeResponse {
        result: outcome.result.into(),
        message: outcome.message.map(Into::into),
        display: outcome.display.map(Into::into),
    }
}
