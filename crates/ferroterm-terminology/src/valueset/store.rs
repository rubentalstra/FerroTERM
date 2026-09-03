//! The value sets a server holds, by `url` and `version`, and the resolver
//! that answers `include.valueSet` references over them.

use std::cell::RefCell;

use super::model::ValueSetModel;
use crate::compose::{Compose, ComposeError, Expander, Expansion, Item, Options, ValueSetResolver};
use crate::registry::Registry;
use crate::versioned::VersionedStore;

/// The value sets by `url`, then `version`.
pub type ValueSetStore = VersionedStore<ValueSetModel>;

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

impl Resolver<'_> {
    /// Whether `compose`, named `url` for cycle detection, contains `code` of
    /// `system`.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] for a cycle or a failure to evaluate.
    pub fn contains_compose(
        &self,
        url: &str,
        compose: &Compose,
        system: &str,
        version: Option<&str>,
        code: &str,
        language: Option<&str>,
    ) -> Result<Option<Item>, ComposeError> {
        if self.active.borrow().iter().any(|active| active == url) {
            return Err(ComposeError::Cycle(url.to_owned()));
        }
        self.active.borrow_mut().push(url.to_owned());
        let result = Expander::with_resolver(self.registry, self)
            .contains(compose, system, version, code, language);
        self.active.borrow_mut().pop();
        result
    }
}

impl ValueSetResolver for Resolver<'_> {
    fn expand(&self, url: &str) -> Result<Expansion, ComposeError> {
        self.expand_with(url, &Options::default())
    }

    fn contains(&self, url: &str, system: &str, code: &str) -> Result<Option<Item>, ComposeError> {
        let compose = self.compose(url)?;
        self.contains_compose(url, &compose, system, None, code, None)
    }
}
