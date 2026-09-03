//! Request-scoped resources: `tx-resource` parameters and `$cache-control`.
//!
//! The terminology ecosystem's runner never uploads its setup resources: it
//! sends them as `tx-resource` parameters on every operation, or front-loads
//! them once with `$cache-control?mode=start` and names the returned cache
//! with the `X-Cache-Id` header
//! (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
//! A request's resources form a [`Scope`]: the loaded registry and value set
//! store with those resources layered on top, for that request only. Each
//! served version converts its own generated resources to the models held
//! here, so a cache started on one version serves every version.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ferroterm_terminology::conceptmap::model::ConceptMapModel;
use ferroterm_terminology::conceptmap::store::ConceptMapStore;
use ferroterm_terminology::fhir_codesystem::model::CodeSystemModel;
use ferroterm_terminology::fhir_codesystem::provider::FhirCodeSystem;
use ferroterm_terminology::operations::Sources;
use ferroterm_terminology::provider::ContentMode;
use ferroterm_terminology::registry::Registry;
use ferroterm_terminology::supplement::Supplemented;
use ferroterm_terminology::valueset::model::ValueSetModel;
use ferroterm_terminology::valueset::store::ValueSetStore;
use http::{HeaderMap, StatusCode};

use crate::outcome::Failure;
use crate::state::{AppState, supplement_of};

/// The parameter that carries a request-scoped resource.
pub const TX_RESOURCE: &str = "tx-resource";
/// The ecosystem runner's profile parameter naming the expansion identifier
/// to use; not declared by any version, peeled off with the resources.
pub const UUID: &str = "uuid";
/// The header that names a cache started with `$cache-control`.
pub const CACHE_ID_HEADER: &str = "X-Cache-Id";
/// How long an unused cache lives. No spec fixes this: our own design.
pub const CACHE_IDLE: Duration = Duration::from_mins(30);

/// A request-scoped resource as the model the engine serves.
#[derive(Debug, Clone)]
pub enum Loaded {
    /// A `CodeSystem`, complete or a supplement.
    CodeSystem(CodeSystemModel),
    /// A `ValueSet`.
    ValueSet(ValueSetModel),
    /// A `ConceptMap`.
    ConceptMap(ConceptMapModel),
}

/// The registry, value set store, and concept map store one request sees.
#[derive(Debug)]
pub struct Scope<'a> {
    registry: Cow<'a, Registry>,
    value_sets: Cow<'a, ValueSetStore>,
    concept_maps: Cow<'a, ConceptMapStore>,
}

impl<'a> Scope<'a> {
    /// The loaded registry and stores, unchanged.
    #[must_use]
    pub fn base(state: &'a AppState) -> Self {
        Self {
            registry: Cow::Borrowed(state.registry()),
            value_sets: Cow::Borrowed(state.value_sets()),
            concept_maps: Cow::Borrowed(state.concept_maps()),
        }
    }

    /// The loaded registry and stores with `resources` layered on top.
    ///
    /// A resource with the url and version of a loaded one replaces it for
    /// this request; a `CodeSystem` supplement is layered over the system it
    /// names.
    ///
    /// # Errors
    ///
    /// A `CodeSystem` the engine cannot serve is a 400; a supplement whose
    /// target is not loaded is a 404.
    pub fn layered(state: &'a AppState, resources: &[Loaded]) -> Result<Self, Failure> {
        if resources.is_empty() {
            return Ok(Self::base(state));
        }
        let mut registry = state.registry().clone();
        let mut value_sets = state.value_sets().clone();
        let mut concept_maps = state.concept_maps().clone();
        let mut supplements = Vec::new();
        for resource in resources {
            match resource {
                Loaded::CodeSystem(model) => {
                    if model.content == ContentMode::Supplement {
                        supplements.push(model);
                        continue;
                    }
                    let provider = FhirCodeSystem::new(model.clone()).map_err(|e| {
                        Failure::new(
                            StatusCode::BAD_REQUEST,
                            "invalid",
                            format!("a `{TX_RESOURCE}` cannot be served: {e}"),
                        )
                    })?;
                    registry.register_or_replace(Arc::new(provider));
                }
                Loaded::ValueSet(model) => value_sets.replace(model.clone()),
                Loaded::ConceptMap(model) => concept_maps.replace(model.clone()),
            }
        }
        for model in supplements {
            let Some(target) = &model.supplements else {
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    format!(
                        "the supplement `{}` names no code system to supplement",
                        model.url
                    ),
                ));
            };
            let (url, version) = match target.split_once('|') {
                Some((url, version)) => (url, Some(version)),
                None => (target.as_str(), None),
            };
            let resolved = registry.resolve(url, version).map_err(|e| {
                Failure::new(
                    StatusCode::NOT_FOUND,
                    "not-found",
                    format!("cannot supplement: {e}"),
                )
            })?;
            registry.register_or_replace(Arc::new(Supplemented::new(
                resolved.provider,
                vec![supplement_of(model)],
            )));
        }
        Ok(Self {
            registry: Cow::Owned(registry),
            value_sets: Cow::Owned(value_sets),
            concept_maps: Cow::Owned(concept_maps),
        })
    }

    /// The registry this request resolves systems in.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The value sets this request resolves.
    #[must_use]
    pub fn value_sets(&self) -> &ValueSetStore {
        &self.value_sets
    }

    /// The engine's view of this scope.
    #[must_use]
    pub fn sources(&self) -> Sources<'_> {
        Sources {
            registry: &self.registry,
            value_sets: &self.value_sets,
            concept_maps: &self.concept_maps,
        }
    }
}

/// The scope of one request: the named cache's resources, then the request's
/// own, over the loaded state.
///
/// # Errors
///
/// A malformed or unknown `X-Cache-Id`, or a resource that cannot be layered.
pub fn scope_of<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
    mut resources: Vec<Loaded>,
) -> Result<Scope<'a>, Failure> {
    if let Some(id) = cache_id(headers)? {
        let cached = state.caches().get(&id)?;
        let mut all = Vec::with_capacity(cached.len() + resources.len());
        all.extend(cached.iter().cloned());
        all.append(&mut resources);
        resources = all;
    }
    Scope::layered(state, &resources)
}

/// The cache the request names, if any.
///
/// # Errors
///
/// More than one `X-Cache-Id`, or one that is not text.
pub fn cache_id(headers: &HeaderMap) -> Result<Option<String>, Failure> {
    let mut values = headers.get_all(CACHE_ID_HEADER).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid",
            format!("the request carries more than one `{CACHE_ID_HEADER}`"),
        ));
    }
    first.to_str().map(|id| Some(id.to_owned())).map_err(|_| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "invalid",
            format!("`{CACHE_ID_HEADER}` is not text"),
        )
    })
}

struct Entry {
    resources: Arc<Vec<Loaded>>,
    last_used: Instant,
}

/// The caches `$cache-control` started, by id; an unused one expires after
/// [`CACHE_IDLE`].
#[derive(Default)]
pub struct Caches {
    entries: Mutex<BTreeMap<String, Entry>>,
}

impl std::fmt::Debug for Caches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        f.debug_struct("Caches").field("count", &count).finish()
    }
}

impl Caches {
    /// Starts a cache holding `resources` and returns its id.
    pub fn start(&self, resources: Vec<Loaded>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        Self::prune(&mut entries);
        entries.insert(
            id.clone(),
            Entry {
                resources: Arc::new(resources),
                last_used: Instant::now(),
            },
        );
        id
    }

    /// The resources of cache `id`, touching it.
    ///
    /// # Errors
    ///
    /// The cache is unknown or expired.
    pub fn get(&self, id: &str) -> Result<Arc<Vec<Loaded>>, Failure> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        Self::prune(&mut entries);
        let entry = entries.get_mut(id).ok_or_else(|| unknown(id))?;
        entry.last_used = Instant::now();
        Ok(Arc::clone(&entry.resources))
    }

    /// Ends cache `id`.
    ///
    /// # Errors
    ///
    /// The cache is unknown or expired.
    pub fn end(&self, id: &str) -> Result<(), Failure> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.remove(id).map(|_| ()).ok_or_else(|| unknown(id))
    }

    fn prune(entries: &mut BTreeMap<String, Entry>) {
        let now = Instant::now();
        entries.retain(|_, entry| now.duration_since(entry.last_used) < CACHE_IDLE);
    }
}

fn unknown(id: &str) -> Failure {
    Failure::new(
        StatusCode::NOT_FOUND,
        "not-found",
        format!("cache `{id}` is not known: it was never started, or it expired or was ended"),
    )
}
