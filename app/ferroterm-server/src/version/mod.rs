//! The FHIR surface of one served version, instantiated per version.
//!
//! R4 (4.0.1) and R4B (4.3.0) declare the same terminology operation
//! parameters and the same `CapabilityStatement`, `TerminologyCapabilities`,
//! `ValueSet`, and `Parameters` elements this server fills, so one set of
//! macros produces each version's modules from the generated
//! `ferroterm_fhir::<version>` types: `surface!(r4b, "4.3.0", "R4B", to_r4b)`
//! expands to `parameters`, `resources`, `map`, `metadata`, `system`,
//! `operations`, and `router()`. A version that declares a different shape
//! (R5 and later) gets its own modules.

pub(crate) mod map;
pub(crate) mod metadata;
pub(crate) mod operations;
pub(crate) mod parameters;
pub(crate) mod resources;
pub(crate) mod system;

macro_rules! surface {
    ($fhir:ident, $fhir_version:literal, $label:literal, $capabilities:ident) => {
        crate::version::parameters::parameters!($fhir);
        crate::version::resources::resources!($fhir);
        crate::version::map::map!($fhir);
        crate::version::metadata::metadata!($fhir, $fhir_version, $label, $capabilities);
        crate::version::system::system!($fhir);
        crate::version::operations::operations!($fhir);

        /// The routes of this version, nested under its root by the crate router.
        pub fn router() -> axum::Router<std::sync::Arc<crate::state::AppState>> {
            use axum::routing::{get, post};
            axum::Router::new()
                .route("/metadata", get(metadata::metadata))
                .route("/$versions", get(system::versions))
                .route("/$cache-control", post(system::cache_control))
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
    };
}

pub(crate) use surface;
