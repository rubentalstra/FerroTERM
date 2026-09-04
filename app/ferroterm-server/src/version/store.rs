//! The REST interactions of the persisted `CodeSystem`, `ValueSet`, and
//! `ConceptMap` resources.
//!
//! The FHIR REST API defines create, update, read, version read, search,
//! and delete, their status codes, and the `ETag` and `Last-Modified` headers
//! a version-aware client works with
//! (<https://hl7.org/fhir/R4B/http.html>). One implementation serves every
//! version: a resource is stored as the JSON object it arrived as, with the
//! FHIR version it arrived in, and a read renders it through the reading
//! version's own codec. The per-version `store!` macro only binds the routes
//! to it.

use axum::body::Bytes;
use axum::response::Response;
use fhir_terminology::valueset::model::ValueSetModel;
use fhir_types::codec::{Object, expect_object};
use fhir_types::xml::Schemas;
use http::header::{CONTENT_TYPE, ETAG, IF_MATCH, LAST_MODIFIED, LOCATION};
use http::{HeaderMap, HeaderValue, StatusCode};

use crate::outcome::Failure;
use crate::persistence::{Record, ResourceType};
use crate::state::{AppState, PersistError};
use crate::wire::Wire;

/// The generated codec of one served version.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Surface {
    /// The FHIR version this endpoint serves.
    pub fhir_version: &'static str,
    /// The version's XML schema, for an XML request or response.
    pub schemas: &'static Schemas,
    /// Reads a stored object as a resource of this version and writes it back.
    pub round_trip: fn(&Object) -> Result<Object, String>,
    /// Renders a loaded value set as a `ValueSet` of this version.
    pub render_value_set: fn(&ValueSetModel) -> Result<Object, String>,
}

/// One request against the persisted resources of one type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Request<'a> {
    /// The server state.
    pub state: &'a AppState,
    /// The version this request arrived on.
    pub surface: Surface,
    /// The resource type the route serves.
    pub resource_type: ResourceType,
    /// The request headers.
    pub headers: &'a HeaderMap,
    /// The request path, for the `Location` of a created resource.
    pub path: &'a str,
    /// The negotiated response format.
    pub wire: Wire,
}

/// `POST {type}`: stores `body` under a server-assigned id.
///
/// # Errors
///
/// A body that is not a resource of this type is a 400; a deployment that
/// persists nothing refuses with a 422.
pub(crate) fn create(request: &Request<'_>, body: &Bytes) -> Result<Response, Failure> {
    let mut object = body_object(request, body)?;
    let id = uuid::Uuid::new_v4().to_string();
    object.insert("id".to_owned(), serde_json::Value::String(id.clone()));
    let record = write(request, &id, object)?;
    Ok(created(request, &record))
}

/// `PUT {type}/{id}`: stores `body` as `id`, creating it when it is new.
///
/// # Errors
///
/// A body that is not a resource of this type, or one whose `id` is another,
/// is a 400; an `If-Match` that does not hold is a 412.
pub(crate) fn update(request: &Request<'_>, id: &str, body: &Bytes) -> Result<Response, Failure> {
    let known = check_id(id)?;
    let mut object = body_object(request, body)?;
    if let Some(sent) = object.get("id").and_then(serde_json::Value::as_str)
        && sent != known
    {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid",
            format!("the resource id `{sent}` is not the id `{known}` of the request URL"),
        ));
    }
    let held = request.state.persisted_record(request.resource_type, known);
    check_if_match(request.headers, held.as_ref())?;
    object.insert("id".to_owned(), serde_json::Value::String(known.to_owned()));
    let record = write(request, known, object)?;
    if held.is_none() {
        return Ok(created(request, &record));
    }
    let object = rendered(request, &record)?;
    Ok(with_headers(
        request
            .wire
            .response(StatusCode::OK, &object, request.surface.schemas),
        &record,
        None,
    ))
}

/// `GET {type}/{id}`: the current version of the resource.
///
/// A `ValueSet` id the deployment loaded from disk reads here too, rendered
/// from the model the engine holds; it carries no `ETag`, because a loaded
/// resource has no version this server counts.
///
/// # Errors
///
/// An id that was never stored is a 404, one whose current version was
/// deleted is a 410.
pub(crate) fn read(request: &Request<'_>, id: &str) -> Result<Response, Failure> {
    let known = check_id(id)?;
    let Some(record) = request.state.persisted_record(request.resource_type, known) else {
        if request.resource_type == ResourceType::ValueSet
            && let Some(model) = request.state.value_set_instance(known)
        {
            let object =
                (request.surface.render_value_set)(&model).map_err(|reason| rendering(&reason))?;
            return Ok(request
                .wire
                .response(StatusCode::OK, &object, request.surface.schemas));
        }
        return Err(gone_or_missing(request, known));
    };
    let object = rendered(request, &record)?;
    Ok(with_headers(
        request
            .wire
            .response(StatusCode::OK, &object, request.surface.schemas),
        &record,
        None,
    ))
}

/// `GET {type}/{id}/_history/{version_id}`: one version of the resource.
///
/// # Errors
///
/// A version that was never written is a 404.
pub(crate) fn version_read(
    request: &Request<'_>,
    id: &str,
    version_id: &str,
) -> Result<Response, Failure> {
    let known = check_id(id)?;
    let wanted = version_id.parse::<u32>().map_err(|_unparsed| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "not-found",
            format!("`{version_id}` is not a version this server writes"),
        )
    })?;
    let held = request
        .state
        .persisted_version(request.resource_type, known, wanted)
        .map_err(|error| persist_failure(&error))?;
    let Some(record) = held else {
        return Err(Failure::new(
            StatusCode::NOT_FOUND,
            "not-found",
            format!(
                "no version `{wanted}` of {}/{known}",
                request.resource_type.name()
            ),
        ));
    };
    let object = rendered(request, &record)?;
    Ok(with_headers(
        request
            .wire
            .response(StatusCode::OK, &object, request.surface.schemas),
        &record,
        None,
    ))
}

/// `DELETE {type}/{id}`: removes the current version.
///
/// A delete of a resource that is already deleted, or of one that never
/// existed, has no effect and answers `204` all the same
/// (<https://hl7.org/fhir/R4B/http.html#delete>).
///
/// # Errors
///
/// An `If-Match` that does not hold is a 412; a deployment that persists
/// nothing refuses with a 422.
pub(crate) fn delete(request: &Request<'_>, id: &str) -> Result<Response, Failure> {
    let known = check_id(id)?;
    let held = request.state.persisted_record(request.resource_type, known);
    check_if_match(request.headers, held.as_ref())?;
    request
        .state
        .delete_persisted(request.resource_type, known)
        .map_err(|error| persist_failure(&error))?;
    Ok(status_response(StatusCode::NO_CONTENT))
}

/// The records a search over `url` and `version` matches, sorted by id.
///
/// # Errors
///
/// A search parameter this server does not answer is a 400.
pub(crate) fn matches(
    state: &AppState,
    resource_type: ResourceType,
    query: &[(String, String)],
) -> Result<Vec<Record>, Failure> {
    let (url, version) = criteria(query)?;
    Ok(state
        .persisted_records(resource_type)
        .into_iter()
        .filter(|record| {
            url.is_none_or(|wanted| record.url.as_deref() == Some(wanted))
                && version.is_none_or(|wanted| record.version.as_deref() == Some(wanted))
        })
        .collect())
}

/// `record` as a resource of the reading version, with its `meta`.
///
/// # Errors
///
/// A resource stored in another FHIR version that carries an element this one
/// does not define is a 422.
pub(crate) fn rendered(request: &Request<'_>, record: &Record) -> Result<Object, Failure> {
    (request.surface.round_trip)(&record.resource).map_err(|reason| {
        Failure::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "not-supported",
            format!(
                "{}/{} was written as FHIR {} and does not read as FHIR {}: {reason}",
                record.resource_type, record.id, record.fhir_version, request.surface.fhir_version
            ),
        )
    })
}

/// The `ETag` and `Last-Modified` of `record` on `response`, and its
/// `Location` when one is given.
pub(crate) fn with_headers(
    mut response: Response,
    record: &Record,
    location: Option<&str>,
) -> Response {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&record.etag()) {
        headers.insert(ETAG, value);
    }
    if let Some(value) = http_date(&record.last_modified) {
        headers.insert(LAST_MODIFIED, value);
    }
    if let Some(value) = location.and_then(|text| HeaderValue::from_str(text).ok()) {
        headers.insert(LOCATION, value);
    }
    response
}

/// The `201 Created` of a resource that was just written.
fn created(request: &Request<'_>, record: &Record) -> Response {
    let location = format!(
        "{}/{}/_history/{}",
        request.path.trim_end_matches('/'),
        record.id,
        record.version_id
    );
    let response = match rendered(request, record) {
        Ok(object) => request
            .wire
            .response(StatusCode::CREATED, &object, request.surface.schemas),
        // NOTE: the resource was just read through this version's codec, so it renders;
        // if it ever does not, the status and headers still tell the client what happened.
        Err(_) => status_response(StatusCode::CREATED),
    };
    with_headers(response, record, Some(&location))
}

/// Stores `object` as `id` and returns the record it became.
fn write(request: &Request<'_>, id: &str, object: Object) -> Result<Record, Failure> {
    request
        .state
        .put_persisted(
            request.resource_type,
            id,
            request.surface.fhir_version,
            object,
        )
        .map_err(|error| persist_failure(&error))
}

/// The body as a JSON object of the request's format, refused when it is not a
/// resource of the route's type.
fn body_object(request: &Request<'_>, body: &Bytes) -> Result<Object, Failure> {
    let structure = |text: String| Failure::new(StatusCode::BAD_REQUEST, "structure", text);
    let object = match Wire::of_body(request.headers)? {
        Wire::Json => {
            let value: serde_json::Value = serde_json::from_slice(body)
                .map_err(|error| structure(format!("the body is not JSON: {error}")))?;
            let path = fhir_types::codec::Path::root(request.resource_type.name());
            expect_object(&value, &path)
                .map_err(|error| structure(error.to_string()))?
                .clone()
        }
        Wire::Xml => {
            let text = std::str::from_utf8(body)
                .map_err(|error| structure(format!("the body is not UTF-8: {error}")))?;
            fhir_types::xml::from_xml(request.surface.schemas, text)
                .map_err(|error| structure(error.to_string()))?
        }
    };
    let sent = object
        .get("resourceType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if sent != request.resource_type.name() {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid",
            format!(
                "this endpoint stores {} resources, and the body is a `{sent}`",
                request.resource_type.name()
            ),
        ));
    }
    Ok(object)
}

/// `id` when it is a FHIR id.
fn check_id(id: &str) -> Result<&str, Failure> {
    let shaped = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if shaped {
        return Ok(id);
    }
    Err(Failure::new(
        StatusCode::BAD_REQUEST,
        "value",
        format!("`{id}` is not a FHIR id: at most 64 of A-Z, a-z, 0-9, `-`, and `.`"),
    ))
}

/// The `412` of an `If-Match` that names another version.
fn check_if_match(headers: &HeaderMap, held: Option<&Record>) -> Result<(), Failure> {
    let Some(wanted) = headers.get(IF_MATCH).and_then(|value| value.to_str().ok()) else {
        return Ok(());
    };
    let current = held.map(Record::etag);
    if current.as_deref() == Some(wanted.trim()) {
        return Ok(());
    }
    Err(Failure::new(
        StatusCode::PRECONDITION_FAILED,
        "conflict",
        match current {
            Some(current) => {
                format!("`If-Match: {wanted}` does not hold: the version is {current}")
            }
            None => format!("`If-Match: {wanted}` does not hold: there is no such resource"),
        },
    ))
}

/// The `410` of a resource whose versions were written and then deleted, and
/// the `404` of one that never existed.
fn gone_or_missing(request: &Request<'_>, id: &str) -> Failure {
    let deleted = request
        .state
        .persisted_version(request.resource_type, id, 1)
        .is_ok_and(|held| held.is_some());
    if deleted {
        return Failure::new(
            StatusCode::GONE,
            "deleted",
            format!("{}/{id} is deleted", request.resource_type.name()),
        );
    }
    Failure::new(
        StatusCode::NOT_FOUND,
        "not-found",
        format!("no {} with id `{id}`", request.resource_type.name()),
    )
}

/// The failure a store error answers with.
fn persist_failure(error: &PersistError) -> Failure {
    let (status, code) = match error {
        PersistError::NotConfigured => (StatusCode::UNPROCESSABLE_ENTITY, "not-supported"),
        PersistError::Convert { .. } | PersistError::Layer(_) => {
            (StatusCode::BAD_REQUEST, "invalid")
        }
        PersistError::Store(_) => (StatusCode::INTERNAL_SERVER_ERROR, "exception"),
    };
    Failure::new(status, code, error.to_string())
}

/// The ids of the value sets the deployment loaded that `query` matches, the
/// persisted ones left out because a search lists those from their records.
///
/// # Errors
///
/// A search parameter this server does not answer is a 400.
pub(crate) fn loaded_value_sets(
    state: &AppState,
    query: &[(String, String)],
) -> Result<Vec<String>, Failure> {
    let (url, version) = criteria(query)?;
    Ok(state
        .value_set_instances()
        .into_iter()
        .filter(|(id, served_url, served_version)| {
            state.persisted_record(ResourceType::ValueSet, id).is_none()
                && url.is_none_or(|wanted| served_url == wanted)
                && version.is_none_or(|wanted| served_version.as_deref() == Some(wanted))
        })
        .map(|(id, _, _)| id)
        .collect())
}

/// The `url` and `version` a search names.
fn criteria(query: &[(String, String)]) -> Result<(Option<&str>, Option<&str>), Failure> {
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
    Ok((url, version))
}

/// The `500` of a resource the server holds but cannot encode.
pub(crate) fn rendering(reason: &str) -> Failure {
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "exception",
        format!("cannot encode the resource: {reason}"),
    )
}

/// A response that carries only a status.
fn status_response(status: StatusCode) -> Response {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, crate::wire::FHIR_JSON)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| axum::response::IntoResponse::into_response(status))
}

/// A stored instant as the HTTP-date `Last-Modified` carries
/// (<https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.7>).
fn http_date(instant: &str) -> Option<HeaderValue> {
    let timestamp: jiff::Timestamp = instant.parse().ok()?;
    HeaderValue::from_str(&timestamp.strftime("%a, %d %b %Y %H:%M:%S GMT").to_string()).ok()
}

/// The six axum handlers of one resource type, over the shared implementation.
///
/// The `store!` macro invokes this by path, once per type: a `macro_rules!`
/// definition nested inside another one cannot spell its own metavariables
/// (<https://doc.rust-lang.org/reference/macros-by-example.html>).
macro_rules! store_routes {
    ($create:ident, $read:ident, $update:ident, $version_read:ident, $delete:ident, $search:ident, $kind:expr) => {
        /// `POST`: stores the body under a server-assigned id.
        pub async fn $create(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
            body: axum::body::Bytes,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            let request = crate::version::store::Request {
                state: &state,
                surface: surface(),
                resource_type: $kind,
                headers: &headers,
                path: uri.path(),
                wire,
            };
            finish(crate::version::store::create(&request, &body), wire)
        }

        /// `GET {id}`: the current version of the resource.
        pub async fn $read(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            axum::extract::Path(id): axum::extract::Path<String>,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            let request = crate::version::store::Request {
                state: &state,
                surface: surface(),
                resource_type: $kind,
                headers: &headers,
                path: "",
                wire,
            };
            finish(crate::version::store::read(&request, &id), wire)
        }

        /// `PUT {id}`: stores the body as `id`.
        pub async fn $update(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            axum::extract::Path(id): axum::extract::Path<String>,
            axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
            body: axum::body::Bytes,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            let path = uri
                .path()
                .rsplit_once('/')
                .map_or_else(|| uri.path().to_owned(), |(head, _)| head.to_owned());
            let request = crate::version::store::Request {
                state: &state,
                surface: surface(),
                resource_type: $kind,
                headers: &headers,
                path: &path,
                wire,
            };
            finish(crate::version::store::update(&request, &id, &body), wire)
        }

        /// `GET {id}/_history/{version}`: one version of the resource.
        pub async fn $version_read(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            axum::extract::Path((id, version)): axum::extract::Path<(String, String)>,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            let request = crate::version::store::Request {
                state: &state,
                surface: surface(),
                resource_type: $kind,
                headers: &headers,
                path: "",
                wire,
            };
            finish(
                crate::version::store::version_read(&request, &id, &version),
                wire,
            )
        }

        /// `DELETE {id}`: removes the current version.
        pub async fn $delete(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            axum::extract::Path(id): axum::extract::Path<String>,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            let request = crate::version::store::Request {
                state: &state,
                surface: surface(),
                resource_type: $kind,
                headers: &headers,
                path: "",
                wire,
            };
            finish(crate::version::store::delete(&request, &id), wire)
        }

        /// `GET ?url=&version=`: a `searchset` of the persisted resources.
        pub async fn $search(
            axum::extract::State(state): axum::extract::State<
                std::sync::Arc<crate::state::AppState>,
            >,
            headers: http::HeaderMap,
            axum::extract::Query(query): axum::extract::Query<Vec<(String, String)>>,
        ) -> axum::response::Response {
            let Some(wire) = negotiated(&query, &headers) else {
                return refused(&query, &headers);
            };
            finish(search(&state, $kind, &query, wire), wire)
        }
    };
}

pub(crate) use store_routes;

macro_rules! store {
    ($fhir:ident) => {
        pub mod store {
            //! The persisted-resource routes of one version, over the shared
            //! implementation in `crate::version::store`.

            use axum::response::{IntoResponse, Response};
            use fhir_types::$fhir::bundle::{Bundle, BundleEntry, BundleEntrySearch};
            use fhir_types::$fhir::resource::Resource;
            use http::{HeaderMap, StatusCode};

            use crate::outcome::Failure;
            use crate::persistence::ResourceType;
            use crate::state::AppState;
            use crate::version::store::{Request, Surface};
            use crate::wire::Wire;

            use super::metadata::FHIR_VERSION;
            use super::parameters;

            /// The codec of this version, for the shared implementation.
            fn surface() -> Surface {
                Surface {
                    fhir_version: FHIR_VERSION,
                    schemas: &fhir_types::$fhir::schema::SCHEMAS,
                    round_trip: super::resources::round_trip,
                    render_value_set: |model| {
                        fhir_types::codec::Json::to_json(
                            &fhir_terminology::valueset::render::$fhir::value_set(model, true),
                        )
                        .map_err(|error| error.to_string())
                    },
                }
            }

            /// The response format the request asks for, `None` when the server
            /// does not speak it.
            fn negotiated(query: &[(String, String)], headers: &HeaderMap) -> Option<Wire> {
                Wire::negotiate(query, headers).ok()
            }

            /// The refusal of a format the server does not speak.
            fn refused(query: &[(String, String)], headers: &HeaderMap) -> Response {
                match Wire::negotiate(query, headers) {
                    Ok(_) => StatusCode::NOT_ACCEPTABLE.into_response(),
                    Err(failure) => failure.into_response(),
                }
            }

            fn finish(handled: Result<Response, Failure>, wire: Wire) -> Response {
                match handled {
                    Ok(response) => response,
                    Err(failure) => failure.respond(wire),
                }
            }

            /// One `match` entry of a `searchset`.
            fn found(full_url: &str, resource: Resource) -> BundleEntry {
                BundleEntry {
                    full_url: Some(full_url.into()),
                    resource: Some(resource),
                    search: Some(BundleEntrySearch {
                        mode: Some("match".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            }

            /// The `searchset` of the persisted resources of `resource_type`
            /// that `query` matches.
            ///
            /// # Errors
            ///
            /// A search parameter this server does not answer is a 400, and a
            /// resource this version cannot render is a 422.
            pub fn search(
                state: &AppState,
                resource_type: ResourceType,
                query: &[(String, String)],
                wire: Wire,
            ) -> Result<Response, Failure> {
                let headers = HeaderMap::new();
                let request = Request {
                    state,
                    surface: surface(),
                    resource_type,
                    headers: &headers,
                    path: "",
                    wire,
                };
                let mut entry = Vec::new();
                if resource_type == ResourceType::ValueSet {
                    for id in crate::version::store::loaded_value_sets(state, query)? {
                        let Some(model) = state.value_set_instance(&id) else {
                            continue;
                        };
                        entry.push(found(
                            &format!("ValueSet/{id}"),
                            Resource::ValueSet(Box::new(
                                fhir_terminology::valueset::render::$fhir::value_set(&model, true),
                            )),
                        ));
                    }
                }
                for record in crate::version::store::matches(state, resource_type, query)? {
                    let object = crate::version::store::rendered(&request, &record)?;
                    let resource = super::resources::resource_of(&object)
                        .map_err(|reason| crate::version::store::rendering(&reason))?;
                    entry.push(found(
                        &format!("{}/{}", resource_type.name(), record.id),
                        resource,
                    ));
                }
                let total = u32::try_from(entry.len()).map_err(|_| {
                    Failure::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "too-costly",
                        "too many resources to count",
                    )
                })?;
                parameters::respond_resource(
                    &Bundle {
                        r#type: "searchset".into(),
                        total: Some(total.into()),
                        entry,
                        ..Default::default()
                    },
                    wire,
                )
            }

            /// The stored resource of `resource_type` with `id`, when there is
            /// one; a route that also serves loaded resources tries this first.
            pub fn stored(
                state: &AppState,
                resource_type: ResourceType,
                id: &str,
                headers: &HeaderMap,
                wire: Wire,
            ) -> Option<Result<Response, Failure>> {
                state.persisted_record(resource_type, id)?;
                let request = Request {
                    state,
                    surface: surface(),
                    resource_type,
                    headers,
                    path: "",
                    wire,
                };
                Some(crate::version::store::read(&request, id))
            }

            crate::version::store::store_routes!(
                code_system_create,
                code_system_read,
                code_system_update,
                code_system_version_read,
                code_system_delete,
                code_system_search,
                ResourceType::CodeSystem
            );
            crate::version::store::store_routes!(
                value_set_create,
                value_set_read,
                value_set_update,
                value_set_version_read,
                value_set_delete,
                value_set_search,
                ResourceType::ValueSet
            );
            crate::version::store::store_routes!(
                concept_map_create,
                concept_map_read,
                concept_map_update,
                concept_map_version_read,
                concept_map_delete,
                concept_map_search,
                ResourceType::ConceptMap
            );
        }
    };
}

pub(crate) use store;
