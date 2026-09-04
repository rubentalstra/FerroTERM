//! The system-level routes of one version: `$versions`, `$cache-control`, and `ValueSet` read and search.

macro_rules! system {
    ($fhir:ident) => {
        pub mod system {
            //! The system-level operations and the `ValueSet` read and search
            //! interactions of one version.
            //!
            //! `$versions` is R5's `CapabilityStatement/$versions`
            //! (<https://hl7.org/fhir/R5/capabilitystatement-operation-versions.html>),
            //! served here so a client learns the FHIR version of this base before it
            //! reads the capability statement. `$cache-control` is the terminology
            //! ecosystem's (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>):
            //! `mode=start` stores the request's `tx-resource`s under a new `cache-id`,
            //! `mode=end` releases the cache named by `X-Cache-Id` or `cache-id`.

            use std::sync::Arc;

            use axum::body::Bytes;
            use axum::extract::{Path as UrlPath, Query, State};
            use axum::response::{IntoResponse, Response};
            use fhir_types::$fhir::bundle::{Bundle, BundleEntry, BundleEntrySearch};
            use fhir_types::$fhir::parameters::{Parameters, ParametersParameter, ParametersParameterValue};
            use fhir_types::$fhir::resource::Resource;
            use fhir_terminology::valueset::render;
            use http::{HeaderMap, StatusCode};

            use crate::outcome::Failure;
            use super::metadata::FHIR_VERSION;
            use super::parameters;
            use super::resources::split_resources;
            use crate::scope::{CACHE_ID_HEADER, cache_id};
            use crate::state::AppState;
            use crate::wire::Wire;

            /// The canonical of the `$cache-control` operation, the ecosystem IG's.
            pub const CACHE_CONTROL_URL: &str =
                "http://hl7.org/fhir/uv/tx-ecosystem/OperationDefinition/cache-control";
            /// The canonical of R5's `$versions`.
            pub const VERSIONS_URL: &str =
                "http://hl7.org/fhir/OperationDefinition/CapabilityStatement-versions";

            fn finish(handled: Result<Response, Failure>, wire: Wire) -> Response {
                match handled {
                    Ok(response) => response,
                    Err(failure) => failure.respond(wire),
                }
            }

            /// The negotiated format of a request; a format the server does not speak
            /// is refused (in JSON, at the call site).
            fn negotiated(headers: &HeaderMap, query: &[(String, String)]) -> Result<Wire, Failure> {
                Wire::negotiate(query, headers)
            }

            /// `GET /$versions`: the FHIR versions this base serves, `MAJOR.MINOR`.
            pub async fn versions(
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let wire = match negotiated(&headers, &query) {
                    Ok(wire) => wire,
                    Err(failure) => return failure.into_response(),
                };
                let major_minor: String = FHIR_VERSION
                    .rsplitn(2, '.')
                    .last()
                    .unwrap_or(FHIR_VERSION)
                    .to_owned();
                finish(parameters::respond(&Parameters {
                    parameter: vec![
                        ParametersParameter {
                            name: "version".into(),
                            value: Some(ParametersParameterValue::Code(major_minor.as_str().into())),
                            ..Default::default()
                        },
                        ParametersParameter {
                            name: "default".into(),
                            value: Some(ParametersParameterValue::Code(major_minor.as_str().into())),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }, wire), wire)
            }

            /// `POST /$cache-control?mode=start|end`.
            pub async fn cache_control(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
                body: Bytes,
            ) -> Response {
                let wire = match negotiated(&headers, &query) {
                    Ok(wire) => wire,
                    Err(failure) => return failure.into_response(),
                };
                finish(run_cache_control(&state, &headers, &query, &body, wire), wire)
            }

            fn run_cache_control(
                state: &AppState,
                headers: &HeaderMap,
                query: &[(String, String)],
                body: &Bytes,
                wire: Wire,
            ) -> Result<Response, Failure> {
                let mode = query
                    .iter()
                    .find(|(name, _)| name == "mode")
                    .map(|(_, value)| value.as_str())
                    .ok_or_else(|| {
                        Failure::new(
                            StatusCode::BAD_REQUEST,
                            "required",
                            "`mode` is required: `start` or `end`",
                        )
                    })?;
                let (parameters, resources) = if body.is_empty() {
                    (Parameters::default(), Vec::new())
                } else {
                    split_resources(parameters::parameters_from_body(headers, body)?)?
                };
                match mode {
                    "start" => {
                        let id = state.caches().start(resources);
                        parameters::respond(&Parameters {
                            parameter: vec![ParametersParameter {
                                name: "cache-id".into(),
                                value: Some(ParametersParameterValue::Id(id.as_str().into())),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }, wire)
                    }
                    "end" => {
                        let named = parameters
                            .parameter
                            .iter()
                            .find(|p| p.name.value.as_deref() == Some("cache-id"))
                            .and_then(|p| match &p.value {
                                Some(ParametersParameterValue::Id(id)) => id.value.clone(),
                                Some(ParametersParameterValue::String(id)) => id.value.clone(),
                                _ => None,
                            });
                        let Some(id) = named.or(cache_id(headers)?) else {
                            return Err(Failure::new(
                                StatusCode::BAD_REQUEST,
                                "required",
                                format!(
                                    "`mode=end` needs the cache in `{CACHE_ID_HEADER}` or a `cache-id` parameter"
                                ),
                            ));
                        };
                        state.caches().end(&id)?;
                        parameters::respond(&Parameters::default(), wire)
                    }
                    other => Err(Failure::new(
                        StatusCode::BAD_REQUEST,
                        "value",
                        format!("`mode={other}` is not `start` or `end`"),
                    )),
                }
            }

            /// `GET /ValueSet/{id}`: the stored value set with its `compose`.
            pub async fn value_set_read(
                State(state): State<Arc<AppState>>,
                UrlPath(id): UrlPath<String>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let wire = match negotiated(&headers, &query) {
                    Ok(wire) => wire,
                    Err(failure) => return failure.into_response(),
                };
                finish(
                    match state.value_set_instance(&id) {
                        Some(model) => parameters::respond_resource(&render::$fhir::value_set(&model, true), wire),
                        None => Err(Failure::new(
                            StatusCode::NOT_FOUND,
                            "not-found",
                            format!("no ValueSet with id `{id}`"),
                        )),
                    },
                    wire,
                )
            }

            /// `GET /ValueSet?url=&version=`: a `searchset` of the stored value sets.
            pub async fn value_set_search(
                State(state): State<Arc<AppState>>,
                headers: HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let wire = match negotiated(&headers, &query) {
                    Ok(wire) => wire,
                    Err(failure) => return failure.into_response(),
                };
                finish(run_value_set_search(&state, &query, wire), wire)
            }

            fn run_value_set_search(
                state: &AppState,
                query: &[(String, String)],
                wire: Wire,
            ) -> Result<Response, Failure> {
                let mut url = None;
                let mut version = None;
                for (name, value) in query {
                    match name.as_str() {
                        "url" => url = Some(value.as_str()),
                        "version" => version = Some(value.as_str()),
                        "_format" => {}
                        other => {
                            return Err(Failure::new(
                                StatusCode::BAD_REQUEST,
                                "not-supported",
                                format!("search parameter `{other}` is not supported; use `url` and `version`"),
                            ));
                        }
                    }
                }
                let mut entry = Vec::new();
                for (id, served_url, served_version) in state.value_set_instances() {
                    if url.is_some_and(|u| u != served_url)
                        || version.is_some_and(|v| Some(v) != served_version)
                    {
                        continue;
                    }
                    let Some(model) = state.value_set_instance(id) else {
                        continue;
                    };
                    entry.push(BundleEntry {
                        full_url: Some(format!("ValueSet/{id}").as_str().into()),
                        resource: Some(Resource::ValueSet(Box::new(render::$fhir::value_set(&model, true)))),
                        search: Some(BundleEntrySearch {
                            mode: Some("match".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
                let total = u32::try_from(entry.len()).map_err(|_| {
                    Failure::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "too-costly",
                        "too many value sets to count",
                    )
                })?;
                let bundle = Bundle {
                    r#type: "searchset".into(),
                    total: Some(total.into()),
                    entry,
                    ..Default::default()
                };
                parameters::respond_resource(&bundle, wire)
            }
        }
    };
}

pub(crate) use system;
