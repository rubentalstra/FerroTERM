//! The operation handlers of one version: GET and POST, type and instance level, over the engine.

macro_rules! operations {
    ($fhir:ident) => {
        pub mod operations {
            //! The `CodeSystem`, `ValueSet`, and `ConceptMap` operation handlers of one version.
            //!
            //! Each handler turns the wire form into the generated request type, runs the
            //! operation in the request's [`Scope`], and writes the generated response.
            //! `tx-resource` parameters and the `X-Cache-Id` header are peeled off before
            //! the generated request refuses what the operation does not declare.

            use std::sync::Arc;

            use axum::body::Bytes;
            use axum::extract::{Path as UrlPath, Query, State};
            use axum::response::{IntoResponse, Response};
            use fhir_terminology::operations::{
                Invocation, expand, lookup, subsumes, translate, validate_code,
                value_set_validate_code,
            };
            use fhir_terminology::valueset::render;
            use fhir_types::$fhir::operations::code_system_lookup::{
                CODE_SYSTEM_LOOKUP, CodeSystemLookupRequest,
            };
            use fhir_types::$fhir::operations::code_system_subsumes::{
                CODE_SYSTEM_SUBSUMES, CodeSystemSubsumesRequest,
            };
            use fhir_types::$fhir::operations::code_system_validate_code::{
                CODE_SYSTEM_VALIDATE_CODE, CodeSystemValidateCodeRequest,
            };
            use fhir_types::$fhir::operations::concept_map_translate::{
                CONCEPT_MAP_TRANSLATE, ConceptMapTranslateRequest,
            };
            use fhir_types::$fhir::operations::value_set_expand::{
                VALUE_SET_EXPAND, ValueSetExpandRequest,
            };
            use fhir_types::$fhir::operations::value_set_validate_code::{
                VALUE_SET_VALIDATE_CODE, ValueSetValidateCodeRequest,
            };
            use fhir_types::$fhir::parameters::Parameters;
            use http::{HeaderMap, StatusCode};

            use super::resources::split_resources;
            use super::{map, parameters};
            use crate::outcome::Failure;
            use crate::scope::{Scope, scope_of};
            use crate::state::AppState;
            use crate::wire::{Wire, without_format};

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
            fn from_query(
                state: &AppState,
                operation: &fhir_types::operation::Operation,
                headers: &HeaderMap,
                query: &[(String, String)],
            ) -> Result<(Scope, Parameters), Failure> {
                let own: Vec<(String, String)> = query
                    .iter()
                    .filter(|(name, _)| name != crate::scope::UUID)
                    .cloned()
                    .collect();
                let mut parameters = parameters::parameters_from_query(operation, &own)?;
                parameters::apply_accept_language(operation, headers, &mut parameters);
                Ok((scope_of(state, headers, Vec::new())?, parameters))
            }

            /// A `POST` invocation: the body's `Parameters` less its `tx-resource`s, in
            /// the scope those resources and the headers form.
            fn from_body(
                state: &AppState,
                operation: &fhir_types::operation::Operation,
                headers: &HeaderMap,
                body: &Bytes,
            ) -> Result<(Scope, Parameters), Failure> {
                let (mut parameters, resources) =
                    split_resources(parameters::parameters_from_body(headers, body)?)?;
                parameters::apply_accept_language(operation, headers, &mut parameters);
                Ok((scope_of(state, headers, resources)?, parameters))
            }

            fn run_lookup(
                scope: &Scope,
                invocation: &Invocation,
                parameters: &Parameters,
                wire: Wire,
            ) -> Handled {
                let request = CodeSystemLookupRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome =
                    lookup::lookup(scope.registry(), invocation, &map::lookup_input(&request))?;
                parameters::respond(&map::lookup_response(outcome).to_parameters(), wire)
            }

            fn run_validate_code(
                scope: &Scope,
                invocation: &Invocation,
                parameters: &Parameters,
                wire: Wire,
            ) -> Handled {
                let request = CodeSystemValidateCodeRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = validate_code::validate_code(
                    scope.registry(),
                    invocation,
                    &map::validate_code_input(&request),
                )?;
                parameters::respond(&map::validate_code_response(outcome).to_parameters(), wire)
            }

            fn run_subsumes(
                scope: &Scope,
                invocation: &Invocation,
                parameters: &Parameters,
                wire: Wire,
            ) -> Handled {
                let request = CodeSystemSubsumesRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = subsumes::subsumes(
                    scope.registry(),
                    invocation,
                    &map::subsumes_input(&request),
                )?;
                parameters::respond(&map::subsumes_response(outcome).to_parameters(), wire)
            }

            fn run_expand(scope: &Scope, parameters: &Parameters, wire: Wire) -> Handled {
                let request = ValueSetExpandRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = expand::expand(&scope.sources(), &map::expand_input(&request))?;
                parameters::respond_resource(&render::$fhir::expansion(&outcome), wire)
            }

            fn run_value_set_validate_code(scope: &Scope, parameters: &Parameters, wire: Wire) -> Handled {
                let request = ValueSetValidateCodeRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let validation = value_set_validate_code::validate_code(
                    &scope.sources(),
                    &map::value_set_validate_input(&request),
                )?;
                parameters::respond(&map::value_set_validation_parameters(&validation), wire)
            }

            /// The refusal of `reverse` on a version that does not declare it.
            ///
            /// R5 replaced `reverse` with the `target*` inputs
            /// (<https://hl7.org/fhir/R5/conceptmap-operation-translate.html>); the
            /// ecosystem wants `not-supported` naming them.
            fn reverse_refusal<'a>(mut names: impl Iterator<Item = &'a str>) -> Result<(), Failure> {
                if CONCEPT_MAP_TRANSLATE
                    .parameter(fhir_types::operation::ParameterUse::In, "reverse")
                    .is_none()
                    && names.any(|name| name == "reverse")
                {
                    return Err(Failure::new(
                        axum::http::StatusCode::BAD_REQUEST,
                        "not-supported",
                        format!(
                            "The 'reverse' parameter is not defined in {}: name the target concept with targetCode, targetCoding or targetCodeableConcept instead",
                            stringify!($fhir).to_uppercase()
                        ),
                    )
                    .kind("not-supported"));
                }
                Ok(())
            }

            fn run_translate(scope: &Scope, parameters: &Parameters, wire: Wire) -> Handled {
                reverse_refusal(
                    parameters
                        .parameter
                        .iter()
                        .filter_map(|p| p.name.value.as_deref()),
                )?;
                let request = ConceptMapTranslateRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let translation =
                    translate::translate(&scope.sources(), &map::translate_input(&request))?;
                parameters::respond(&map::translation_parameters(&translation), wire)
            }

            /// The response of a handled invocation, a failure rendered in `wire`.
            fn finish(handled: Handled, wire: Wire) -> Response {
                match handled {
                    Ok(response) => response,
                    Err(failure) => failure.respond(wire),
                }
            }

            /// The negotiated format and the query less `_format`; a format the server
            /// does not speak is refused (in JSON, at the call site).
            fn negotiated(
                headers: &HeaderMap,
                query: &[(String, String)],
            ) -> Result<(Wire, Vec<(String, String)>), Failure> {
                Wire::negotiate(query, headers).map(|wire| (wire, without_format(query)))
            }

            /// `GET /CodeSystem/$lookup`.
            pub async fn lookup_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_query(&state, &CODE_SYSTEM_LOOKUP, &headers, &query)
                        .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `POST /CodeSystem/$lookup`.
            pub async fn lookup_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &CODE_SYSTEM_LOOKUP, &headers, &body)
                        .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `GET /CodeSystem/$validate-code`.
            pub async fn validate_code_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_query(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &query)
                        .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `POST /CodeSystem/$validate-code`.
            pub async fn validate_code_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &body)
                        .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `GET /CodeSystem/{id}/$validate-code`.
            pub async fn validate_code_instance_get(
                State(state): State<Arc<AppState>>,
                UrlPath(id): UrlPath<String>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(instance(&state, &id).and_then(|invocation| {
                    from_query(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &query)
                        .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p, wire))
                }), wire)
            }

            /// `POST /CodeSystem/{id}/$validate-code`.
            pub async fn validate_code_instance_post(
                State(state): State<Arc<AppState>>,
                UrlPath(id): UrlPath<String>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    instance(&state, &id).and_then(|invocation| {
                    from_body(&state, &CODE_SYSTEM_VALIDATE_CODE, &headers, &body)
                        .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p, wire))
}),
                    wire,
                )
            }

            /// `GET /CodeSystem/$subsumes`.
            pub async fn subsumes_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_query(&state, &CODE_SYSTEM_SUBSUMES, &headers, &query)
                        .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `POST /CodeSystem/$subsumes`.
            pub async fn subsumes_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &CODE_SYSTEM_SUBSUMES, &headers, &body)
                        .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p, wire)), wire)
            }

            /// `GET /CodeSystem/{id}/$subsumes`.
            pub async fn subsumes_instance_get(
                State(state): State<Arc<AppState>>,
                UrlPath(id): UrlPath<String>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(instance(&state, &id).and_then(|invocation| {
                    from_query(&state, &CODE_SYSTEM_SUBSUMES, &headers, &query)
                        .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p, wire))
                }), wire)
            }

            /// `POST /CodeSystem/{id}/$subsumes`.
            pub async fn subsumes_instance_post(
                State(state): State<Arc<AppState>>,
                UrlPath(id): UrlPath<String>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    instance(&state, &id).and_then(|invocation| {
                    from_body(&state, &CODE_SYSTEM_SUBSUMES, &headers, &body)
                        .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p, wire))
}),
                    wire,
                )
            }

            /// `GET /ValueSet/$expand`.
            pub async fn expand_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_query(&state, &VALUE_SET_EXPAND, &headers, &query)
                        .and_then(|(scope, p)| run_expand(&scope, &p, wire)), wire)
            }

            /// `POST /ValueSet/$expand`.
            pub async fn expand_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &VALUE_SET_EXPAND, &headers, &body)
                        .and_then(|(scope, p)| run_expand(&scope, &p, wire)), wire)
            }

            /// `GET /ValueSet/$validate-code`.
            pub async fn value_set_validate_code_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_query(&state, &VALUE_SET_VALIDATE_CODE, &headers, &query)
                        .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p, wire)), wire)
            }

            /// `POST /ValueSet/$validate-code`.
            pub async fn value_set_validate_code_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &VALUE_SET_VALIDATE_CODE, &headers, &body)
                        .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p, wire)), wire)
            }

            /// `GET /ConceptMap/$translate`.
            pub async fn translate_get(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let (wire, query) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    reverse_refusal(query.iter().map(|(name, _)| name.as_str()))
                        .and_then(|()| from_query(&state, &CONCEPT_MAP_TRANSLATE, &headers, &query))
                        .and_then(|(scope, p)| run_translate(&scope, &p, wire)), wire)
            }

            /// `POST /ConceptMap/$translate`.
            pub async fn translate_post(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let (wire, _) = match negotiated(&headers, &query) {
                    Ok(negotiated) => negotiated,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    from_body(&state, &CONCEPT_MAP_TRANSLATE, &headers, &body)
                        .and_then(|(scope, p)| run_translate(&scope, &p, wire)), wire)
            }
        }
    };
}

pub(crate) use operations;
