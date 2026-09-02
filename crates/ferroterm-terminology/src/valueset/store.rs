//! The value sets a server holds, by `url` and `version`, and the resolver
//! that answers `include.valueSet` references over them.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use super::model::ValueSetModel;
use crate::compose::{Compose, ComposeError, Expander, Expansion, Options, ValueSetResolver};
use crate::registry::Registry;

/// Two value sets with the same `url` and `version`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("value set `{canonical}` is already stored")]
pub struct DuplicateValueSet {
    /// The `url|version`.
    pub canonical: String,
}

/// The value sets by `url`, then `version` (`""` for none).
#[derive(Debug, Default, Clone)]
pub struct ValueSetStore {
    by_url: BTreeMap<String, BTreeMap<String, Arc<ValueSetModel>>>,
}

impl ValueSetStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `model`.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateValueSet`] when the same `url` and `version` is stored.
    pub fn insert(&mut self, model: ValueSetModel) -> Result<(), DuplicateValueSet> {
        let versions = self.by_url.entry(model.url.clone()).or_default();
        let key = model.version.clone().unwrap_or_default();
        if versions.contains_key(&key) {
            return Err(DuplicateValueSet {
                canonical: model.canonical(),
            });
        }
        versions.insert(key, Arc::new(model));
        Ok(())
    }

    /// The value set at `url`, at `version` or the default version.
    ///
    /// `url` may carry its version as `url|version`. No FHIR version fixes
    /// which version is the default: the greatest, by numeric-aware
    /// comparison of dot-separated segments, is our own design.
    #[must_use]
    pub fn resolve(&self, url: &str, version: Option<&str>) -> Option<Arc<ValueSetModel>> {
        let (url, embedded) = match url.split_once('|') {
            Some((url, version)) => (url, Some(version)),
            None => (url, None),
        };
        let versions = self.by_url.get(url)?;
        match version.or(embedded) {
            Some(version) => versions.get(version).cloned(),
            None => versions
                .iter()
                .max_by(|(a, _), (b, _)| version_order(a, b))
                .map(|(_, model)| Arc::clone(model)),
        }
    }

    /// This store with `overlay`'s value sets on top; an overlay version wins.
    #[must_use]
    pub fn layered(&self, overlay: &Self) -> Self {
        let mut merged = self.clone();
        for (url, versions) in &overlay.by_url {
            let target = merged.by_url.entry(url.clone()).or_default();
            for (version, model) in versions {
                target.insert(version.clone(), Arc::clone(model));
            }
        }
        merged
    }

    /// Every stored value set, by `url` then `version`.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<ValueSetModel>> {
        self.by_url.values().flat_map(|versions| versions.values())
    }

    /// The number of stored value sets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_url.values().map(BTreeMap::len).sum()
    }

    /// Whether nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_url.is_empty()
    }
}

/// Orders version strings segment by segment, numerically where both
/// segments are numbers, else lexically.
fn version_order(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    _ => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

/// Resolves `include.valueSet` references from a store and the providers'
/// implicit value sets, refusing a cycle.
#[derive(Debug)]
pub struct Resolver<'a> {
    registry: &'a Registry,
    store: &'a ValueSetStore,
    active: RefCell<Vec<String>>,
}

impl<'a> Resolver<'a> {
    /// A resolver over `registry` and `store`.
    #[must_use]
    pub fn new(registry: &'a Registry, store: &'a ValueSetStore) -> Self {
        Self {
            registry,
            store,
            active: RefCell::new(Vec::new()),
        }
    }

    /// The compose of the value set at `url`: stored, or implicit.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError::UnknownValueSet`] when neither knows it, or the
    /// provider's error for a malformed implicit form.
    pub fn compose(&self, url: &str) -> Result<Compose, ComposeError> {
        if let Some(model) = self.store.resolve(url, None) {
            return Ok(model.compose.clone());
        }
        match self.registry.implicit_value_set(url) {
            Some(Ok(compose)) => Ok(compose),
            Some(Err(source)) => Err(ComposeError::Provider {
                system: url.to_owned(),
                source,
            }),
            None => Err(ComposeError::UnknownValueSet(url.to_owned())),
        }
    }

    /// Expands the value set at `url` under `options`, tracking the reference
    /// chain so a value set that reaches itself is an error.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] when the value set is unknown, cyclic, or fails
    /// to expand.
    pub fn expand_with(&self, url: &str, options: &Options) -> Result<Expansion, ComposeError> {
        let compose = self.compose(url)?;
        self.expand_compose(url, &compose, options)
    }

    /// Expands `compose`, named `url` for cycle detection, under `options`.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] for a cycle or an expansion failure.
    pub fn expand_compose(
        &self,
        url: &str,
        compose: &Compose,
        options: &Options,
    ) -> Result<Expansion, ComposeError> {
        if self.active.borrow().iter().any(|active| active == url) {
            return Err(ComposeError::Cycle(url.to_owned()));
        }
        self.active.borrow_mut().push(url.to_owned());
        let result = Expander::with_resolver(self.registry, self).expand(compose, options);
        self.active.borrow_mut().pop();
        result
    }
}

impl ValueSetResolver for Resolver<'_> {
    fn expand(&self, url: &str) -> Result<Expansion, ComposeError> {
        self.expand_with(url, &Options::default())
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::version_order;

    #[test]
    fn versions_order_numerically_by_segment() {
        assert_eq!(version_order("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(version_order("2.0", "2.0.1"), Ordering::Less);
        assert_eq!(version_order("2024", "2025"), Ordering::Less);
        assert_eq!(version_order("b", "a"), Ordering::Greater);
        assert_eq!(version_order("", "1"), Ordering::Less);
    }
}
