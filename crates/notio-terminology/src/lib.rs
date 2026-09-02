//! The terminology engine.
//!
//! Implements the FHIR terminology operations (`$lookup`, `$validate-code`,
//! `$expand`, `$subsumes`, `$translate`) over the store, the graph, the text
//! index, and the ECL evaluator, dispatched per FHIR version against the
//! generated `notio-fhir` contracts.
#![doc(test(attr(deny(warnings))))]
