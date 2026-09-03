//! A store of canonical resources by `url` and `version`, with the
//! default-version rule the value sets and concept maps share.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A resource identified by a canonical `url` and an optional `version`.
pub trait Versioned {
    /// `url`.
    fn url(&self) -> &str;
    /// `version`.
    fn version(&self) -> Option<&str>;

    /// The canonical `url|version`, or `url` alone.
    fn canonical(&self) -> String {
        match self.version() {
            Some(version) => format!("{}|{version}", self.url()),
            None => self.url().to_owned(),
        }
    }
}

/// Two resources with the same `url` and `version`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{canonical}` is already stored")]
pub struct Duplicate {
    /// The `url|version`.
    pub canonical: String,
}

/// The resources by `url`, then `version` (`""` for none).
#[derive(Debug)]
pub struct VersionedStore<T> {
    by_url: BTreeMap<String, BTreeMap<String, Arc<T>>>,
}

impl<T> Default for VersionedStore<T> {
    fn default() -> Self {
        Self {
            by_url: BTreeMap::new(),
        }
    }
}

impl<T> Clone for VersionedStore<T> {
    fn clone(&self) -> Self {
        Self {
            by_url: self.by_url.clone(),
        }
    }
}

impl<T: Versioned> VersionedStore<T> {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `resource`.
    ///
    /// # Errors
    ///
    /// Returns [`Duplicate`] when the same `url` and `version` is stored.
    pub fn insert(&mut self, resource: T) -> Result<(), Duplicate> {
        let versions = self.by_url.entry(resource.url().to_owned()).or_default();
        let key = resource.version().unwrap_or_default().to_owned();
        if versions.contains_key(&key) {
            return Err(Duplicate {
                canonical: resource.canonical(),
            });
        }
        versions.insert(key, Arc::new(resource));
        Ok(())
    }

    /// Stores `resource`, replacing the one with the same `url` and `version`.
    pub fn replace(&mut self, resource: T) {
        let key = resource.version().unwrap_or_default().to_owned();
        self.by_url
            .entry(resource.url().to_owned())
            .or_default()
            .insert(key, Arc::new(resource));
    }

    /// The resource at `url`, at `version` or the default version.
    ///
    /// `url` may carry its version as `url|version`. No FHIR version fixes
    /// which version is the default: the greatest, by numeric-aware
    /// comparison of dot-separated segments, is our own design.
    #[must_use]
    pub fn resolve(&self, url: &str, version: Option<&str>) -> Option<Arc<T>> {
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
                .map(|(_, resource)| Arc::clone(resource)),
        }
    }

    /// This store with `overlay`'s resources on top; an overlay version wins.
    #[must_use]
    pub fn layered(&self, overlay: &Self) -> Self {
        let mut merged = self.clone();
        for (url, versions) in &overlay.by_url {
            let target = merged.by_url.entry(url.clone()).or_default();
            for (version, resource) in versions {
                target.insert(version.clone(), Arc::clone(resource));
            }
        }
        merged
    }

    /// Every stored resource, by `url` then `version`.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<T>> {
        self.by_url.values().flat_map(|versions| versions.values())
    }

    /// The number of stored resources.
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
