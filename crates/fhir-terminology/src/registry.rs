//! The loaded code system versions, by system URI, with the default-version rule.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::compose::Compose;
use crate::provider::{CodeSystemProvider, ProviderError};

/// A failure to resolve a system or version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// No provider serves the system.
    #[error("unknown code system `{0}`")]
    UnknownSystem(String),
    /// The system is served, this version is not.
    #[error("unknown version `{version}` of code system `{url}`")]
    UnknownVersion {
        /// The system.
        url: String,
        /// The version asked for.
        version: String,
    },
}

/// A failure to register a provider.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// The same system version is already registered.
    #[error("code system `{url}` version `{version}` is already registered")]
    Duplicate {
        /// The system.
        url: String,
        /// The version.
        version: String,
    },
}

/// A resolved provider and how its version was chosen.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The provider.
    pub provider: Arc<dyn CodeSystemProvider>,
    /// Whether the version came from the default rule rather than the request.
    pub defaulted: bool,
}

#[derive(Debug, Default, Clone)]
struct System {
    versions: BTreeMap<String, Arc<dyn CodeSystemProvider>>,
    default: Option<String>,
}

/// The providers a server has loaded.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    systems: BTreeMap<String, System>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a provider.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Duplicate`] when that system version is present.
    pub fn register(&mut self, provider: Arc<dyn CodeSystemProvider>) -> Result<(), RegisterError> {
        let (url, version) = {
            let identity = provider.identity();
            (identity.url.clone(), identity.version.clone())
        };
        let system = self.systems.entry(url.clone()).or_default();
        if system.versions.contains_key(&version) {
            return Err(RegisterError::Duplicate { url, version });
        }
        system.versions.insert(version, provider);
        Ok(())
    }

    /// Adds a provider, replacing the one already registered for that system
    /// version (a request-scoped resource shadows a loaded one).
    pub fn register_or_replace(&mut self, provider: Arc<dyn CodeSystemProvider>) {
        let (url, version) = {
            let identity = provider.identity();
            (identity.url.clone(), identity.version.clone())
        };
        self.systems
            .entry(url)
            .or_default()
            .versions
            .insert(version, provider);
    }

    /// Configures the version a request without one resolves to.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] when the system or version is not registered.
    pub fn set_default(&mut self, url: &str, version: &str) -> Result<(), ResolveError> {
        let system = self
            .systems
            .get_mut(url)
            .ok_or_else(|| ResolveError::UnknownSystem(url.to_owned()))?;
        if !system.versions.contains_key(version) {
            return Err(ResolveError::UnknownVersion {
                url: url.to_owned(),
                version: version.to_owned(),
            });
        }
        system.default = Some(version.to_owned());
        Ok(())
    }

    /// The default version of a system: the configured one, else the greatest
    /// version string.
    ///
    /// The greatest-string rule is our own design for an unconfigured system;
    /// the FHIR terminology service only asks that the resolved version is
    /// echoed (<https://hl7.org/fhir/R4B/terminology-service.html>).
    #[must_use]
    pub fn default_version(&self, url: &str) -> Option<&str> {
        let system = self.systems.get(url)?;
        system
            .default
            .as_deref()
            .or_else(|| system.versions.keys().next_back().map(String::as_str))
    }

    /// Resolves a system and optional version to a provider.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] for an unknown system or version.
    pub fn resolve(&self, url: &str, version: Option<&str>) -> Result<Resolved, ResolveError> {
        let system = self
            .systems
            .get(url)
            .ok_or_else(|| ResolveError::UnknownSystem(url.to_owned()))?;
        let (wanted, defaulted) = match version {
            Some(version) => (version, false),
            None => (
                self.default_version(url)
                    .ok_or_else(|| ResolveError::UnknownSystem(url.to_owned()))?,
                true,
            ),
        };
        // NOTE: `1.0.x` and `1.x.x` name the greatest matching version, the
        // ecosystem's form (`crate::versioned::version_matches`).
        let provider =
            crate::versioned::select_version(system.versions.keys().map(String::as_str), wanted)
                .and_then(|v| system.versions.get(v))
                .ok_or_else(|| ResolveError::UnknownVersion {
                    url: url.to_owned(),
                    version: wanted.to_owned(),
                })?;
        Ok(Resolved {
            provider: Arc::clone(provider),
            defaulted,
        })
    }

    /// The compose an implicit value set URI denotes, asking the default
    /// version of every system whose URI prefixes `url`.
    ///
    /// `None` when no system claims the URI.
    #[must_use]
    pub fn implicit_value_set(&self, url: &str) -> Option<Result<Compose, ProviderError>> {
        self.systems
            .keys()
            .filter(|system| url.starts_with(system.as_str()))
            .filter_map(|system| self.resolve(system, None).ok())
            .find_map(|resolved| resolved.provider.implicit_value_set(url))
    }

    /// The registered system URIs, sorted.
    pub fn systems(&self) -> impl Iterator<Item = &str> {
        self.systems.keys().map(String::as_str)
    }

    /// The registered versions of a system, sorted, with the provider.
    pub fn versions(&self, url: &str) -> impl Iterator<Item = &Arc<dyn CodeSystemProvider>> {
        self.systems
            .get(url)
            .into_iter()
            .flat_map(|system| system.versions.values())
    }
}
