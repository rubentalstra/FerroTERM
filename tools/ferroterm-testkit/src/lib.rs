//! Synthetic fixtures for the FerroTERM test suites.
//!
//! Shared by the crates' integration tests as a dev-dependency; nothing here
//! ships. Every fixture is invented content shaped like the real thing.
#![doc(test(attr(deny(warnings))))]
// The synthetic CodeSystem literals nest deeper than the default macro budget.
#![recursion_limit = "256"]

pub mod atc;
pub mod classification;
pub mod dhd;
pub mod fhir;
pub mod gstandaard;
pub mod icd11;
pub mod loinc;
pub mod rxnorm;
pub mod snomed;
