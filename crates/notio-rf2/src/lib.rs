//! SNOMED CT RF2 loading and the typed component model.
//!
//! Streams the RF2 release files (concepts, descriptions, inferred
//! relationships, reference sets, and the transitive-closure file) into typed
//! components keyed by distinct identifier newtypes. Everything downstream
//! (the graph, the store, the text index) is built from this crate's output
//! by `notio-build`, offline, once per edition.
#![doc(test(attr(deny(warnings))))]
