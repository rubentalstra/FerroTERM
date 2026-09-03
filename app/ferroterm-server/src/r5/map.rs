//! The R5 wire of the operations.
//!
//! The generated 5.0.0 request becomes the engine's neutral input, and the
//! neutral outcome the generated 5.0.0 response, so `/r5` emits exactly what
//! the R5 `OperationDefinition`s declare
//! (<https://hl7.org/fhir/R5/terminology-service.html>).

use concept_graph::subsumption::Outcome;
use fhir_terminology::operations::expand::ExpandInput;
use fhir_terminology::operations::lookup::{LookupInput, LookupOutcome};
use fhir_terminology::operations::subsumes::SubsumesInput;
use fhir_terminology::operations::translate::{TranslateInput, Translation};
use fhir_terminology::operations::validate_code::{ValidateCodeInput, ValidationOutcome};
use fhir_terminology::operations::value_set_validate_code::{
    TX_ISSUE_TYPE, Validation, ValueSetValidateInput,
};
use fhir_terminology::operations::{CodingRef, Issue};
use fhir_terminology::provider::{Designation, PropertyValue};
use fhir_terminology::{conceptmap, valueset};
use fhir_types::r5::codeable_concept::CodeableConcept;
use fhir_types::r5::coding::Coding;
use fhir_types::r5::operation_outcome::{OperationOutcome, OperationOutcomeIssue};
use fhir_types::r5::operations::code_system_lookup::{
    CodeSystemLookupRequest, CodeSystemLookupResponse, CodeSystemLookupResponseDesignation,
    CodeSystemLookupResponseProperty, CodeSystemLookupResponsePropertySubproperty,
};
use fhir_types::r5::operations::code_system_subsumes::{
    CodeSystemSubsumesRequest, CodeSystemSubsumesResponse,
};
use fhir_types::r5::operations::code_system_validate_code::{
    CodeSystemValidateCodeRequest, CodeSystemValidateCodeResponse,
};
use fhir_types::r5::operations::concept_map_translate::{
    ConceptMapTranslateRequest, ConceptMapTranslateResponse, ConceptMapTranslateResponseMatch,
    ConceptMapTranslateResponseMatchProduct,
};
use fhir_types::r5::operations::value_set_expand::ValueSetExpandRequest;
use fhir_types::r5::operations::value_set_validate_code::{
    ValueSetValidateCodeRequest, ValueSetValidateCodeResponse,
};
use fhir_types::r5::parameters::{Parameters, ParametersParameterValue};
use fhir_types::r5::primitives::{Boolean, Canonical, Integer};

/// A generated `Coding` as the engine's coding.
#[must_use]
pub fn coding_ref(coding: &Coding) -> CodingRef {
    CodingRef {
        system: coding.system.as_ref().and_then(|v| v.value.clone()),
        version: coding.version.as_ref().and_then(|v| v.value.clone()),
        code: coding.code.as_ref().and_then(|v| v.value.clone()),
        display: coding.display.as_ref().and_then(|v| v.value.clone()),
    }
}

fn canonicals(list: &[Canonical]) -> Vec<String> {
    list.iter().filter_map(|c| c.value.clone()).collect()
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
        use_supplement: canonicals(&request.use_supplement),
    }
}

/// The `$lookup` outcome as the R5 response: `name`, `version`, `display`,
/// `definition`, `designation`, and `property` with its `subproperty` parts.
#[must_use]
pub fn lookup_response(outcome: LookupOutcome) -> CodeSystemLookupResponse {
    CodeSystemLookupResponse {
        name: outcome.name.into(),
        version: outcome.version.map(Into::into),
        display: outcome.display.into(),
        definition: outcome.definition.map(Into::into),
        designation: outcome.designations.into_iter().map(designation).collect(),
        // NOTE: R5 answers `definition` as its own output parameter, and a property
        // with a named parameter's name is returned there, not in `property`
        // (<https://hl7.org/fhir/R5/codesystem-operation-lookup.html>).
        property: outcome
            .properties
            .into_iter()
            .filter(|property| property.code != "definition")
            .map(|property| CodeSystemLookupResponseProperty {
                code: property.code.into(),
                value: Some(parameter_value(&property.value)),
                description: property.description.map(Into::into),
                source: None,
                subproperty: property
                    .subproperties
                    .iter()
                    .map(|part| CodeSystemLookupResponsePropertySubproperty {
                        code: part.code.as_str().into(),
                        value: parameter_value(&part.value),
                        description: part.description.as_deref().map(Into::into),
                        source: None,
                    })
                    .collect(),
            })
            .collect(),
        // TODO(#163): answer the located code, its system, and abstract.
        code: None,
        system: None,
        r#abstract: None,
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
        additional_use: Vec::new(),
        value: d.value.into(),
    }
}

/// A property value as a `Parameters` value.
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

/// The `$subsumes` outcome as the R5 response.
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

/// The `CodeSystem/$validate-code` outcome as the R5 response: `result`,
/// `message`, `display`, the validated `code`, `system`, and `version`, and
/// the itemised `issues`.
#[must_use]
pub fn validate_code_response(outcome: ValidationOutcome) -> CodeSystemValidateCodeResponse {
    CodeSystemValidateCodeResponse {
        result: outcome.result.into(),
        message: outcome.message.map(Into::into),
        display: outcome.display.map(Into::into),
        code: outcome.code.map(Into::into),
        system: outcome.system.map(Into::into),
        version: outcome.version.map(Into::into),
        codeable_concept: None,
        issues: issues(&outcome.issues),
        x_caused_by_unknown_system: outcome
            .unknown_systems
            .into_iter()
            .map(Into::into)
            .collect(),
    }
}

/// The R5 `Coding` of a `tx-issue-type` code, for `issue.details.coding`.
#[must_use]
pub fn tx_issue_coding(kind: &str) -> Coding {
    Coding {
        system: Some(TX_ISSUE_TYPE.into()),
        code: Some(kind.into()),
        ..Default::default()
    }
}

/// The itemised issues as the `issues` `OperationOutcome`; none when empty.
fn issues(list: &[Issue]) -> Option<OperationOutcome> {
    if list.is_empty() {
        return None;
    }
    Some(OperationOutcome {
        issue: list
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
            .collect(),
        ..Default::default()
    })
}

/// The `ValueSet/$validate-code` request as the engine's input; an inline
/// `valueSet` is converted as an R5 resource.
#[must_use]
pub fn value_set_validate_input(request: &ValueSetValidateCodeRequest) -> ValueSetValidateInput {
    ValueSetValidateInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        value_set_version: request
            .value_set_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_value_set: request
            .value_set
            .as_ref()
            .map(valueset::convert::r5::convert),
        use_supplement: canonicals(&request.use_supplement),
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
        default_system_version: canonicals(&request.system_version_canonical),
        check_system_version: canonicals(&request.check_system_version),
        force_system_version: canonicals(&request.force_system_version),
        default_valueset_version: canonicals(&request.default_valueset_version),
        check_valueset_version: canonicals(&request.check_valueset_version),
        force_valueset_version: canonicals(&request.force_valueset_version),
        infer_system: request.infer_system.as_ref().and_then(|b| b.value),
        lenient_display_validation: request
            .lenient_display_validation
            .as_ref()
            .and_then(|b| b.value),
    }
}

/// The `ValueSet/$validate-code` outcome as the R5 `Parameters`: `result`,
/// `message`, `display`, the validated `code`, `system`, and `version`, and
/// the itemised `issues`.
#[must_use]
pub fn value_set_validation_parameters(validation: &Validation) -> Parameters {
    ValueSetValidateCodeResponse {
        result: validation.result.into(),
        message: validation.message.as_deref().map(Into::into),
        display: validation.display.as_deref().map(Into::into),
        code: validation.code.as_deref().map(Into::into),
        system: validation.system.as_deref().map(Into::into),
        version: validation.version.as_deref().map(Into::into),
        codeable_concept: None,
        issues: issues(&validation.issues),
        x_caused_by_unknown_system: validation
            .unknown_systems
            .iter()
            .map(|s| s.as_str().into())
            .collect(),
    }
    .to_parameters()
}

/// A neutral coding as an R5 `Coding`.
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
/// converted as an R5 resource.
///
/// R5 renames the R4 inputs (`sourceCode`, `sourceScope`, `sourceCoding`,
/// `sourceCodeableConcept`, `targetScope`, `targetSystem`) and replaces
/// `reverse` with `targetCode`: a `targetCode` is the code to translate in
/// reverse (<https://hl7.org/fhir/R5/conceptmap-operation-translate.html>).
#[must_use]
pub fn translate_input(request: &ConceptMapTranslateRequest) -> TranslateInput {
    let target_code = request.target_code.as_ref().and_then(|v| v.value.clone());
    TranslateInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        concept_map_version: request
            .concept_map_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_concept_map: request
            .concept_map
            .as_ref()
            .map(conceptmap::convert::r5::convert),
        code: request
            .source_code
            .as_ref()
            .and_then(|v| v.value.clone())
            .or_else(|| target_code.clone()),
        system: request
            .system
            .as_ref()
            .or(request.source_system.as_ref())
            .and_then(|v| v.value.clone()),
        version: request
            .version
            .as_ref()
            .or(request.source_version.as_ref())
            .and_then(|v| v.value.clone()),
        coding: request.source_coding.as_ref().map(coding_ref),
        codeable_concept: request
            .source_codeable_concept
            .as_ref()
            .map(|concept| concept.coding.iter().map(coding_ref).collect()),
        source: request.source_scope.as_ref().and_then(|v| v.value.clone()),
        target: request.target_scope.as_ref().and_then(|v| v.value.clone()),
        target_system: request.target_system.as_ref().and_then(|v| v.value.clone()),
        reverse: target_code.is_some().then_some(true),
        dependency: !request.dependency.is_empty(),
    }
}

/// The `$translate` outcome as the R5 `Parameters`: `result`, `message`, and
/// each `match` with `relationship`, `concept`, `product` (an `attribute` and
/// a `value`), and `originMap`.
#[must_use]
pub fn translation_parameters(translation: &Translation) -> Parameters {
    ConceptMapTranslateResponse {
        result: translation.result.into(),
        message: translation.message.as_deref().map(Into::into),
        r#match: translation
            .matches
            .iter()
            .map(|m| ConceptMapTranslateResponseMatch {
                relationship: Some(m.relationship.relationship().into()),
                concept: m.concept.as_ref().map(coding_of),
                property: Vec::new(),
                product: m
                    .products
                    .iter()
                    .map(|p| ConceptMapTranslateResponseMatchProduct {
                        attribute: p.element.as_str().into(),
                        value: ParametersParameterValue::Coding(coding_of(&p.concept)),
                    })
                    .collect(),
                depends_on: Vec::new(),
                origin_map: Some(m.origin.origin_map.as_str().into()),
            })
            .collect(),
    }
    .to_parameters()
}

/// The `$expand` request as the engine's input; an inline `valueSet` is
/// converted as an R5 resource.
#[must_use]
pub fn expand_input(request: &ValueSetExpandRequest) -> ExpandInput {
    let flag = |b: &Option<Boolean>| b.as_ref().and_then(|b| b.value);
    let number = |i: &Option<Integer>| i.as_ref().and_then(|i| i.value).map(i64::from);
    ExpandInput {
        url: request.url.as_ref().and_then(|v| v.value.clone()),
        value_set_version: request
            .value_set_version
            .as_ref()
            .and_then(|v| v.value.clone()),
        inline_value_set: request
            .value_set
            .as_ref()
            .map(valueset::convert::r5::convert),
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
        property: request
            .property
            .iter()
            .filter_map(|p| p.value.clone())
            .collect(),
        use_supplement: canonicals(&request.use_supplement),
    }
}
