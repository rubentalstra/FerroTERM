//! The FHIR surface of one served version, instantiated per version.
//!
//! R4 (4.0.1) and R4B (4.3.0) declare the same terminology operation
//! parameters and the same `CapabilityStatement`, `TerminologyCapabilities`,
//! `ValueSet`, and `Parameters` elements this server fills, so one set of
//! macros produces each version's modules from the generated
//! `fhir_types::<version>` types: `surface!(r4b, "4.3.0", "R4B", to_r4b)`
//! expands to `parameters`, `resources`, `map`, `metadata`, `system`,
//! `operations`, and `router()`. The `map` module comes from one of two
//! family macros: `map_r4` for R4 and R4B, `map_r5` for R5 and the R6 ballot,
//! which share their result shapes and differ in a few inputs.

/// The FHIR versions this server serves, each with the modules of its surface.
///
/// A persisted resource records the version it arrived in, so a rebuild of the
/// served layer converts it with that version's own codec and converter.
pub const VERSIONS: [&str; 4] = [
    crate::r4::metadata::FHIR_VERSION,
    crate::r4b::metadata::FHIR_VERSION,
    crate::r5::metadata::FHIR_VERSION,
    crate::r6::metadata::FHIR_VERSION,
];

/// The model of a resource stored as a JSON object of `fhir_version`.
///
/// # Errors
///
/// Returns what the codec or the conversion refused, as text, and says so when
/// `fhir_version` is not one this server serves.
pub fn loaded_of(
    fhir_version: &str,
    object: &fhir_types::codec::Object,
) -> Result<crate::scope::Loaded, String> {
    if fhir_version == crate::r4::metadata::FHIR_VERSION {
        crate::r4::resources::model_of(object)
    } else if fhir_version == crate::r4b::metadata::FHIR_VERSION {
        crate::r4b::resources::model_of(object)
    } else if fhir_version == crate::r5::metadata::FHIR_VERSION {
        crate::r5::resources::model_of(object)
    } else if fhir_version == crate::r6::metadata::FHIR_VERSION {
        crate::r6::resources::model_of(object)
    } else {
        Err(format!(
            "`{fhir_version}` is not a FHIR version this server serves"
        ))
    }
}

pub(crate) mod batch;
pub(crate) mod closure;
pub(crate) mod map_r4;
pub(crate) mod map_r5;
pub(crate) mod metadata;
pub(crate) mod operations;
pub(crate) mod parameters;
pub(crate) mod resources;
pub(crate) mod store;
pub(crate) mod system;

macro_rules! surface {
    // A version whose package declares no `ConceptMap/$closure`: the R6 ballot
    // ships no OperationDefinition for it, so this server offers none.
    ($fhir:ident, $fhir_version:literal, $label:literal, $capabilities:ident) => {
        crate::version::surface!(@common $fhir, $fhir_version, $label, $capabilities);

        /// This version declares no `$closure`, so its router adds no route for it.
        fn closure_route(
            router: axum::Router<std::sync::Arc<crate::state::AppState>>,
        ) -> axum::Router<std::sync::Arc<crate::state::AppState>> {
            router
        }

        /// The canonical of `$closure`, for a version that declares it; none here.
        pub(crate) const CLOSURE_URL: Option<&str> = None;
    };
    ($fhir:ident, $fhir_version:literal, $label:literal, $capabilities:ident, closure) => {
        crate::version::surface!(@common $fhir, $fhir_version, $label, $capabilities);
        crate::version::closure::closure!($fhir);

        /// The `$closure` route of a version that declares the operation.
        fn closure_route(
            router: axum::Router<std::sync::Arc<crate::state::AppState>>,
        ) -> axum::Router<std::sync::Arc<crate::state::AppState>> {
            router.route("/$closure", axum::routing::post(closure::closure))
        }

        /// The canonical of `$closure`, which this version declares.
        pub(crate) const CLOSURE_URL: Option<&str> = Some(closure::CLOSURE_URL);
    };
    (@common $fhir:ident, $fhir_version:literal, $label:literal, $capabilities:ident) => {
        crate::version::parameters::parameters!($fhir);
        crate::version::resources::resources!($fhir);
        crate::version::metadata::metadata!($fhir, $fhir_version, $label, $capabilities);
        crate::version::system::system!($fhir);
        crate::version::store::store!($fhir);
        crate::version::batch::batch!($fhir);
        crate::version::operations::operations!($fhir);

        /// The routes of this version, nested under its root by the crate router.
        pub fn router() -> axum::Router<std::sync::Arc<crate::state::AppState>> {
            use axum::routing::{get, post};
            closure_route(axum::Router::new())
                .route("/", post(batch::batch))
                .route("/metadata", get(metadata::metadata))
                .route("/$versions", get(system::versions))
                .route("/$cache-control", post(system::cache_control))
                .route(
                    "/CodeSystem",
                    get(store::code_system_search).post(store::code_system_create),
                )
                .route(
                    "/CodeSystem/{id}",
                    get(store::code_system_read)
                        .put(store::code_system_update)
                        .delete(store::code_system_delete),
                )
                .route(
                    "/CodeSystem/{id}/_history/{version}",
                    get(store::code_system_version_read),
                )
                .route(
                    "/ValueSet",
                    get(store::value_set_search).post(store::value_set_create),
                )
                .route(
                    "/ValueSet/{id}",
                    get(store::value_set_read)
                        .put(store::value_set_update)
                        .delete(store::value_set_delete),
                )
                .route(
                    "/ValueSet/{id}/_history/{version}",
                    get(store::value_set_version_read),
                )
                .route(
                    "/ConceptMap",
                    get(store::concept_map_search).post(store::concept_map_create),
                )
                .route(
                    "/ConceptMap/{id}",
                    get(store::concept_map_read)
                        .put(store::concept_map_update)
                        .delete(store::concept_map_delete),
                )
                .route(
                    "/ConceptMap/{id}/_history/{version}",
                    get(store::concept_map_version_read),
                )
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
                    "/ValueSet/$batch-validate-code",
                    post(operations::batch_validate_code_post),
                )
                .route(
                    "/CodeSystem/$batch-validate-code",
                    post(operations::batch_validate_code_post),
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
    };
}

pub(crate) use surface;
