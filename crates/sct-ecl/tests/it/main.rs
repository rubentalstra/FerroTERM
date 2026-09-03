//! Integration tests: the vendored corpus, the grammar's refusals, and the
//! print round trip.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test assertions"
)]

mod corpus;
mod grammar;
mod roundtrip;
