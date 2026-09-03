//! Version negotiation for the systems and value sets an operation touches.
//!
//! The `system-version`, `check-system-version`, and `force-system-version`
//! parameters (R4 onwards on `$expand`, pre-adopted from R6 on
//! `ValueSet/$validate-code`) and their value set twins
//! `default-valueset-version`, `check-valueset-version`, and
//! `force-valueset-version` (R6, pre-adopted) name a version per canonical: a
//! default when the reference names none, a check that refuses a differing
//! reference, and a force that overrides it
//! (<https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-validate-code.html>,
//! <https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-expand.html>). One
//! negotiation applies to the value set named by the request, to every value
//! set it imports, and to every system any of them selects from.

use crate::compose::{Compose, Include};

/// A version negotiation could not be honoured.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NegotiationError {
    /// A `check-system-version` names one version, the value set another.
    #[error(
        "The version '{named}' is not allowed for system '{url}': required to be '{checked}' by a version-check parameter"
    )]
    SystemVersion {
        /// The system.
        url: String,
        /// The version checked for.
        checked: String,
        /// The version the value set names.
        named: String,
    },
    /// A `check-valueset-version` names one version, the reference another.
    #[error(
        "`check-valueset-version` names `{url}|{checked}` but the value set is referenced as version `{named}`"
    )]
    ValueSetVersion {
        /// The value set.
        url: String,
        /// The version checked for.
        checked: String,
        /// The version the reference names.
        named: String,
    },
}

/// The canonicals of a version parameter as `(url, version)` pairs.
#[must_use]
pub fn canonicals(list: &[String]) -> Vec<(String, Option<String>)> {
    list.iter()
        .map(|c| match c.split_once('|') {
            Some((url, version)) => (url.to_owned(), Some(version.to_owned())),
            None => (c.clone(), None),
        })
        .collect()
}

/// The defaults, checks, and forced versions of one kind of canonical.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Rules {
    defaults: Vec<(String, Option<String>)>,
    checks: Vec<(String, Option<String>)>,
    forced: Vec<(String, Option<String>)>,
}

impl Rules {
    fn is_empty(&self) -> bool {
        self.defaults.is_empty() && self.checks.is_empty() && self.forced.is_empty()
    }

    /// The version to use for `url` when the reference names `named`.
    fn version<E>(
        &self,
        url: &str,
        named: Option<&str>,
        mismatch: impl Fn(String, String) -> E,
    ) -> Result<Option<String>, E> {
        for (checked_url, checked) in &self.checks {
            if checked_url == url
                && let Some(named) = named
                && let Some(checked) = checked
                && !crate::versioned::version_matches(checked, named)
            {
                return Err(mismatch(checked.clone(), named.to_owned()));
            }
        }
        let mut version = named.map(str::to_owned);
        if version.is_none()
            && let Some((_, default)) = self
                .defaults
                .iter()
                .chain(&self.checks)
                .find(|(u, _)| u == url)
        {
            version.clone_from(default);
        }
        if let Some((_, forced)) = self.forced.iter().find(|(u, _)| u == url) {
            version.clone_from(forced);
        }
        Ok(version)
    }
}

/// The version negotiation of one request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Negotiation {
    systems: Rules,
    value_sets: Rules,
}

impl Negotiation {
    /// The negotiation the six parameters describe, each a list of
    /// `url|version` canonicals.
    #[must_use]
    pub fn new(
        system_version: &[String],
        check_system_version: &[String],
        force_system_version: &[String],
        default_valueset_version: &[String],
        check_valueset_version: &[String],
        force_valueset_version: &[String],
    ) -> Self {
        Self {
            systems: Rules {
                defaults: canonicals(system_version),
                checks: canonicals(check_system_version),
                forced: canonicals(force_system_version),
            },
            value_sets: Rules {
                defaults: canonicals(default_valueset_version),
                checks: canonicals(check_valueset_version),
                forced: canonicals(force_valueset_version),
            },
        }
    }

    /// Whether no parameter was given.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty() && self.value_sets.is_empty()
    }

    /// The version to use for system `url` when the reference names `named`.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::SystemVersion`] when a check disagrees with
    /// the named version.
    pub fn system_version(
        &self,
        url: &str,
        named: Option<&str>,
    ) -> Result<Option<String>, NegotiationError> {
        self.systems.version(url, named, |checked, named| {
            NegotiationError::SystemVersion {
                url: url.to_owned(),
                checked,
                named,
            }
        })
    }

    /// The version to use for value set `url` when the reference names
    /// `named`; `url` may carry its version as `url|version`.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::ValueSetVersion`] when a check disagrees
    /// with the named version.
    pub fn value_set(
        &self,
        url: &str,
        named: Option<&str>,
    ) -> Result<(String, Option<String>), NegotiationError> {
        let (url, embedded) = match url.split_once('|') {
            Some((url, version)) => (url, Some(version)),
            None => (url, None),
        };
        let version = self
            .value_sets
            .version(url, named.or(embedded), |checked, named| {
                NegotiationError::ValueSetVersion {
                    url: url.to_owned(),
                    checked,
                    named,
                }
            })?;
        Ok((url.to_owned(), version))
    }

    /// The version parameter that would apply to system `url` when the
    /// reference names `named`, without running the checks: the forced one,
    /// else the named one, else a default; `None` when nothing names one.
    #[must_use]
    pub fn system_literal(&self, url: &str, named: Option<&str>) -> Option<String> {
        if let Some((_, forced)) = self.systems.forced.iter().find(|(u, _)| u == url) {
            return forced.clone();
        }
        if named.is_some() {
            return named.map(str::to_owned);
        }
        self.systems
            .defaults
            .iter()
            .chain(&self.systems.checks)
            .find(|(u, _)| u == url)
            .and_then(|(_, v)| v.clone())
    }

    /// Whether the checks allow `version` for system `url`.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::SystemVersion`] when a check disagrees.
    pub fn check_system(&self, url: &str, version: &str) -> Result<(), NegotiationError> {
        for (checked_url, checked) in &self.systems.checks {
            if checked_url == url
                && let Some(checked) = checked
                && !crate::versioned::version_matches(checked, version)
            {
                return Err(NegotiationError::SystemVersion {
                    url: url.to_owned(),
                    checked: checked.clone(),
                    named: version.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// `compose` with every system reference at its negotiated version.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::SystemVersion`] when a check disagrees with
    /// a version the compose names.
    pub fn pin(&self, compose: &Compose) -> Result<Compose, NegotiationError> {
        if self.systems.is_empty() {
            return Ok(compose.clone());
        }
        let pin = |include: &Include| -> Result<Include, NegotiationError> {
            let mut pinned = include.clone();
            if let Some(system) = pinned.system.as_mut() {
                system.version = self.system_version(&system.url, system.version.as_deref())?;
            }
            Ok(pinned)
        };
        Ok(Compose {
            include: compose.include.iter().map(pin).collect::<Result<_, _>>()?,
            exclude: compose.exclude.iter().map(pin).collect::<Result<_, _>>()?,
            inactive: compose.inactive,
        })
    }

    /// `compose` with every system reference at its negotiated version, the
    /// checks left to the caller (validation reports a disagreement as an
    /// itemised issue, never as a refusal).
    #[must_use]
    pub fn pin_lenient(&self, compose: &Compose) -> Compose {
        if self.systems.is_empty() {
            return compose.clone();
        }
        let pin = |include: &Include| -> Include {
            let mut pinned = include.clone();
            if let Some(system) = pinned.system.as_mut() {
                system.version = self.system_literal(&system.url, system.version.as_deref());
            }
            pinned
        };
        Compose {
            include: compose.include.iter().map(pin).collect(),
            exclude: compose.exclude.iter().map(pin).collect(),
            inactive: compose.inactive,
        }
    }
}
