//! The materialized ontology.
//!
//! Integer-keyed compressed sparse row adjacency for the is-a hierarchy and
//! each attribute type, plus roaring transitive-closure bitmaps. Subsumption
//! is a bitmap membership test, and the ECL evaluator compiles constraints to
//! set algebra over these bitmaps. The graph is built offline and served
//! read-only; no request traverses edges live.
#![doc(test(attr(deny(warnings))))]
