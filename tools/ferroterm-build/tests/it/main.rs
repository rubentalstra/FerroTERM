//! The one integration-test binary for `ferroterm-build`; one module per topic.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test assertions"
)]

mod archive;
mod classification;
mod fixture;
mod local_edition;
mod loinc;
mod pipeline;
mod rxnorm;
