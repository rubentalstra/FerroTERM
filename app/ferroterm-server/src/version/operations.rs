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

            /// What one operation produced: the JSON object of its response resource.
            ///
            /// Every terminology operation answers `200` with a resource, so an
            /// invocation over HTTP and one inside a `Bundle` entry differ only in how
            /// this object is delivered.
            type Handled = Result<fhir_types::codec::Object, Failure>;

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
            ) -> Handled {
                let request = CodeSystemLookupRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome =
                    lookup::lookup(scope.registry(), invocation, &map::lookup_input(&request))?;
                parameters::encode(&map::lookup_response(outcome).to_parameters())
            }

            fn run_validate_code(
                scope: &Scope,
                invocation: &Invocation,
                parameters: &Parameters,
            ) -> Handled {
                let request = CodeSystemValidateCodeRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = validate_code::validate_code(
                    scope.registry(),
                    invocation,
                    &map::validate_code_input(&request),
                )?;
                parameters::encode(&map::validate_code_response(outcome).to_parameters())
            }

            fn run_subsumes(
                scope: &Scope,
                invocation: &Invocation,
                parameters: &Parameters,
            ) -> Handled {
                let request = CodeSystemSubsumesRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = subsumes::subsumes(
                    scope.registry(),
                    invocation,
                    &map::subsumes_input(&request),
                )?;
                parameters::encode(&map::subsumes_response(outcome).to_parameters())
            }

            fn run_expand(scope: &Scope, parameters: &Parameters) -> Handled {
                let request = ValueSetExpandRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let outcome = expand::expand(&scope.sources(), &map::expand_input(&request))?;
                parameters::encode(&render::$fhir::expansion(&outcome))
            }

            fn run_value_set_validate_code(scope: &Scope, parameters: &Parameters) -> Handled {
                let request = ValueSetValidateCodeRequest::from_parameters(parameters)
                    .map_err(|e| parameters::parameters_failure(&e))?;
                let validation = value_set_validate_code::validate_code(
                    &scope.sources(),
                    &map::value_set_validate_input(&request),
                )?;
                parameters::encode(&map::value_set_validation_parameters(&validation))
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

            fn run_translate(scope: &Scope, parameters: &Parameters) -> Handled {
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
                parameters::encode(&map::translation_parameters(&translation))
            }

            /// Which operation a `Bundle` entry's URL names.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            enum Which {
                /// `CodeSystem/$lookup`.
                Lookup,
                /// `CodeSystem/$validate-code`.
                ValidateCode,
                /// `CodeSystem/$subsumes`.
                Subsumes,
                /// `ValueSet/$expand`.
                Expand,
                /// `ValueSet/$validate-code`.
                ValueSetValidateCode,
                /// `ConceptMap/$translate`.
                Translate,
            }

            /// The operation the path `segments` name, with the descriptor that
            /// declares its parameters and the id an instance invocation carries.
            fn route(
                segments: &[&str],
            ) -> Option<(Which, &'static fhir_types::operation::Operation, Option<String>)> {
                let (which, operation, id) = match segments {
                    ["CodeSystem", "$lookup"] => (Which::Lookup, &CODE_SYSTEM_LOOKUP, None),
                    ["CodeSystem", "$validate-code"] => {
                        (Which::ValidateCode, &CODE_SYSTEM_VALIDATE_CODE, None)
                    }
                    ["CodeSystem", id, "$validate-code"] => (
                        Which::ValidateCode,
                        &CODE_SYSTEM_VALIDATE_CODE,
                        Some((*id).to_owned()),
                    ),
                    ["CodeSystem", "$subsumes"] => (Which::Subsumes, &CODE_SYSTEM_SUBSUMES, None),
                    ["CodeSystem", id, "$subsumes"] => (
                        Which::Subsumes,
                        &CODE_SYSTEM_SUBSUMES,
                        Some((*id).to_owned()),
                    ),
                    ["ValueSet", "$expand"] => (Which::Expand, &VALUE_SET_EXPAND, None),
                    ["ValueSet", "$validate-code"] => {
                        (Which::ValueSetValidateCode, &VALUE_SET_VALIDATE_CODE, None)
                    }
                    ["ConceptMap", "$translate"] => {
                        (Which::Translate, &CONCEPT_MAP_TRANSLATE, None)
                    }
                    _ => return None,
                };
                Some((which, operation, id))
            }

            /// Runs the operation `path` names, for a `Bundle` entry.
            ///
            /// `parameters` carries a `POST` entry's `Parameters` resource; a `GET`
            /// entry passes `None` and its inputs arrive in `query`.
            ///
            /// # Errors
            ///
            /// A path that names no operation of this server is a `404`, and every
            /// refusal the operation itself makes is the entry's own.
            pub(super) fn invoke(
                state: &AppState,
                headers: &HeaderMap,
                path: &str,
                query: &[(String, String)],
                sent: Option<Parameters>,
            ) -> Handled {
                let segments: Vec<&str> = path.split('/').collect();
                let Some((which, operation, id)) = route(&segments) else {
                    return Err(Failure::new(
                        StatusCode::NOT_FOUND,
                        "not-found",
                        format!("`{path}` is not an operation of this server"),
                    ));
                };
                let invocation = match &id {
                    Some(id) => instance(state, id)?,
                    None => Invocation::Type,
                };
                let (scope, parameters) = match sent {
                    Some(sent) => {
                        let (mut own, resources) = split_resources(sent)?;
                        parameters::apply_accept_language(operation, headers, &mut own);
                        (scope_of(state, headers, resources)?, own)
                    }
                    None => {
                        if which == Which::Translate {
                            reverse_refusal(query.iter().map(|(name, _)| name.as_str()))?;
                        }
                        from_query(state, operation, headers, query)?
                    }
                };
                match which {
                    Which::Lookup => run_lookup(&scope, &invocation, &parameters),
                    Which::ValidateCode => run_validate_code(&scope, &invocation, &parameters),
                    Which::Subsumes => run_subsumes(&scope, &invocation, &parameters),
                    Which::Expand => run_expand(&scope, &parameters),
                    Which::ValueSetValidateCode => run_value_set_validate_code(&scope, &parameters),
                    Which::Translate => run_translate(&scope, &parameters),
                }
            }

            /// The response of a handled invocation, a failure rendered in `wire`.
            fn finish(handled: Handled, wire: Wire) -> Response {
                match handled {
                    Ok(object) => {
                        wire.response(StatusCode::OK, &object, &fhir_types::$fhir::schema::SCHEMAS)
                    }
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
                        .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_lookup(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_validate_code(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p))
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
                        .and_then(|(scope, p)| run_validate_code(&scope, &invocation, &p))
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
                        .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_subsumes(&scope, &Invocation::Type, &p)), wire)
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
                        .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p))
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
                        .and_then(|(scope, p)| run_subsumes(&scope, &invocation, &p))
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
                        .and_then(|(scope, p)| run_expand(&scope, &p)), wire)
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
                        .and_then(|(scope, p)| run_expand(&scope, &p)), wire)
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
                        .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p)), wire)
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
                        .and_then(|(scope, p)| run_value_set_validate_code(&scope, &p)), wire)
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
                        .and_then(|(scope, p)| run_translate(&scope, &p)), wire)
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
                        .and_then(|(scope, p)| run_translate(&scope, &p)), wire)
            }
        }
    };
}

pub(crate) use operations;
