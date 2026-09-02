//! The one integration-test binary for `ferroterm-server`; one module per topic.
#![allow(clippy::panic_in_result_fn, reason = "test assertions")]

mod health;
mod shutdown;
