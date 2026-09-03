//! The one integration-test binary for `ferroterm-store`; one module per topic.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "test assertions and one long local-edition build"
)]

mod artifact;
mod footprint;
mod local_edition;
