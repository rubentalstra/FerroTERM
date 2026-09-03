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
use ferroterm_fhir::r4b::parameters::Parameters;
use ferroterm_terminology::operations::{
    Invocation, expand, lookup, subsumes, translate, validate_code, value_set_validate_code,
};
use ferroterm_terminology::valueset::render;
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
    let outcome = validate_code::validate_code(
        scope.registry(),
        invocation,
        &map::validate_code_input(&request),
    )?;
    wire::respond(&map::validate_code_response(outcome).to_parameters())
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
    let outcome = expand::expand(&scope.sources(), &map::expand_input(&request))?;
    wire::respond_resource(&render::expansion_r4b(&outcome))
}

fn run_value_set_validate_code(scope: &Scope<'_>, parameters: &Parameters) -> Handled {
    let request = ValueSetValidateCodeRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let validation = value_set_validate_code::validate_code(
        &scope.sources(),
        &map::value_set_validate_input(&request),
    )?;
    wire::respond(&map::value_set_validation_parameters(&validation))
}

fn run_translate(scope: &Scope<'_>, parameters: &Parameters) -> Handled {
    let request = ConceptMapTranslateRequest::from_parameters(parameters)
        .map_err(|e| wire::parameters_failure(&e))?;
    let translation = translate::translate(&scope.sources(), &map::translate_input(&request))?;
    wire::respond(&map::translation_parameters(&translation))
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
