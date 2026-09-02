//! The FHIR R4B surface under `/r4b`.

pub mod metadata;
pub mod operations;
pub mod wire;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::state::AppState;

/// The R4B routes: the capability statements, the `CodeSystem` operations at
/// the levels the R4B `OperationDefinition`s declare, and the type-level
/// `ValueSet` operations.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/metadata", get(metadata::metadata))
        .route(
            "/CodeSystem/$lookup",
            get(operations::lookup_get).post(operations::lookup_post),
        )
        .route(
            "/CodeSystem/$validate-code",
            get(operations::validate_code_get).post(operations::validate_code_post),
        )
        .route(
            "/CodeSystem/{id}/$validate-code",
            get(operations::validate_code_instance_get)
                .post(operations::validate_code_instance_post),
        )
        .route(
            "/ValueSet/$expand",
            get(operations::expand_get).post(operations::expand_post),
        )
        .route(
            "/ValueSet/$validate-code",
            get(operations::value_set_validate_code_get)
                .post(operations::value_set_validate_code_post),
        )
        .route(
            "/CodeSystem/$subsumes",
            get(operations::subsumes_get).post(operations::subsumes_post),
        )
        .route(
            "/CodeSystem/{id}/$subsumes",
            get(operations::subsumes_instance_get).post(operations::subsumes_instance_post),
        )
}
