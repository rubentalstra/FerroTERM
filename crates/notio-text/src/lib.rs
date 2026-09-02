//! The description search index.
//!
//! An `fst` term dictionary over description words plus roaring posting
//! bitmaps per word, with refset and status filters, sorted by matched term
//! length. This is the `$expand` `filter` and `$find-matches` engine.
#![doc(test(attr(deny(warnings))))]
