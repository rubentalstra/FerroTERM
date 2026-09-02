//! The one integration-test binary for `ferroterm-terminology`; one module per topic.
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

mod artifact;
mod capabilities;
mod compose;
mod filter;
mod fixture;
mod registry;
mod snomed;
mod supplement;
