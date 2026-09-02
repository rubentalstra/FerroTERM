//! The loaded server state: a registry of code system providers and the ids
//! their `CodeSystem` instances answer on.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use ferroterm_terminology::provider::CodeSystemProvider;
use ferroterm_terminology::registry::{RegisterError, Registry, Resolved};
use ferroterm_terminology::snomed::{OpenError, SnomedProvider};

use crate::config::Config;

/// A failure to load the state.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// An artifact directory does not open.
    #[error("cannot open the artifact at {path}")]
    Open {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<OpenError>,
    },
    /// Two artifacts serve the same system version.
    #[error(transparent)]
    Register(#[from] RegisterError),
}

/// What the handlers share.
#[derive(Debug)]
pub struct AppState {
    registry: Registry,
    /// `CodeSystem` instance id to (system, version).
    instances: BTreeMap<String, (String, String)>,
    /// The software version reported in the capability statements.
    software_version: &'static str,
}

impl AppState {
    /// Loads every artifact `config` names into a registry.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when an artifact does not open or two artifacts
    /// serve the same system version. A server never starts on a bad index.
    pub fn load(config: &Config) -> Result<Self, LoadError> {
        let mut registry = Registry::new();
        for path in &config.index {
            let provider =
                SnomedProvider::open(path, &config.default_language).map_err(|source| {
                    LoadError::Open {
                        path: path.clone(),
                        source: Box::new(source),
                    }
                })?;
            registry.register(Arc::new(provider))?;
        }
        Ok(Self::from_registry(registry))
    }

    /// Wraps an already-built registry (tests and embedders).
    #[must_use]
    pub fn from_registry(registry: Registry) -> Self {
        let mut instances = BTreeMap::new();
        for url in registry.systems() {
            for provider in registry.versions(url) {
                let identity = provider.identity();
                instances.insert(
                    instance_id(&identity.version),
                    (identity.url.clone(), identity.version.clone()),
                );
            }
        }
        Self {
            registry,
            instances,
            software_version: env!("CARGO_PKG_VERSION"),
        }
    }

    /// The registry.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The `CodeSystem` instance ids and what they serve, sorted by id.
    pub fn instances(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        self.instances
            .iter()
            .map(|(id, (url, version))| (id.as_str(), url.as_str(), version.as_str()))
    }

    /// Resolves a `CodeSystem` instance id.
    #[must_use]
    pub fn instance(&self, id: &str) -> Option<Resolved> {
        let (url, version) = self.instances.get(id)?;
        self.registry.resolve(url, Some(version)).ok()
    }

    /// The software version.
    #[must_use]
    pub fn software_version(&self) -> &'static str {
        self.software_version
    }

    /// The default provider of a system, for callers that need one.
    #[must_use]
    pub fn provider(&self, url: &str) -> Option<Arc<dyn CodeSystemProvider>> {
        self.registry.resolve(url, None).ok().map(|r| r.provider)
    }
}

/// A FHIR resource id for a code system version: the version string reduced
/// to the id alphabet (`[A-Za-z0-9.-]`, at most 64 characters,
/// <https://hl7.org/fhir/R4B/datatypes.html#id>), scheme dropped.
///
/// No spec governs how a server names its instances: our own design.
#[must_use]
pub fn instance_id(version: &str) -> String {
    let stripped = version
        .strip_prefix("https://")
        .or_else(|| version.strip_prefix("http://"))
        .unwrap_or(version);
    let mut id = String::with_capacity(stripped.len());
    let mut dash = false;
    for c in stripped.chars() {
        if c.is_ascii_alphanumeric() || c == '.' {
            id.push(c);
            dash = false;
        } else if !dash && !id.is_empty() {
            id.push('-');
            dash = true;
        }
    }
    let trimmed = id.trim_end_matches('-');
    trimmed.chars().take(64).collect()
}

#[cfg(test)]
mod tests {
    use super::instance_id;

    #[test]
    fn instance_ids_fit_the_fhir_id_alphabet() {
        assert_eq!(
            instance_id("http://snomed.info/sct/11000146104/version/20260630"),
            "snomed.info-sct-11000146104-version-20260630"
        );
        assert_eq!(instance_id("2.80"), "2.80");
        assert!(instance_id(&"x".repeat(100)).len() <= 64);
    }
}
