//! The R4B wire of the operations.
//!
//! The generated R4B request becomes the engine's neutral input, and the
//! neutral outcome the generated R4B response, so `/r4b` emits exactly what
//! the 4.3.0 `OperationDefinition`s declare.

use ferroterm_fhir::r4b::codeable_concept::CodeableConcept;
use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operation_outcome::{OperationOutcome, OperationOutcomeIssue};
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
use ferroterm_fhir::r4b::operations::concept_map_translate::{
    ConceptMapTranslateRequest, ConceptMapTranslateResponse, ConceptMapTranslateResponseMatch,
    ConceptMapTranslateResponseMatchProduct,
};
use ferroterm_fhir::r4b::operations::value_set_expand::ValueSetExpandRequest;
use ferroterm_fhir::r4b::operations::value_set_validate_code::{
    ValueSetValidateCodeRequest, ValueSetValidateCodeResponse,
};
use ferroterm_fhir::r4b::parameters::ParametersParameterValue;
use ferroterm_fhir::r4b::parameters::{Parameters, ParametersParameter};
use ferroterm_fhir::r4b::resource::Resource;
use ferroterm_graph::subsumption::Outcome;
use ferroterm_terminology::operations::CodingRef;
use ferroterm_terminology::operations::expand::ExpandInput;
use ferroterm_terminology::operations::lookup::{LookupInput, LookupOutcome};
use ferroterm_terminology::operations::subsumes::SubsumesInput;
use ferroterm_terminology::operations::translate::{TranslateInput, Translation};
use ferroterm_terminology::operations::validate_code::{ValidateCodeInput, ValidationOutcome};
use ferroterm_terminology::operations::value_set_validate_code::{
    TX_ISSUE_TYPE, Validation, ValueSetValidateInput,
};
use ferroterm_terminology::provider::{Designation, PropertyValue};
use ferroterm_terminology::valueset::convert;

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

/// The R4B `Coding` of a `tx-issue-type` code, for `issue.details.coding`.
#[must_use]
pub fn tx_issue_coding(kind: &str) -> Coding {
    Coding {
        system: Some(TX_ISSUE_TYPE.into()),
        code: Some(kind.into()),
        ..Default::default()
    }
}

/// The `ValueSet/$validate-code` request as the engine's input; an inline
/// `valueSet` is converted as an R4B resource.
#[must_use]
pub fn value_set_validate_input(request: &ValueSetValidateCodeRequest) -> ValueSetValidateInput {
    ValueSetValidateInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        value_set_version: request
            .value_set_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_value_set: request.value_set.as_ref().map(convert::r4b::convert),
        context: request.context.is_some(),
        date: request.date.is_some(),
        code: request.code.as_ref().and_then(|v| v.value.clone()),
        system: request.system.as_ref().and_then(|v| v.value.clone()),
        system_version: request
            .system_version
            .as_ref()
            .and_then(|v| v.value.clone()),
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
        abstract_ok: request.r#abstract.as_ref().and_then(|b| b.value),
    }
}

/// The `ValueSet/$validate-code` outcome as R4B `Parameters`.
///
/// The declared `result`, `message`, and `display` come first, then the
/// `system`, `version`, `code`, and `issues` the terminology ecosystem's
/// general-purpose servers return beside them (R5's declared outputs; R4B
/// leaves the resource open).
#[must_use]
pub fn value_set_validation_parameters(validation: &Validation) -> Parameters {
    let response = ValueSetValidateCodeResponse {
        result: validation.result.into(),
        message: validation.message.as_deref().map(Into::into),
        display: validation.display.as_deref().map(Into::into),
    };
    let mut parameters = response.to_parameters();
    let mut push = |name: &str, value: ParametersParameterValue| {
        parameters.parameter.push(ParametersParameter {
            name: name.into(),
            value: Some(value),
            ..Default::default()
        });
    };
    if let Some(system) = &validation.system {
        push(
            "system",
            ParametersParameterValue::Uri(system.as_str().into()),
        );
    }
    if let Some(version) = &validation.version {
        push(
            "version",
            ParametersParameterValue::String(version.as_str().into()),
        );
    }
    if let Some(code) = &validation.code {
        push("code", ParametersParameterValue::Code(code.as_str().into()));
    }
    if !validation.issues.is_empty() {
        let issue = validation
            .issues
            .iter()
            .map(|issue| OperationOutcomeIssue {
                severity: issue.severity.into(),
                code: issue.code.into(),
                details: Some(CodeableConcept {
                    coding: vec![tx_issue_coding(issue.kind)],
                    text: Some(issue.text.as_str().into()),
                    ..Default::default()
                }),
                expression: issue.expression.map(Into::into).into_iter().collect(),
                ..Default::default()
            })
            .collect();
        parameters.parameter.push(ParametersParameter {
            name: "issues".into(),
            resource: Some(Resource::OperationOutcome(Box::new(OperationOutcome {
                issue,
                ..Default::default()
            }))),
            ..Default::default()
        });
    }
    parameters
}

/// A neutral coding as an R4B `Coding`.
fn coding_of(coding: &CodingRef) -> Coding {
    Coding {
        system: coding.system.as_deref().map(Into::into),
        version: coding.version.as_deref().map(Into::into),
        code: coding.code.as_deref().map(Into::into),
        display: coding.display.as_deref().map(Into::into),
        ..Default::default()
    }
}

/// The `$translate` request as the engine's input; an inline `conceptMap` is
/// converted as an R4B resource.
#[must_use]
pub fn translate_input(request: &ConceptMapTranslateRequest) -> TranslateInput {
    TranslateInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        concept_map_version: request
            .concept_map_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_concept_map: request
            .concept_map
            .as_ref()
            .map(ferroterm_terminology::conceptmap::convert::r4b::convert),
        code: request.code.as_ref().and_then(|v| v.value.clone()),
        system: request.system.as_ref().and_then(|v| v.value.clone()),
        version: request.version.as_ref().and_then(|v| v.value.clone()),
        coding: request.coding.as_ref().map(coding_ref),
        codeable_concept: request
            .codeable_concept
            .as_ref()
            .map(|concept| concept.coding.iter().map(coding_ref).collect()),
        source: request.source.as_ref().and_then(|v| v.value.clone()),
        target: request.target.as_ref().and_then(|v| v.value.clone()),
        target_system: request.targetsystem.as_ref().and_then(|v| v.value.clone()),
        reverse: request.reverse.as_ref().and_then(|b| b.value),
        dependency: !request.dependency.is_empty(),
    }
}

/// The `$translate` outcome as R4B `Parameters`.
///
/// `result`, `message`, and each `match` with `equivalence`, `concept`,
/// `product`, and `source`, plus the `originMap`, `sourceConcept`,
/// `sourceComment`, and `noMap` parts the terminology ecosystem's servers add
/// to a match.
#[must_use]
pub fn translation_parameters(translation: &Translation) -> Parameters {
    let response = ConceptMapTranslateResponse {
        result: translation.result.into(),
        message: translation.message.as_deref().map(Into::into),
        r#match: translation
            .matches
            .iter()
            .map(|m| ConceptMapTranslateResponseMatch {
                equivalence: Some(m.relationship.equivalence().into()),
                concept: m.concept.as_ref().map(coding_of),
                product: m
                    .products
                    .iter()
                    .map(|p| ConceptMapTranslateResponseMatchProduct {
                        element: Some(p.element.as_str().into()),
                        concept: Some(coding_of(&p.concept)),
                    })
                    .collect(),
                source: m.source.as_deref().map(Into::into),
            })
            .collect(),
    };
    let mut parameters = response.to_parameters();
    let mut origins = translation.matches.iter().map(|m| &m.origin);
    for parameter in &mut parameters.parameter {
        if parameter.name.value.as_deref() != Some("match") {
            continue;
        }
        let Some(origin) = origins.next() else { break };
        let mut part = |name: &str, value: ParametersParameterValue| {
            parameter.part.push(ParametersParameter {
                name: name.into(),
                value: Some(value),
                ..Default::default()
            });
        };
        part(
            "originMap",
            ParametersParameterValue::Canonical(origin.origin_map.as_str().into()),
        );
        if let Some(concept) = &origin.source_concept {
            part(
                "sourceConcept",
                ParametersParameterValue::Coding(coding_of(concept)),
            );
        }
        if let Some(comment) = &origin.source_comment {
            part(
                "sourceComment",
                ParametersParameterValue::String(comment.as_str().into()),
            );
        }
        if origin.no_map {
            part("noMap", ParametersParameterValue::Boolean(true.into()));
        }
    }
    parameters
}

/// The canonicals of a repeated `canonical` parameter, as text.
fn canonicals(list: &[ferroterm_fhir::r4b::primitives::Canonical]) -> Vec<String> {
    list.iter().filter_map(|c| c.value.clone()).collect()
}

/// The `$expand` request as the engine's input; an inline `valueSet` is
/// converted as an R4B resource. R4B declares no `property` or
/// `useSupplement`, so those stay empty.
#[must_use]
pub fn expand_input(request: &ValueSetExpandRequest) -> ExpandInput {
    let flag =
        |b: &Option<ferroterm_fhir::r4b::primitives::Boolean>| b.as_ref().and_then(|b| b.value);
    let number = |i: &Option<ferroterm_fhir::r4b::primitives::Integer>| {
        i.as_ref().and_then(|i| i.value).map(i64::from)
    };
    ExpandInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        value_set_version: request
            .value_set_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_value_set: request.value_set.as_ref().map(convert::r4b::convert),
        context: request.context.is_some() || request.context_direction.is_some(),
        date: request.date.is_some(),
        filter: request.filter.as_ref().and_then(|v| v.value.clone()),
        offset: number(&request.offset),
        count: number(&request.count),
        include_designations: flag(&request.include_designations),
        designation: request
            .designation
            .iter()
            .filter_map(|d| d.value.clone())
            .collect(),
        include_definition: flag(&request.include_definition),
        active_only: flag(&request.active_only),
        exclude_nested: flag(&request.exclude_nested),
        exclude_not_for_ui: flag(&request.exclude_not_for_u_i),
        exclude_post_coordinated: flag(&request.exclude_post_coordinated),
        display_language: request
            .display_language
            .as_ref()
            .and_then(|v| v.value.clone()),
        exclude_system: canonicals(&request.exclude_system),
        system_version: canonicals(&request.system_version),
        check_system_version: canonicals(&request.check_system_version),
        force_system_version: canonicals(&request.force_system_version),
        property: Vec::new(),
        use_supplement: Vec::new(),
    }
}
