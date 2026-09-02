//! The one integration-test binary for `ferroterm-graph`; one module per topic.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test assertions"
)]

mod local_edition;
mod properties;
