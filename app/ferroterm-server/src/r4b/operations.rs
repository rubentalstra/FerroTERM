//! The `CodeSystem` operation handlers on R4B.
//!
//! Each handler turns the wire form into the generated request type, runs the
//! operation, and writes the generated response as `Parameters`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path as UrlPath, Query, State};
use axum::response::{IntoResponse, Response};
use ferroterm_fhir::r4b::operations::code_system_lookup::{
    CODE_SYSTEM_LOOKUP, CodeSystemLookupRequest,
};
use ferroterm_fhir::r4b::operations::code_system_subsumes::{
    CODE_SYSTEM_SUBSUMES, CodeSystemSubsumesRequest,
};
use ferroterm_fhir::r4b::operations::code_system_validate_code::{
    CODE_SYSTEM_VALIDATE_CODE, CodeSystemValidateCodeRequest,
};
use ferroterm_fhir::r4b::parameters::Parameters;
use ferroterm_terminology::operations::{Invocation, lookup, subsumes, validate_code};
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
