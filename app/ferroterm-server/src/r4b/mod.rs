//! The FHIR R4B surface under `/r4b`.

pub mod map;
pub mod metadata;
pub mod operations;
pub mod system;
pub mod wire;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::state::AppState;

/// The R4B routes: the capability statements, the `CodeSystem` operations at
/// the levels the R4B `OperationDefinition`s declare, and the type-level
/// `ValueSet` and `ConceptMap` operations.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/metadata", get(metadata::metadata))
        .route("/$versions", get(system::versions))
        .route(
            "/$cache-control",
            axum::routing::post(system::cache_control),
        )
        .route("/ValueSet", get(system::value_set_search))
        .route("/ValueSet/{id}", get(system::value_set_read))
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
            "/ConceptMap/$translate",
            get(operations::translate_get).post(operations::translate_post),
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
