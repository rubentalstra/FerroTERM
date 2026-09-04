//! The one integration-test binary for `ferroterm-server`; one module per topic.
//!
//! The FHIR tests drive the router with `tower::ServiceExt::oneshot` over a
//! synthetic SNOMED edition written by `ferroterm-testkit`; no socket, no
//! SNOMED content.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test assertions"
)]

mod config;
mod ecosystem;
mod fixture;
mod health;
mod metadata;
mod operations;
mod r4;
mod r5;
mod r6;
mod scope;
mod shutdown;
mod telemetry;
mod translate;
mod value_set;
