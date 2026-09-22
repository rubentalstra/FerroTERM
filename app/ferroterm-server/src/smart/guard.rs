//! The middleware that decides which requests need a token, and checks it.
//!
//! One layer sits inside each version's router, so the path it reads is the one
//! under the version prefix, and one sits on the admin listener. Both pass
//! every request through untouched when the deployment configured no issuer.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use http::Method;

use crate::smart::scopes::Permission;
use crate::smart::{Need, Smart};

/// The resource types a client writes through the REST API.
const WRITTEN: [&str; 3] = ["CodeSystem", "ValueSet", "ConceptMap"];

/// The system-level route of `ConceptMap/$closure`.
const CLOSURE: &str = "$closure";

/// Gates the write routes of one FHIR version.
///
/// `POST` on a resource type creates, `PUT` on an instance updates, and
/// `DELETE` on an instance deletes; `$closure` maintains a stored table and is
/// gated with them. Every other route, the operations included, reads and stays
/// open (<https://hl7.org/fhir/R4B/http.html>).
pub async fn writes(
    State(smart): State<Option<Arc<Smart>>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(smart) = smart else {
        return next.run(request).await;
    };
    let Some(need) = needed(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };
    match smart.authorize(request.headers(), need).await {
        Ok(()) => next.run(request).await,
        Err(refusal) => refusal.fhir_response(request.headers()),
    }
}

/// Gates every route of the admin listener.
///
/// No specification governs the admin surface: our own design, so the scope it
/// requires is the one the deployment names.
pub async fn admin(
    State(smart): State<Option<Arc<Smart>>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(smart) = smart else {
        return next.run(request).await;
    };
    match smart.authorize(request.headers(), Need::Admin).await {
        Ok(()) => next.run(request).await,
        Err(refusal) => refusal.admin_response(),
    }
}

/// What `method` on `path` requires, or `None` when the route reads.
///
/// `path` is the one under the version prefix, which is what a router nested by
/// `axum` sees (<https://docs.rs/axum/0.8/axum/struct.Router.html#method.nest>).
fn needed(method: &Method, path: &str) -> Option<Need> {
    let mut segments = path.trim_matches('/').split('/');
    let named = segments.next()?;
    let instance = segments.next();
    if named == CLOSURE && instance.is_none() && method == Method::POST {
        // NOTE: `$closure` keeps a closure table in the resource database
        // (<https://hl7.org/fhir/R4B/conceptmap-operation-closure.html>), and no
        // SMART scope names it, so this mapping is our own design.
        return Some(Need::Write {
            resource: "ConceptMap",
            permission: Permission::Update,
        });
    }
    let resource = WRITTEN.into_iter().find(|known| *known == named)?;
    if segments.next().is_some() {
        return None;
    }
    let permission = match (method, instance) {
        (&Method::POST, None) => Permission::Create,
        // An instance segment starting with `$` is an operation, never an id.
        (&Method::PUT, Some(id)) if !id.starts_with('$') => Permission::Update,
        (&Method::DELETE, Some(id)) if !id.starts_with('$') => Permission::Delete,
        _ => return None,
    };
    Some(Need::Write {
        resource,
        permission,
    })
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::needed;
    use crate::smart::Need;
    use crate::smart::scopes::Permission;

    #[test]
    fn the_three_write_interactions_are_the_gated_ones() {
        assert_eq!(
            needed(&Method::POST, "/CodeSystem"),
            Some(Need::Write {
                resource: "CodeSystem",
                permission: Permission::Create
            })
        );
        assert_eq!(
            needed(&Method::PUT, "/ValueSet/vs-1"),
            Some(Need::Write {
                resource: "ValueSet",
                permission: Permission::Update
            })
        );
        assert_eq!(
            needed(&Method::DELETE, "/ConceptMap/cm-1"),
            Some(Need::Write {
                resource: "ConceptMap",
                permission: Permission::Delete
            })
        );
    }

    // `$closure` maintains a stored closure table
    // (<https://hl7.org/fhir/R4B/conceptmap-operation-closure.html>).
    #[test]
    fn closure_is_gated_as_a_concept_map_update() {
        assert_eq!(
            needed(&Method::POST, "/$closure"),
            Some(Need::Write {
                resource: "ConceptMap",
                permission: Permission::Update
            })
        );
        assert_eq!(needed(&Method::GET, "/$closure"), None);
    }

    #[test]
    fn every_read_and_every_operation_stays_open() {
        // NOTE: `$cache-control` holds supplied resources for a run of reads and
        // stores no content, so it stays open
        // (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>).
        for (method, path) in [
            (Method::POST, "/$cache-control"),
            (Method::GET, "/CodeSystem"),
            (Method::GET, "/CodeSystem/cs-1"),
            (Method::GET, "/CodeSystem/cs-1/_history/1"),
            (Method::POST, "/CodeSystem/$lookup"),
            (Method::POST, "/CodeSystem/cs-1/$validate-code"),
            (Method::POST, "/ValueSet/$expand"),
            (Method::POST, "/ConceptMap/$translate"),
            (Method::POST, "/"),
            (Method::GET, "/metadata"),
            (Method::GET, "/.well-known/smart-configuration"),
            (Method::POST, "/Patient"),
        ] {
            assert_eq!(needed(&method, path), None, "{method} {path}");
        }
    }
}
