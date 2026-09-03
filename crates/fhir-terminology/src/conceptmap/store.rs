//! The concept maps a server holds, by `url` and `version`.

use super::model::ConceptMapModel;
use crate::versioned::VersionedStore;

/// The concept maps by `url`, then `version`.
pub type ConceptMapStore = VersionedStore<ConceptMapModel>;
