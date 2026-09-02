//! The materialized hierarchy.
//!
//! Integer-keyed compressed sparse row adjacency for a code system's is-a
//! hierarchy and each typed relationship, plus roaring transitive-closure
//! bitmaps. Subsumption is a bitmap membership test, and the ECL evaluator
//! compiles constraints to set algebra over these bitmaps. The graph is built
//! offline by a loader and served read-only; no request traverses edges live.
#![doc(test(attr(deny(warnings))))]
