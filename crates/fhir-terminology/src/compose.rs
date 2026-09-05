//! The compose layer: `ValueSet.compose` evaluated once, above every provider.
//!
//! Includes union, criteria within an include intersect, excludes subtract,
//! and a concept appears once per system, version, and code
//! (<https://hl7.org/fhir/R4B/valueset.html#compositions>,
//! <https://hl7.org/fhir/R5/valueset.html#union-intersection>). A flat
//! expansion answers in the order the compose selected: the includes in order,
//! each include's named concepts as it named them and each filter's concepts in
//! the provider's concept order, first occurrence winning. No version fixes an
//! expansion order, and `ValueSet.compose.include.concept` says an expansion
//! typically follows the compose, so this is our own design. Includes and
//! excludes are bitmap algebra; only the page asked for is read from the store.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::filter::{Filter, FilterOperator};
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a boolean parameter the operation declares, named as it names it"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// `activeOnly`: drop inactive concepts.
    pub active_only: bool,
    /// `excludeNested`: keep the expansion flat even where the compose admits
    /// the system's hierarchy.
    pub exclude_nested: bool,
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
    /// The item's depth in a nested expansion; `0` for a root and for every
    /// item of a flat one.
    pub depth: usize,
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
    /// The entries of this page, in the order the compose selected them.
    pub items: Vec<Item>,
    /// The system versions used.
    pub versions: Vec<UsedVersion>,
    /// Whether `items` carry the system's hierarchy in their depths.
    pub nested: bool,
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
    /// What each include contributed, in the order the includes ran.
    segments: Vec<Segment>,
    /// Everything the segments already carry, so an overlapping include adds
    /// nothing and the first occurrence keeps its place.
    seen: ConceptSet,
    overrides: BTreeMap<u32, String>,
    /// The compose's spelling of an enumerated code the system spells otherwise.
    // NOTE: no FHIR specification governs which spelling `contains.code` carries when a
    // system admits several (the ecosystem's icd-11 `expand-adhoc-enum-uri` keeps the
    // compose's); the compose's spelling is kept wherever the compose is: our own design.
    spellings: BTreeMap<u32, String>,
}

/// What one include contributed to a selection.
enum Segment {
    /// Concepts the include named, in the order it named them.
    Named(Vec<u32>),
    /// Concepts a filter selected, in the provider's concept order.
    Selected(ConceptSet),
}

/// One segment's concepts, without deciding the type at the call site.
enum Concepts<'a> {
    Named(std::iter::Copied<std::slice::Iter<'a, u32>>),
    Selected(roaring::bitmap::Iter<'a>),
}

impl Iterator for Concepts<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Named(iter) => iter.next(),
            Self::Selected(iter) => iter.next(),
        }
    }
}

impl Selection {
    /// The concepts of this selection in the order the includes selected them.
    ///
    /// A compose of one filter is one segment over the bitmap, so a page still
    /// comes off the selection without reading a concept.
    fn ordered(&self) -> impl Iterator<Item = u32> + '_ {
        self.segments
            .iter()
            .flat_map(|segment| match segment {
                Segment::Named(named) => Concepts::Named(named.iter().copied()),
                Segment::Selected(set) => Concepts::Selected(set.iter()),
            })
            .filter(|index| self.set.contains(*index))
    }

    /// Adds what another include selected of the same system version.
    fn absorb(&mut self, part: Self) {
        for segment in part.segments {
            let fresh = match segment {
                Segment::Named(named) => Segment::Named(
                    named
                        .into_iter()
                        .filter(|index| !self.seen.contains(*index))
                        .collect(),
                ),
                Segment::Selected(set) => Segment::Selected(&set - &self.seen),
            };
            match &fresh {
                Segment::Named(named) => self.seen.extend(named.iter().copied()),
                Segment::Selected(set) => self.seen |= set,
            }
            self.segments.push(fresh);
        }
        self.set |= &part.set;
        for (ordinal, display) in part.overrides {
            self.overrides.entry(ordinal).or_insert(display);
        }
        for (ordinal, spelling) in part.spellings {
            self.spellings.entry(ordinal).or_insert(spelling);
        }
    }
}

/// What an include's own system says about a code.
enum Contained {
    /// The include names no system, so only its referenced value sets decide.
    NoSystem,
    /// The code is not in this include.
    Refused,
    /// The include contains it, as this item.
    Item(Item),
}

/// Whether `include` admits `concept`: every filter matches, or the code is one
/// of the enumerated ones.
///
/// # Errors
///
/// Returns the provider's failure when a filter cannot be evaluated.
fn admits(
    provider: &Arc<dyn CodeSystemProvider>,
    include: &Include,
    concept: Concept,
    code: &str,
) -> Result<bool, ProviderError> {
    if !include.concepts.is_empty() {
        return Ok(include.concepts.iter().any(|c| c.code == code));
    }
    for filter in &include.filters {
        if !provider.filter_matches(concept, filter)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The three checks `ValueSet.compose.include` must pass (`vsd-1`, `vsd-2`,
/// `vsd-3`, <https://hl7.org/fhir/R4B/valueset.html>).
///
/// # Errors
///
/// Returns the constraint the include breaks.
fn well_formed(include: &Include) -> Result<(), ComposeError> {
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
    Ok(())
}

/// What the includes and excludes gathered: the selection per system version,
/// the order the compose first named those versions in, and the versions the
/// evaluation used.
#[derive(Default)]
struct Gathered {
    selections: BTreeMap<SelectionKey, Selection>,
    order: Vec<SelectionKey>,
    versions: Vec<UsedVersion>,
}

/// One page of an expansion.
struct Page {
    items: Vec<Item>,
    /// Whether the items carry the system's hierarchy in their depths.
    nested: bool,
}

/// Removes what the compose and the options refuse: the inactive concepts and
/// the ones no user interface should offer.
///
/// `compose.inactive = false` is a floor `activeOnly` cannot lift, and
/// `activeOnly` removes inactive codes a compose admits
/// (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>, activeOnly).
fn drop_refused(
    selections: &mut BTreeMap<SelectionKey, Selection>,
    compose: &Compose,
    options: &Options,
) -> Result<(), ComposeError> {
    if options.active_only || compose.inactive == Some(false) {
        for ((url, _), selection) in &mut *selections {
            drop_inactive(url, selection)?;
        }
    }
    if options.exclude_not_for_ui {
        for ((url, _), selection) in selections {
            drop_not_for_ui(url, selection)?;
        }
    }
    Ok(())
}

/// The page `options` asks for, over the order the compose stated.
///
/// A nested expansion pages over its pre-order flattening, so `count` and
/// `offset` mean the same thing they do in a flat one.
fn paged(compose: &Compose, options: &Options, gathered: &Gathered) -> Result<Page, ComposeError> {
    let mut page = Page {
        items: Vec::new(),
        nested: false,
    };
    let mut skip = options.offset;
    let mut remaining = options.count.unwrap_or(usize::MAX);
    for key in &gathered.order {
        if remaining == 0 {
            break;
        }
        let (url, version) = key;
        let Some(selection) = gathered.selections.get(key) else {
            continue;
        };
        let len = usize::try_from(selection.set.len()).unwrap_or(usize::MAX);
        if skip >= len {
            skip = skip.saturating_sub(len);
            continue;
        }
        let cut: Vec<(u32, usize)> = match nests(compose, options)
            .then(|| preorder(selection))
            .flatten()
        {
            Some(tree) => {
                page.nested = true;
                tree.into_iter().skip(skip).take(remaining).collect()
            }
            None => selection
                .ordered()
                .map(|index| (index, 0))
                .skip(skip)
                .take(remaining)
                .collect(),
        };
        for (index, depth) in cut {
            page.items
                .push(materialize(url, version, selection, index, depth, options)?);
        }
        remaining = remaining.saturating_sub(len.saturating_sub(skip));
        skip = 0;
    }
    Ok(page)
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
        let mut gathered = self.gather(compose, options)?;
        drop_refused(&mut gathered.selections, compose, options)?;
        let total = gathered.selections.values().map(|s| s.set.len()).sum();
        let page = paged(compose, options, &gathered)?;
        gathered
            .versions
            .sort_by(|a, b| (&a.url, &a.version).cmp(&(&b.url, &b.version)));
        gathered
            .versions
            .dedup_by(|a, b| a.url == b.url && a.version == b.version);
        Ok(Expansion {
            total,
            offset: options.offset,
            items: page.items,
            versions: gathered.versions,
            nested: page.nested,
        })
    }

    /// The includes united and the excludes subtracted, with the order the
    /// compose named the system versions in and the versions it used.
    fn gather(&self, compose: &Compose, options: &Options) -> Result<Gathered, ComposeError> {
        let mut gathered = Gathered::default();
        for include in &compose.include {
            for (key, part) in self.evaluate(include, options, &mut gathered.versions)? {
                if let Some(existing) = gathered.selections.get_mut(&key) {
                    existing.absorb(part);
                } else {
                    gathered.order.push(key.clone());
                    gathered.selections.insert(key, part);
                }
            }
        }
        for exclude in &compose.exclude {
            for (key, part) in self.evaluate(exclude, options, &mut gathered.versions)? {
                if let Some(existing) = gathered.selections.get_mut(&key) {
                    existing.set -= &part.set;
                }
            }
        }
        Ok(gathered)
    }

    /// The selections one include (or exclude) makes, by system version.
    fn evaluate(
        &self,
        include: &Include,
        options: &Options,
        versions: &mut Vec<UsedVersion>,
    ) -> Result<BTreeMap<SelectionKey, Selection>, ComposeError> {
        well_formed(include)?;
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
            let Selected {
                set,
                stated,
                overrides,
                spellings,
            } = Self::select(provider, include, options)?;
            // An include either names its concepts or states a filter, so one
            // include is one segment.
            let segment = match stated {
                Some(named) => Segment::Named(named),
                None => Segment::Selected(set.clone()),
            };
            let seen = set.clone();
            let mut selected = BTreeMap::new();
            selected.insert(
                (identity.url.clone(), identity.version.clone()),
                Selection {
                    provider: Arc::clone(provider),
                    set,
                    segments: vec![segment],
                    seen,
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
                        segments: vec![Segment::Named(Vec::new())],
                        seen: ConceptSet::new(),
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
            // The referenced expansion states an order; it carries into this one.
            if selection.set.insert(located.concept.index()) {
                selection.seen.insert(located.concept.index());
                if let Some(Segment::Named(named)) = selection.segments.first_mut() {
                    named.push(located.concept.index());
                }
            }
            if located.code != item.code {
                selection
                    .spellings
                    .insert(located.concept.index(), item.code.clone());
            }
            if let Some(display) = item.display {
                selection.overrides.insert(located.concept.index(), display);
            }
        }
        Ok(selections)
    }

    /// The concepts `include` selects from `provider`, with the compose's display
    /// overrides and spellings of its enumerated codes, each located once.
    fn select(
        provider: &Arc<dyn CodeSystemProvider>,
        include: &Include,
        options: &Options,
    ) -> Result<Selected, ComposeError> {
        let system = provider.identity().url.clone();
        let failed = |source: ProviderError| ComposeError::Provider {
            system: system.clone(),
            source,
        };
        let mut overrides = BTreeMap::new();
        let mut spellings = BTreeMap::new();
        let mut stated: Vec<u32> = Vec::new();
        let names = !include.concepts.is_empty();
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
                if set.insert(located.concept.index()) {
                    stated.push(located.concept.index());
                }
                if let Some(display) = &concept.display {
                    overrides.insert(located.concept.index(), display.clone());
                }
                if located.code != concept.code {
                    spellings.insert(located.concept.index(), concept.code.clone());
                }
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
        stated.retain(|index| set.contains(*index));
        Ok(Selected {
            set,
            stated: names.then_some(stated),
            overrides,
            spellings,
        })
    }
}

/// What one include selects: the concept set with the compose's display
/// overrides and spellings, by ordinal.
struct Selected {
    set: ConceptSet,
    /// The enumerated concepts in the order the include named them, or `None`
    /// when the include stated a filter and named no concept.
    stated: Option<Vec<u32>>,
    overrides: BTreeMap<u32, String>,
    spellings: BTreeMap<u32, String>,
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

/// Whether `compose` expands as the system's own hierarchy: one include of one
/// system, selected by the whole system or by a hierarchical filter, so the
/// selection is a set of subtrees rather than a list somebody enumerated.
///
/// A `ValueSet.expansion.contains` may nest its children
/// (<https://hl7.org/fhir/R4B/valueset-definitions.html#ValueSet.expansion.contains>);
/// the ecosystem's `parameters-expand-*-hierarchy` cases pin which composes do.
fn nests(compose: &Compose, options: &Options) -> bool {
    if options.exclude_nested || !compose.exclude.is_empty() {
        return false;
    }
    let [include] = compose.include.as_slice() else {
        return false;
    };
    if include.system.is_none() || !include.concepts.is_empty() || !include.value_sets.is_empty() {
        return false;
    }
    let hierarchical = |filter: &Filter| {
        matches!(
            filter.op,
            FilterOperator::IsA | FilterOperator::DescendentOf
        )
    };
    if !include.filters.iter().all(hierarchical) {
        return false;
    }
    // NOTE: a text search over a whole system returns scattered matches with no root
    // to hang them from and the ecosystem's `search-all-yes` case wants them flat; the
    // same search inside an `is-a` include keeps the subtree (its `search-filter-yes`).
    include.filters.iter().any(hierarchical)
        || options
            .text
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
}

/// The members of `selection` in pre-order with their depths: each root (a
/// member no other member subsumes) followed by its children, in the
/// provider's concept order.
///
/// `None` when the provider has no hierarchy to walk.
fn preorder(selection: &Selection) -> Option<Vec<(u32, usize)>> {
    let hierarchy = selection.provider.hierarchy()?;
    let mut roots = Vec::new();
    for index in &selection.set {
        let parents = hierarchy.parents(Concept::new(index));
        if !parents.iter().any(|parent| selection.set.contains(parent)) {
            roots.push(index);
        }
    }
    let mut out = Vec::with_capacity(selection.set.len().try_into().unwrap_or(usize::MAX));
    let mut seen = ConceptSet::new();
    let mut stack: Vec<(u32, usize)> = roots.into_iter().rev().map(|root| (root, 0)).collect();
    while let Some((index, depth)) = stack.pop() {
        if !seen.insert(index) {
            continue;
        }
        out.push((index, depth));
        let children: Vec<u32> = hierarchy
            .children(Concept::new(index))
            .iter()
            .filter(|child| selection.set.contains(*child) && !seen.contains(*child))
            .collect();
        stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }
    // A cycle or a member no root reaches keeps its place at the end, flat.
    for index in &selection.set {
        if !seen.contains(index) {
            out.push((index, 0));
        }
    }
    Some(out)
}

/// The item for the concept at `index` of `selection`.
fn materialize(
    url: &str,
    version: &str,
    selection: &Selection,
    index: u32,
    depth: usize,
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
        depth,
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
        well_formed(include)?;
        let mut item = match self.system_contains(include, system, version, code, language)? {
            Contained::Refused => return Ok(None),
            Contained::NoSystem => None,
            Contained::Item(item) => Some(item),
        };
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

    /// What the include's own system says about `code`.
    ///
    /// The include's version pattern and the subject's version are matched the
    /// way `$validate-code` matches them: a subject version inside the
    /// include's pattern (`1.0.0` in `1.x.x`) is the version checked, the
    /// include's own otherwise.
    fn system_contains(
        &self,
        include: &Include,
        system: &str,
        version: Option<&str>,
        code: &str,
        language: Option<&str>,
    ) -> Result<Contained, ComposeError> {
        let Some(named) = &include.system else {
            return Ok(Contained::NoSystem);
        };
        if named.url != system {
            return Ok(Contained::Refused);
        }
        let wanted = match (named.version.as_deref(), version) {
            (Some(pattern), Some(v)) if crate::versioned::version_matches(pattern, v) => Some(v),
            (Some(pattern), _) => Some(pattern),
            (None, v) => v,
        };
        let resolved = self.registry.resolve(&named.url, wanted)?;
        let provider = &resolved.provider;
        let identity = provider.identity();
        if version.is_some_and(|v| !crate::versioned::version_matches(v, &identity.version)) {
            return Ok(Contained::Refused);
        }
        let failed = |source| ComposeError::Provider {
            system: identity.url.clone(),
            source,
        };
        let Some(located) = provider.locate(code).map_err(failed)? else {
            return Ok(Contained::Refused);
        };
        if !admits(provider, include, located.concept, &located.code).map_err(failed)? {
            return Ok(Contained::Refused);
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
        Ok(Contained::Item(Item {
            system: identity.url.clone(),
            version: identity.version.clone(),
            code: located.code,
            display,
            inactive: !status.active,
            abstract_concept: status.abstract_concept,
            depth: 0,
        }))
    }
}
