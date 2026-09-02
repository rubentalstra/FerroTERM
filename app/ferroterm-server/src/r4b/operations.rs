//! The `CodeSystem` and `ValueSet` operation handlers on R4B.
//!
//! Each handler turns the wire form into the generated request type, runs the
//! operation, and writes the generated response as `Parameters`.

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
use ferroterm_fhir::r4b::operations::value_set_expand::{VALUE_SET_EXPAND, ValueSetExpandRequest};
use ferroterm_fhir::r4b::operations::value_set_validate_code::{
    VALUE_SET_VALIDATE_CODE, ValueSetValidateCodeRequest,
};
use ferroterm_fhir::r4b::parameters::{Parameters, ParametersParameter, ParametersParameterValue};
use ferroterm_fhir::r4b::resource::Resource;
use ferroterm_terminology::operations::value_set_validate_code::{Validation, tx_issue_coding};
use ferroterm_terminology::operations::{
    Invocation, expand, lookup, subsumes, validate_code, value_set_validate_code,
};
use http::{HeaderMap, StatusCode};

use crate::outcome::Failure;
use crate::r4b::wire;
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

fn run_lookup(state: &AppState, invocation: &Invocation, parameters: &Parameters) -> Handled {
    let request = CodeSystemLookupRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = lookup::lookup(state.registry(), invocation, &request)?;
    wire::respond(&response.to_parameters())
}

fn run_validate_code(
    state: &AppState,
    invocation: &Invocation,
    parameters: &Parameters,
) -> Handled {
    let request = CodeSystemValidateCodeRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = validate_code::validate_code(state.registry(), invocation, &request)?;
    wire::respond(&response.to_parameters())
}

fn run_subsumes(state: &AppState, invocation: &Invocation, parameters: &Parameters) -> Handled {
    let request = CodeSystemSubsumesRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = subsumes::subsumes(state.registry(), invocation, &request)?;
    wire::respond(&response.to_parameters())
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
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        wire::parameters_from_query(&CODE_SYSTEM_LOOKUP, &query)
            .and_then(|p| run_lookup(&state, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$lookup`.
pub async fn lookup_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_lookup(&state, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/$validate-code`.
pub async fn validate_code_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        wire::parameters_from_query(&CODE_SYSTEM_VALIDATE_CODE, &query)
            .and_then(|p| run_validate_code(&state, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$validate-code`.
pub async fn validate_code_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_validate_code(&state, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/{id}/$validate-code`.
pub async fn validate_code_instance_get(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        wire::parameters_from_query(&CODE_SYSTEM_VALIDATE_CODE, &query)
            .and_then(|p| run_validate_code(&state, &invocation, &p))
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
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_validate_code(&state, &invocation, &p))
    }))
}

/// `GET /CodeSystem/$subsumes`.
pub async fn subsumes_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        wire::parameters_from_query(&CODE_SYSTEM_SUBSUMES, &query)
            .and_then(|p| run_subsumes(&state, &Invocation::Type, &p)),
    )
}

/// `POST /CodeSystem/$subsumes`.
pub async fn subsumes_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_subsumes(&state, &Invocation::Type, &p)),
    )
}

/// `GET /CodeSystem/{id}/$subsumes`.
pub async fn subsumes_instance_get(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(instance(&state, &id).and_then(|invocation| {
        wire::parameters_from_query(&CODE_SYSTEM_SUBSUMES, &query)
            .and_then(|p| run_subsumes(&state, &invocation, &p))
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
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_subsumes(&state, &invocation, &p))
    }))
}

fn run_expand(state: &AppState, parameters: &Parameters) -> Handled {
    let request = ValueSetExpandRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let response = expand::expand(&state.sources(), &request)?;
    wire::respond_resource(&response.r#return)
}

fn run_value_set_validate_code(state: &AppState, parameters: &Parameters) -> Handled {
    let request = ValueSetValidateCodeRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let validation = value_set_validate_code::validate_code(&state.sources(), &request)?;
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
                location: issue.expression.map(Into::into).into_iter().collect(),
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

/// `GET /ValueSet/$expand`.
pub async fn expand_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        wire::parameters_from_query(&VALUE_SET_EXPAND, &query).and_then(|p| run_expand(&state, &p)),
    )
}

/// `POST /ValueSet/$expand`.
pub async fn expand_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(wire::parameters_from_body(&headers, &body).and_then(|p| run_expand(&state, &p)))
}

/// `GET /ValueSet/$validate-code`.
pub async fn value_set_validate_code_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        wire::parameters_from_query(&VALUE_SET_VALIDATE_CODE, &query)
            .and_then(|p| run_value_set_validate_code(&state, &p)),
    )
}

/// `POST /ValueSet/$validate-code`.
pub async fn value_set_validate_code_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    finish(
        wire::parameters_from_body(&headers, &body)
            .and_then(|p| run_value_set_validate_code(&state, &p)),
    )
}
