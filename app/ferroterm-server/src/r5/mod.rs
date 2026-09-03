//! FHIR R5 (5.0.0) under `/r5`: the generated `fhir_types::r5` contracts
//! over the engine (<https://hl7.org/fhir/R5/terminology-service.html>).
//!
//! R5 declares shapes the R4 family does not (`$validate-code` `issues` and
//! the validated `code`, `system`, and `version`; `$lookup` `definition`;
//! `$expand` `property` and `useSupplement`; the renamed `$translate`
//! parameters and `relationship`), so its `map` is its own module; the rest
//! of the surface is the shared per-version instantiation.

pub mod map;

crate::version::surface!(r5, "5.0.0", "R5", to_r5);
