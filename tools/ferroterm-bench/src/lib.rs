//! What the two measurement binaries share.
//!
//! `ferroterm-bench` writes a record per code system and `ferroterm-residency`
//! measures one structure of an artifact; both have to read how much memory a
//! process holds, and reading it two different ways is how one of them came to
//! understate it. No specification governs a benchmark: our own design.

pub mod memory;
