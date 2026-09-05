//! The memory-mapped concept store.
//!
//! A `redb` database holding one code system version's concepts, displays,
//! designations, and property values, keyed by dense ordinal, that the
//! terminology operations read point-wise: `$lookup` and `$validate-code`
//! resolve a code here without touching the graph. Built offline by
//! `ferroterm-build`, opened read-only by the server. No spec governs the
//! layout: our own design (`docs/architecture.md` decision 3).
#![doc(test(attr(deny(warnings))))]

pub mod builder;
pub mod column;
pub mod keys;
pub mod record;
pub mod store;
pub mod tables;
