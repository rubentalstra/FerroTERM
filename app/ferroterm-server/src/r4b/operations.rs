//! The `CodeSystem`, `ValueSet`, and `ConceptMap` operation handlers on R4B.
//!
//! Each handler turns the wire form into the generated request type, runs the
//! operation in the request's [`Scope`], and writes the generated response.
//! `tx-resource` parameters and the `X-Cache-Id` header are peeled off before
//! the generated request refuses what the operation does not declare.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path as UrlPath, Query, State};
use axum::response::{IntoResponse, Response};
use ferroterm_fhir::r4b::codeable_concept::CodeableConcept;
use ferroterm_fhir::r4b::operation_outcome::{OperationOutcome, OperationOutcomeIssue};
use ferroterm_fhir::r4b::operations::code_system_lookup::{
    CODE_SYSTEM_LOOKUP, CodeSystemLookupRequest,
};
use ferroterm_fhir::r4b::operations::code_system_subsumes::{
    CODE_SYSTEM_SUBSUMES, CodeSystemSubsumesRequest,
};
use ferroterm_fhir::r4b::operations::code_system_validate_code::{
    CODE_SYSTEM_VALIDATE_CODE, CodeSystemValidateCodeRequest,
};
use ferroterm_fhir::r4b::operations::concept_map_translate::{
    CONCEPT_MAP_TRANSLATE, ConceptMapTranslateRequest,
};
use ferroterm_fhir::r4b::operations::value_set_expand::{VALUE_SET_EXPAND, ValueSetExpandRequest};
use ferroterm_fhir::r4b::operations::value_set_validate_code::{
    VALUE_SET_VALIDATE_CODE, ValueSetValidateCodeRequest,
};
use ferroterm_fhir::r4b::parameters::{Parameters, ParametersParameter, ParametersParameterValue};
use ferroterm_fhir::r4b::resource::Resource;
use ferroterm_terminology::operations::translate::{self, Translation};
use ferroterm_terminology::operations::value_set_validate_code::{Validation, tx_issue_coding};
use ferroterm_terminology::operations::{
    Invocation, expand, lookup, subsumes, validate_code, value_set_validate_code,
};
use http::{HeaderMap, StatusCode};

use crate::outcome::Failure;
use crate::r4b::{map, wire};
use crate::scope::{Scope, scope_of, split_resources};
use crate::state::AppState;

type Handled = Result<Response, Failure>;

fn instance(state: &AppState, id: &str) -> Result<Invocation, Failure> {
    state.instance(id).map(Invocation::Instance).ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "not-found",
            format!("no CodeSystem with id `{id}`"),
        )
    })
}

/// A `GET` invocation: the query parameters, in the scope the headers name.
fn from_query<'a>(
    state: &'a AppState,
    operation: &ferroterm_fhir::operation::Operation,
    headers: &HeaderMap,
    query: &[(String, String)],
) -> Result<(Scope<'a>, Parameters), Failure> {
    let own: Vec<(String, String)> = query
        .iter()
        .filter(|(name, _)| name != crate::scope::UUID)
        .cloned()
        .collect();
    let mut parameters = wire::parameters_from_query(operation, &own)?;
    wire::apply_accept_language(operation, headers, &mut parameters);
    Ok((scope_of(state, headers, Vec::new())?, parameters))
}

/// A `POST` invocation: the body's `Parameters` less its `tx-resource`s, in
/// the scope those resources and the headers form.
fn from_body<'a>(
    state: &'a AppState,
    operation: &ferroterm_fhir::operation::Operation,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<(Scope<'a>, Parameters), Failure> {
    let (mut parameters, resources) = split_resources(wire::parameters_from_body(headers, body)?)?;
    wire::apply_accept_language(operation, headers, &mut parameters);
    Ok((scope_of(state, headers, resources)?, parameters))
}

fn run_lookup(scope: &Scope<'_>, invocation: &Invocation, parameters: &Parameters) -> Handled {
    let request = CodeSystemLookupRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let outcome = lookup::lookup(scope.registry(), invocation, &map::lookup_input(&request))?;
    wire::respond(&map::lookup_response(outcome).to_parameters())
}

fn run_validate_code(
    scope: &Scope<'_>,
    invocation: &Invocation,
    parameters: &Parameters,
) -> Handled {
    let request = CodeSystemValidateCodeRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = validate_code::validate_code(scope.registry(), invocation, &request)?;
    wire::respond(&response.to_parameters())
}

fn run_subsumes(scope: &Scope<'_>, invocation: &Invocation, parameters: &Parameters) -> Handled {
    let request = CodeSystemSubsumesRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let outcome = subsumes::subsumes(scope.registry(), invocation, &map::subsumes_input(&request))?;
    wire::respond(&map::subsumes_response(outcome).to_parameters())
}

fn run_expand(scope: &Scope<'_>, parameters: &Parameters) -> Handled {
    let request = ValueSetExpandRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = expand::expand(&scope.sources(), &request)?;
    wire::respond_resource(&response.r#return)
}

fn run_value_set_validate_code(scope: &Scope<'_>, parameters: &Parameters) -> Handled {
    let request = ValueSetValidateCodeRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let validation = value_set_validate_code::validate_code(&scope.sources(), &request)?;
    wire::respond(&validation_parameters(&validation))
}

/// The R4B response plus the outputs a general-purpose terminology server
/// returns beside it.
///
/// `system`, `version`, `code`, and `issues` are R5 output parameters and
/// ecosystem requirements the R4B definition does not declare; they are
/// appended deliberately for validators that read them
/// (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
fn validation_parameters(validation: &Validation) -> Parameters {
    let mut parameters = validation.response.to_parameters();
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

fn run_translate(scope: &Scope<'_>, parameters: &Parameters) -> Handled {
    let request = ConceptMapTranslateRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let translation = translate::translate(&scope.sources(), &request)?;
    wire::respond(&translation_parameters(&translation))
}

/// The R4B response with, per `match`, the parts a general-purpose
/// terminology server adds: `originMap`, `sourceConcept`, `sourceComment`,
/// and `noMap`. Ecosystem outputs beside the declared ones, appended
/// deliberately (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
fn translation_parameters(translation: &Translation) -> Parameters {
    let mut parameters = translation.response.to_parameters();
    let mut origins = translation.origins.iter();
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
                ParametersParameterValue::Coding(concept.clone()),
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

fn finish(handled: Handled) -> Response {
    match handled {
        Ok(response) => response,
        Err(failure) => failure.into_response(),
    }
}

/// `GET /CodeSystem/$lookup`.
pub async fn lookup_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &CODE_SYSTEM_LOOKUP, &headers, &query)
            .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$lookup`.
pub async fn lookup_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &CODE_SYSTEM_LOOKUP, &headers, &body)
            .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/$validate-code`.
pub async fn validate_code_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &query)
            .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$validate-code`.
pub async fn validate_code_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &body)
            .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/{id}/$validate-code`.
pub async fn validate_code_instance_get(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        from_query(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &query)
            .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p))
    }))
}

/// `POST /CodeSystem/{id}/$validate-code`.
pub async fn validate_code_instance_post(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        from_body(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &body)
            .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p))
    }))
}

/// `GET /CodeSystem/$subsumes`.
pub async fn subsumes_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &CODE_SYSTEM_SUBSUMES, &headers, &query)
            .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$subsumes`.
pub async fn subsumes_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &CODE_SYSTEM_SUBSUMES, &headers, &body)
            .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/{id}/$subsumes`.
pub async fn subsumes_instance_get(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        from_query(&state, &CODE_SYSTEM_SUBSUMES, &headers, &query)
            .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p))
    }))
}

/// `POST /CodeSystem/{id}/$subsumes`.
pub async fn subsumes_instance_post(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        from_body(&state, &CODE_SYSTEM_SUBSUMES, &headers, &body)
            .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p))
    }))
}

/// `GET /ValueSet/$expand`.
pub async fn expand_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &VALUE_SET_EXPAND, &headers, &query)
            .and_then(|(scope, p)| run_expand(&scope, &p)),
    )
}

/// `POST /ValueSet/$expand`.
pub async fn expand_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &VALUE_SET_EXPAND, &headers, &body)
            .and_then(|(scope, p)| run_expand(&scope, &p)),
    )
}

/// `GET /ValueSet/$validate-code`.
pub async fn value_set_validate_code_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &VALUE_SET_VALIDATE_CODE, &headers, &query)
            .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p)),
    )
}

/// `POST /ValueSet/$validate-code`.
pub async fn value_set_validate_code_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &VALUE_SET_VALIDATE_CODE, &headers, &body)
            .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p)),
    )
}

/// `GET /ConceptMap/$translate`.
pub async fn translate_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        from_query(&state, &CONCEPT_MAP_TRANSLATE, &headers, &query)
            .and_then(|(scope, p)| run_translate(&scope, &p)),
    )
}

/// `POST /ConceptMap/$translate`.
pub async fn translate_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        from_body(&state, &CONCEPT_MAP_TRANSLATE, &headers, &body)
            .and_then(|(scope, p)| run_translate(&scope, &p)),
    )
}
