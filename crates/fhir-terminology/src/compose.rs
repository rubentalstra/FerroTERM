//! The compose layer: `ValueSet.compose` evaluated once, above every provider.
//!
//! Includes union, criteria within an include intersect, excludes subtract,
//! and a concept appears once per system, version, and code
//! (<https://hl7.org/fhir/R4B/valueset.html#compositions>,
//! <https://hl7.org/fhir/R5/valueset.html#union-intersection>). Order is by
//! system, version, then the provider's concept order (the ordinal the build
//! assigns from sorted codes): no version fixes an expansion order, so this is
//! our own design, chosen so paging is a stable partition that costs nothing to
//! cut. Includes and excludes are bitmap algebra; only the page asked for is
//! read from the store.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::filter::Filter;
use crate::language;
use crate::provider::{CodeSystemProvider, Concept, ConceptSet, ProviderError};
use crate::registry::{Registry, ResolveError};

/// A system and optional version an include names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemRef {
    /// The system URI.
    pub url: String,
    /// The version, or the registry default.
    pub version: Option<String>,
}

/// One enumerated concept of an include.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptRef {
    /// The code.
    pub code: String,
    /// A display to use instead of the system's.
    pub display: Option<String>,
    /// Whether the value set marks the concept deprecated
    /// (`valueset-deprecated`).
    pub deprecated: bool,
}

/// `ValueSet.compose.include` (and `exclude`, which has the same shape).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Include {
    /// The system, when codes are selected from one.
    pub system: Option<SystemRef>,
    /// Enumerated concepts; empty means every code the filters admit.
    pub concepts: Vec<ConceptRef>,
    /// Filters, all of which apply.
    pub filters: Vec<Filter>,
    /// Value sets whose expansions the selection must be in, all of them.
    pub value_sets: Vec<String>,
}

/// `ValueSet.compose`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compose {
    /// The includes; their union.
    pub include: Vec<Include>,
    /// The excludes; subtracted.
    pub exclude: Vec<Include>,
    /// `compose.inactive`: `Some(false)` keeps inactive codes out of every expansion.
    pub inactive: Option<bool>,
}

/// The request-time controls of an expansion (`$expand` parameters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// `activeOnly`: drop inactive concepts.
    pub active_only: bool,
    /// `excludeNotForUI`: drop the abstract (`notSelectable`) groupers.
    pub exclude_not_for_ui: bool,
    /// `excludePostCoordinated`: drop post-coordinated expressions.
    pub exclude_post_coordinated: bool,
    /// `filter`: a text search every concept must match by a designation.
    pub text: Option<String>,
    /// `displayLanguage`: the language for `display`.
    pub language: Option<String>,
    /// `offset`.
    pub offset: usize,
    /// `count`; `None` is the whole expansion.
    pub count: Option<usize>,
}

/// One expansion entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The system.
    pub system: String,
    /// The version served.
    pub version: String,
    /// The code.
    pub code: String,
    /// The display.
    pub display: Option<String>,
    /// Whether the concept is inactive.
    pub inactive: bool,
    /// Whether the concept is abstract.
    pub abstract_concept: bool,
}

/// A code system version an expansion used, for `expansion.parameter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsedVersion {
    /// The system.
    pub url: String,
    /// The version.
    pub version: String,
    /// Whether the default rule chose it.
    pub defaulted: bool,
}

/// An expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// The number of concepts before paging.
    pub total: u64,
    /// The offset of this page.
    pub offset: usize,
    /// The entries of this page, ordered by system, version, then the
    /// provider's concept order.
    pub items: Vec<Item>,
    /// The system versions used.
    pub versions: Vec<UsedVersion>,
}

/// Resolves `include.valueSet` references to their complete expansions.
pub trait ValueSetResolver: std::fmt::Debug {
    /// The full expansion of the value set at `url`.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] when the value set is unknown or fails to expand.
    fn expand(&self, url: &str) -> Result<Expansion, ComposeError>;

    /// Whether the value set at `url` contains `code` of `system`, as the item
    /// it would expand to.
    ///
    /// The default expands and searches; a resolver over a store answers from
    /// the compose without enumerating.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] when the value set is unknown or fails.
    fn contains(&self, url: &str, system: &str, code: &str) -> Result<Option<Item>, ComposeError> {
        Ok(self
            .expand(url)?
            .items
            .into_iter()
            .find(|item| item.system == system && item.code == code))
    }
}

/// A failure to expand.
#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    /// A system or version is not served.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    /// A provider refused.
    #[error("code system `{system}` failed")]
    Provider {
        /// The system.
        system: String,
        /// The cause.
        #[source]
        source: ProviderError,
    },
    /// An enumerated concept is not in its system.
    #[error("code `{code}` is not in code system `{system}`")]
    UnknownCode {
        /// The system.
        system: String,
        /// The code.
        code: String,
    },
    /// `vsd-1`: an include names neither a system nor a value set.
    #[error("an include names neither a system nor a value set")]
    NoSystemOrValueSet,
    /// `vsd-2`: concepts or filters without a system.
    #[error("an include lists concepts or filters without a system")]
    CriteriaWithoutSystem,
    /// `vsd-3`: both concepts and filters.
    #[error("an include lists both concepts and filters")]
    ConceptsAndFilters,
    /// An include references a value set and no resolver is configured.
    #[error("value set `{0}` cannot be resolved: no resolver")]
    NoResolver(String),
    /// A referenced value set is not known.
    #[error("A definition for the value Set '{0}' could not be found")]
    UnknownValueSet(String),
    /// A version negotiation could not be honoured.
    #[error(transparent)]
    Negotiation(#[from] crate::valueset::negotiation::NegotiationError),
    /// A value set references itself through `include.valueSet`.
    #[error("value set `{0}` references itself")]
    Cycle(String),
}

/// A system URL and the version served.
type SelectionKey = (String, String);

/// One system version's part of an expansion: the concepts selected so far
/// and the displays the compose fixes for enumerated concepts.
struct Selection {
    provider: Arc<dyn CodeSystemProvider>,
    set: ConceptSet,
    overrides: BTreeMap<u32, String>,
    /// The compose's spelling of an enumerated code the system spells otherwise.
    spellings: BTreeMap<u32, String>,
}

/// Sets at most this large check activity concept by concept; larger ones
/// subtract the provider's inactive set, which a system that cannot
/// enumerate never has to produce.
const STATUS_SCAN_LIMIT: u64 = 1024;

/// Expands composes over a registry.
#[derive(Debug)]
pub struct Expander<'a> {
    registry: &'a Registry,
    resolver: Option<&'a dyn ValueSetResolver>,
}

impl<'a> Expander<'a> {
    /// An expander over `registry` without value set references.
    #[must_use]
    pub fn new(registry: &'a Registry) -> Self {
        Self {
            registry,
            resolver: None,
        }
    }

    /// An expander that resolves `include.valueSet` through `resolver`.
    #[must_use]
    pub fn with_resolver(registry: &'a Registry, resolver: &'a dyn ValueSetResolver) -> Self {
        Self {
            registry,
            resolver: Some(resolver),
        }
    }

    /// Expands `compose` under `options`.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] for an invalid compose, an unknown system,
    /// version, or code, or a provider failure.
    pub fn expand(&self, compose: &Compose, options: &Options) -> Result<Expansion, ComposeError> {
        let mut selections: BTreeMap<SelectionKey, Selection> = BTreeMap::new();
        let mut versions: Vec<UsedVersion> = Vec::new();
        for include in &compose.include {
            for (key, part) in self.evaluate(include, options, &mut versions)? {
                match selections.get_mut(&key) {
                    Some(existing) => {
                        existing.set |= &part.set;
                        for (ordinal, display) in part.overrides {
                            existing.overrides.entry(ordinal).or_insert(display);
                        }
                    }
                    None => {
                        selections.insert(key, part);
                    }
                }
            }
        }
        for exclude in &compose.exclude {
            for (key, part) in self.evaluate(exclude, options, &mut versions)? {
                if let Some(existing) = selections.get_mut(&key) {
                    existing.set -= &part.set;
                }
            }
        }
        // NOTE: compose.inactive = false is a floor activeOnly cannot lift, and
        // activeOnly removes inactive codes a compose admits
        // (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>, activeOnly).
        if options.active_only || compose.inactive == Some(false) {
            for ((url, _), selection) in &mut selections {
                drop_inactive(url, selection)?;
            }
        }
        if options.exclude_not_for_ui {
            for ((url, _), selection) in &mut selections {
                drop_not_for_ui(url, selection)?;
            }
        }
        let total = selections.values().map(|s| s.set.len()).sum();
        let mut items = Vec::new();
        let mut skip = options.offset;
        let mut remaining = options.count.unwrap_or(usize::MAX);
        for ((url, version), selection) in &selections {
            if remaining == 0 {
                break;
            }
            let len = usize::try_from(selection.set.len()).unwrap_or(usize::MAX);
            if skip >= len {
                skip = skip.saturating_sub(len);
                continue;
            }
            for index in selection.set.iter().skip(skip).take(remaining) {
                items.push(materialize(url, version, selection, index, options)?);
            }
            remaining = remaining.saturating_sub(len.saturating_sub(skip));
            skip = 0;
        }
        versions.sort_by(|a, b| (&a.url, &a.version).cmp(&(&b.url, &b.version)));
        versions.dedup_by(|a, b| a.url == b.url && a.version == b.version);
        Ok(Expansion {
            total,
            offset: options.offset,
            items,
            versions,
        })
    }

    /// The selections one include (or exclude) makes, by system version.
    fn evaluate(
        &self,
        include: &Include,
        options: &Options,
        versions: &mut Vec<UsedVersion>,
    ) -> Result<BTreeMap<SelectionKey, Selection>, ComposeError> {
        if include.system.is_none() {
            if include.value_sets.is_empty() {
                return Err(ComposeError::NoSystemOrValueSet);
            }
            if !include.concepts.is_empty() || !include.filters.is_empty() {
                return Err(ComposeError::CriteriaWithoutSystem);
            }
        }
        if !include.concepts.is_empty() && !include.filters.is_empty() {
            return Err(ComposeError::ConceptsAndFilters);
        }
        let mut parts: Option<BTreeMap<SelectionKey, Selection>> = None;
        if let Some(system) = &include.system {
            let resolved = self
                .registry
                .resolve(&system.url, system.version.as_deref())?;
            let provider = &resolved.provider;
            let identity = provider.identity();
            versions.push(UsedVersion {
                url: identity.url.clone(),
                version: identity.version.clone(),
                defaulted: resolved.defaulted,
            });
            let set = Self::select(provider, include, options)?;
            let mut overrides = BTreeMap::new();
            let mut spellings = BTreeMap::new();
            for concept in &include.concepts {
                let located =
                    provider
                        .locate(&concept.code)
                        .map_err(|source| ComposeError::Provider {
                            system: identity.url.clone(),
                            source,
                        })?;
                let Some(located) = located else {
                    continue;
                };
                if let Some(display) = &concept.display {
                    overrides.insert(located.concept.index(), display.clone());
                }
                if located.code != concept.code {
                    spellings.insert(located.concept.index(), concept.code.clone());
                }
            }
            let mut selected = BTreeMap::new();
            selected.insert(
                (identity.url.clone(), identity.version.clone()),
                Selection {
                    provider: Arc::clone(provider),
                    set,
                    overrides,
                    spellings,
                },
            );
            parts = Some(selected);
        }
        // NOTE: several value sets in one include intersect, "in all the referenced
        // value sets" (<https://hl7.org/fhir/R4B/valueset.html#compositions>,
        // <https://hl7.org/fhir/R5/valueset.html#union-intersection>).
        for url in &include.value_sets {
            let resolver = self
                .resolver
                .ok_or_else(|| ComposeError::NoResolver(url.clone()))?;
            let referenced = self.selections_of(resolver.expand(url)?)?;
            parts = Some(match parts {
                None => referenced,
                Some(current) => current
                    .into_iter()
                    .filter_map(|(key, mut selection)| {
                        let other = referenced.get(&key)?;
                        selection.set &= &other.set;
                        Some((key, selection))
                    })
                    .collect(),
            });
        }
        Ok(parts.unwrap_or_default())
    }

    /// A referenced value set's expansion as selections, its items located
    /// in their systems, its displays kept as overrides.
    fn selections_of(
        &self,
        expansion: Expansion,
    ) -> Result<BTreeMap<SelectionKey, Selection>, ComposeError> {
        let mut selections: BTreeMap<SelectionKey, Selection> = BTreeMap::new();
        for item in expansion.items {
            let key = (item.system.clone(), item.version.clone());
            if !selections.contains_key(&key) {
                let resolved = self.registry.resolve(&item.system, Some(&item.version))?;
                selections.insert(
                    key.clone(),
                    Selection {
                        provider: Arc::clone(&resolved.provider),
                        set: ConceptSet::new(),
                        overrides: BTreeMap::new(),
                        spellings: BTreeMap::new(),
                    },
                );
            }
            let Some(selection) = selections.get_mut(&key) else {
                continue;
            };
            // NOTE: an enumerated code the system does not define leaves the
            // expansion instead of failing it (the ecosystem's `expand-enum-bad`
            // cases; the FHIR specification is silent on the case).
            let Some(located) =
                selection
                    .provider
                    .locate(&item.code)
                    .map_err(|source| ComposeError::Provider {
                        system: item.system.clone(),
                        source,
                    })?
            else {
                continue;
            };
            selection.set.insert(located.concept.index());
            if let Some(display) = item.display {
                selection.overrides.insert(located.concept.index(), display);
            }
        }
        Ok(selections)
    }

    fn select(
        provider: &Arc<dyn CodeSystemProvider>,
        include: &Include,
        options: &Options,
    ) -> Result<ConceptSet, ComposeError> {
        let system = provider.identity().url.clone();
        let failed = |source: ProviderError| ComposeError::Provider {
            system: system.clone(),
            source,
        };
        let mut set = if include.concepts.is_empty() {
            provider.filter_all(&include.filters).map_err(&failed)?
        } else {
            let mut set = ConceptSet::new();
            for concept in &include.concepts {
                let Some(located) = provider.locate(&concept.code).map_err(&failed)? else {
                    continue;
                };
                if options.exclude_post_coordinated && provider.is_postcoordinated(located.concept)
                {
                    continue;
                }
                set.insert(located.concept.index());
            }
            set
        };
        if let Some(text) = options
            .text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            set &= provider
                .search(
                    text,
                    language::for_provider(provider.as_ref(), options.language.as_deref())
                        .as_deref(),
                )
                .map_err(failed)?;
        }
        Ok(set)
    }
}

/// Removes the inactive concepts from `selection`: concept by concept for a
/// small set, by the provider's inactive set for a large one.
fn drop_inactive(url: &str, selection: &mut Selection) -> Result<(), ComposeError> {
    let failed = |source: ProviderError| ComposeError::Provider {
        system: url.to_owned(),
        source,
    };
    if selection.set.len() > STATUS_SCAN_LIMIT {
        match selection.provider.inactive() {
            Ok(inactive) => {
                selection.set -= &inactive;
                return Ok(());
            }
            Err(ProviderError::NotEnumerable) => {}
            Err(source) => return Err(failed(source)),
        }
    }
    let mut inactive = ConceptSet::new();
    for index in &selection.set {
        if !selection
            .provider
            .status(Concept::new(index))
            .map_err(&failed)?
            .active
        {
            inactive.insert(index);
        }
    }
    selection.set -= inactive;
    Ok(())
}

/// Drops the abstract (`notSelectable`) groupers from `selection`: by a status
/// scan for a small set, by the provider's set for a large one.
fn drop_not_for_ui(url: &str, selection: &mut Selection) -> Result<(), ComposeError> {
    let failed = |source: ProviderError| ComposeError::Provider {
        system: url.to_owned(),
        source,
    };
    if selection.set.len() > STATUS_SCAN_LIMIT {
        match selection.provider.not_for_ui() {
            Ok(abstract_concepts) => {
                selection.set -= &abstract_concepts;
                return Ok(());
            }
            Err(ProviderError::NotEnumerable) => {}
            Err(source) => return Err(failed(source)),
        }
    }
    let mut abstract_concepts = ConceptSet::new();
    for index in &selection.set {
        if selection
            .provider
            .status(Concept::new(index))
            .map_err(&failed)?
            .abstract_concept
        {
            abstract_concepts.insert(index);
        }
    }
    selection.set -= abstract_concepts;
    Ok(())
}

/// The item for the concept at `index` of `selection`.
fn materialize(
    url: &str,
    version: &str,
    selection: &Selection,
    index: u32,
    options: &Options,
) -> Result<Item, ComposeError> {
    let failed = |source: ProviderError| ComposeError::Provider {
        system: url.to_owned(),
        source,
    };
    let concept = Concept::new(index);
    let provider = &selection.provider;
    let code = match selection.spellings.get(&index) {
        Some(spelling) => spelling.clone(),
        None => provider
            .code(concept)
            .map_err(&failed)?
            .unwrap_or_else(|| index.to_string()),
    };
    let display = match selection.overrides.get(&index) {
        Some(display) => Some(display.clone()),
        None => provider
            .display(
                concept,
                language::for_provider(provider.as_ref(), options.language.as_deref()).as_deref(),
            )
            .map_err(&failed)?,
    };
    let status = provider.status(concept).map_err(&failed)?;
    Ok(Item {
        system: url.to_owned(),
        version: version.to_owned(),
        code,
        display,
        inactive: !status.active,
        abstract_concept: status.abstract_concept,
    })
}

impl Expander<'_> {
    /// Whether `compose` contains `code` of `system` (at `version` when
    /// named), evaluated include by include against the code itself, so a
    /// system that cannot enumerate its codes still validates.
    ///
    /// The item carries the display in `language`. `compose.inactive = false`
    /// excludes an inactive code as it does in an expansion.
    ///
    /// # Errors
    ///
    /// Returns [`ComposeError`] for an invalid compose, an unknown system or
    /// version, or a provider failure.
    pub fn contains(
        &self,
        compose: &Compose,
        system: &str,
        version: Option<&str>,
        code: &str,
        language: Option<&str>,
    ) -> Result<Option<Item>, ComposeError> {
        let mut found = None;
        for include in &compose.include {
            if let Some(item) = self.include_contains(include, system, version, code, language)? {
                found = Some(item);
                break;
            }
        }
        let Some(item) = found else {
            return Ok(None);
        };
        for exclude in &compose.exclude {
            if self
                .include_contains(exclude, system, version, code, language)?
                .is_some()
            {
                return Ok(None);
            }
        }
        if compose.inactive == Some(false) && item.inactive {
            return Ok(None);
        }
        Ok(Some(item))
    }

    /// The item `include` admits for `code` of `system`, if any.
    fn include_contains(
        &self,
        include: &Include,
        system: &str,
        version: Option<&str>,
        code: &str,
        language: Option<&str>,
    ) -> Result<Option<Item>, ComposeError> {
        if include.system.is_none() {
            if include.value_sets.is_empty() {
                return Err(ComposeError::NoSystemOrValueSet);
            }
            if !include.concepts.is_empty() || !include.filters.is_empty() {
                return Err(ComposeError::CriteriaWithoutSystem);
            }
        }
        if !include.concepts.is_empty() && !include.filters.is_empty() {
            return Err(ComposeError::ConceptsAndFilters);
        }
        let mut item = None;
        if let Some(named) = &include.system {
            if named.url != system {
                return Ok(None);
            }
            // NOTE: a subject version inside the include's pattern (`1.0.0` in
            // `1.x.x`) is the version checked; the include's own otherwise.
            let wanted = match (named.version.as_deref(), version) {
                (Some(pattern), Some(v)) if crate::versioned::version_matches(pattern, v) => {
                    Some(v)
                }
                (Some(pattern), _) => Some(pattern),
                (None, v) => v,
            };
            let resolved = self.registry.resolve(&named.url, wanted)?;
            let provider = &resolved.provider;
            let identity = provider.identity();
            if version.is_some_and(|v| !crate::versioned::version_matches(v, &identity.version)) {
                return Ok(None);
            }
            let failed = |source| ComposeError::Provider {
                system: identity.url.clone(),
                source,
            };
            let Some(located) = provider.locate(code).map_err(failed)? else {
                return Ok(None);
            };
            let admitted = if include.concepts.is_empty() {
                let mut all = true;
                for filter in &include.filters {
                    if !provider
                        .filter_matches(located.concept, filter)
                        .map_err(failed)?
                    {
                        all = false;
                        break;
                    }
                }
                all
            } else {
                include.concepts.iter().any(|c| c.code == located.code)
            };
            if !admitted {
                return Ok(None);
            }
            let overridden = include
                .concepts
                .iter()
                .find(|c| c.code == located.code)
                .and_then(|c| c.display.clone());
            let display = match overridden {
                Some(display) => Some(display),
                None => provider
                    .display(
                        located.concept,
                        language::for_provider(provider.as_ref(), language).as_deref(),
                    )
                    .map_err(failed)?,
            };
            let status = provider.status(located.concept).map_err(failed)?;
            item = Some(Item {
                system: identity.url.clone(),
                version: identity.version.clone(),
                code: located.code,
                display,
                inactive: !status.active,
                abstract_concept: status.abstract_concept,
            });
        }
        for url in &include.value_sets {
            let resolver = self
                .resolver
                .ok_or_else(|| ComposeError::NoResolver(url.clone()))?;
            match resolver.contains(url, system, code)? {
                Some(referenced) => {
                    if item.is_none() {
                        item = Some(referenced);
                    }
                }
                None => return Ok(None),
            }
        }
        Ok(item)
    }
}
