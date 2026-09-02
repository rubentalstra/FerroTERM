//! The terminology engine.
//!
//! Implements the FHIR terminology operations (`$lookup`, `$validate-code`,
//! `$expand`, `$subsumes`, `$translate`) over the code system provider seam,
//! dispatched per FHIR version against the generated `ferroterm-fhir` contracts.
//! SNOMED CT is the first provider; every other code system reaches the
//! operations through the same seam. No specification governs the seam
//! itself (our own design); the FHIR `CodeSystem` metadata is the capability
//! declaration a provider returns
//! (<https://hl7.org/fhir/R4B/codesystem.html>).
#![doc(test(attr(deny(warnings))))]

pub mod capabilities;
pub mod compose;
pub mod filter;
pub mod operations;
pub mod provider;
pub mod registry;
pub mod snomed;
pub mod supplement;
