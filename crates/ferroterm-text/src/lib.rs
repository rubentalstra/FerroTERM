//! The designation search index.
//!
//! An `fst` term dictionary over designation words plus roaring posting
//! bitmaps per word, with language and designation-use filters, sorted by
//! matched term length. This is the `$expand` `filter` and `$find-matches`
//! engine for every loaded code system.
#![doc(test(attr(deny(warnings))))]
