//! The memory-mapped concept store.
//!
//! A `redb` database holding one code system version's concepts, displays,
//! designations, and property values, keyed by dense ordinal, that the
//! terminology operations read point-wise: `$lookup` and `$validate-code`
//! resolve a code here without touching the graph. Built offline by
//! `ferroterm-build`, opened read-only by the server.
#![doc(test(attr(deny(warnings))))]
