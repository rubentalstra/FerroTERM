//! The one integration-test binary for `fhir-terminology`; one module per topic.
//!
//! Every test drives the seam through a synthetic in-memory provider
//! (`fixture`); nothing here knows a real code system.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test assertions"
)]

mod capabilities;
mod classification;
mod compose;
mod concept_map;
mod ecl;
mod fhir_codesystem;
mod filter;
mod fixture;
mod icd11;
mod labcodeset;
mod loinc;
mod operations;
mod registries;
mod registry;
mod rxnorm;
mod snomed;
mod supplement;
mod ucum;
mod value_set;
