//! The terminology engine.
//!
//! Implements the FHIR terminology operations (`$lookup`, `$validate-code`,
//! `$expand`, `$subsumes`, `$translate`) over the code system provider seam,
//! dispatched per FHIR version against the generated `ferroterm-fhir` contracts.
//! SNOMED CT is the first provider; every other code system reaches the
//! operations through the same seam.
#![doc(test(attr(deny(warnings))))]
