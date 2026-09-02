//! The memory-mapped concept and description store.
//!
//! A `redb` database holding the columnar concept, description, and refset
//! member records the terminology operations read point-wise: `$lookup` and
//! `$validate-code` resolve a code here without touching the graph. Built
//! offline by `notio-build`, opened read-only by the server.
#![doc(test(attr(deny(warnings))))]
