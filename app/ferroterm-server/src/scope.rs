//! Request-scoped resources: `tx-resource` parameters and `$cache-control`.
//!
//! The terminology ecosystem's runner never uploads its setup resources: it
//! sends them as `tx-resource` parameters on every operation, or front-loads
//! them once with `$cache-control?mode=start` and names the returned cache
//! with the `X-Cache-Id` header
//! (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
//! A request's resources form a [`Scope`]: the loaded registry and value set
//! store with those resources layered on top, for that request only.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ferroterm_fhir::r4b::parameters::Parameters;
use ferroterm_fhir::r4b::resource::Resource;
use ferroterm_terminology::fhir_codesystem::convert;
use ferroterm_terminology::fhir_codesystem::provider::FhirCodeSystem;
use ferroterm_terminology::operations::Sources;
use ferroterm_terminology::provider::ContentMode;
use ferroterm_terminology::registry::Registry;
use ferroterm_terminology::supplement::Supplemented;
use ferroterm_terminology::valueset;
use ferroterm_terminology::valueset::store::ValueSetStore;
use http::{HeaderMap, StatusCode};

use crate::outcome::Failure;
use crate::state::{AppState, supplement_of};

/// The parameter that carries a request-scoped resource.
pub const TX_RESOURCE: &str = "tx-resource";
/// The header that names a cache started with `$cache-control`.
pub const CACHE_ID_HEADER: &str = "X-Cache-Id";
/// How long an unused cache lives. No spec fixes this: our own design.
pub const CACHE_IDLE: Duration = Duration::from_mins(30);

/// The code systems and value sets one request works over.
#[derive(Debug)]
pub struct Scope<'a> {
    registry: Cow<'a, Registry>,
    value_sets: Cow<'a, ValueSetStore>,
}

impl<'a> Scope<'a> {
    /// The loaded resources alone.
    #[must_use]
    pub fn base(state: &'a AppState) -> Self {
        Self {
            registry: Cow::Borrowed(state.registry()),
            value_sets: Cow::Borrowed(state.value_sets()),
        }
    }

    /// The loaded resources with `resources` layered on top; a resource with
    /// the same identity as a loaded one shadows it for this request.
    ///
    /// # Errors
    ///
    /// Returns a `400` failure for a resource that is not a `CodeSystem` or a
    /// `ValueSet`, or one the model cannot represent, and a `404` for a
    /// supplement whose system is not served.
    pub fn layered(state: &'a AppState, resources: &[Resource]) -> Result<Self, Failure> {
        if resources.is_empty() {
            return Ok(Self::base(state));
        }
        let mut registry = state.registry().clone();
        let mut value_sets = state.value_sets().clone();
        let mut supplements = Vec::new();
        for resource in resources {
            match resource {
                Resource::CodeSystem(code_system) => {
                    let model = convert::r4b::convert(code_system).map_err(invalid)?;
                    if model.content == ContentMode::Supplement {
                        supplements.push(model);
                        continue;
                    }
                    let provider = FhirCodeSystem::new(model).map_err(invalid)?;
                    registry.register_or_replace(Arc::new(provider));
                }
                Resource::ValueSet(value_set) => {
                    let model = valueset::convert::r4b::convert(value_set).map_err(invalid)?;
                    value_sets.replace(model);
                }
                other => {
                    return Err(Failure::new(
                        StatusCode::BAD_REQUEST,
                        "not-supported",
                        format!(
                            "`{TX_RESOURCE}` carries a {}; only CodeSystem and ValueSet resources are accepted",
                            resource_type(other)
                        ),
                    ));
                }
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
                vec![supplement_of(&model)],
            )));
        }
        Ok(Self {
            registry: Cow::Owned(registry),
            value_sets: Cow::Owned(value_sets),
        })
    }

    /// The registry of this scope.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The value sets of this scope.
    #[must_use]
    pub fn value_sets(&self) -> &ValueSetStore {
        &self.value_sets
    }

    /// What the operations take.
    #[must_use]
    pub fn sources(&self) -> Sources<'_> {
        Sources {
            registry: &self.registry,
            value_sets: &self.value_sets,
        }
    }
}

fn invalid(error: impl std::fmt::Display) -> Failure {
    Failure::new(
        StatusCode::BAD_REQUEST,
        "invalid",
        format!("a `{TX_RESOURCE}` cannot be served: {error}"),
    )
}

/// The resource type of `resource`, for a refusal.
fn resource_type(resource: &Resource) -> &str {
    match resource {
        Resource::Bundle(_) => "Bundle",
        Resource::CapabilityStatement(_) => "CapabilityStatement",
        Resource::CodeSystem(_) => "CodeSystem",
        Resource::ConceptMap(_) => "ConceptMap",
        Resource::OperationOutcome(_) => "OperationOutcome",
        Resource::Parameters(_) => "Parameters",
        Resource::TerminologyCapabilities(_) => "TerminologyCapabilities",
        Resource::ValueSet(_) => "ValueSet",
        Resource::Unknown(_) => "resource of another type",
    }
}

/// Splits the `tx-resource` parameters off `parameters`, returning the
/// operation's own parameters and the resources.
///
/// # Errors
///
/// Returns a `400` failure for a `tx-resource` parameter without a resource.
pub fn split_resources(parameters: Parameters) -> Result<(Parameters, Vec<Resource>), Failure> {
    let mut own = Vec::with_capacity(parameters.parameter.len());
    let mut resources = Vec::new();
    for parameter in parameters.parameter {
        if parameter.name.value.as_deref() == Some(TX_RESOURCE) {
            let resource = parameter.resource.ok_or_else(|| {
                Failure::new(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    format!("`{TX_RESOURCE}` must carry a resource"),
                )
            })?;
            resources.push(resource);
        } else {
            own.push(parameter);
        }
    }
    Ok((
        Parameters {
            parameter: own,
            ..parameters
        },
        resources,
    ))
}

/// The scope of a request: the cache the `X-Cache-Id` header names, then the
/// request's own `tx-resource`s on top.
///
/// # Errors
///
/// Returns a `404` failure for an unknown cache id and the failures of
/// [`Scope::layered`].
pub fn scope_of<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
    mut resources: Vec<Resource>,
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

/// The `X-Cache-Id` header, when present and well-formed.
///
/// # Errors
///
/// Returns a `400` failure when the header is present more than once or is
/// not text.
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
    resources: Arc<Vec<Resource>>,
    last_used: Instant,
}

/// The caches `$cache-control` started, by id, expiring after [`CACHE_IDLE`]
/// without use.
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
    /// Starts a cache holding `resources`; returns its id.
    pub fn start(&self, resources: Vec<Resource>) -> String {
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

    /// The resources of cache `id`, refreshing its idle timer.
    ///
    /// # Errors
    ///
    /// Returns a `404` failure when the id is unknown or expired.
    pub fn get(&self, id: &str) -> Result<Arc<Vec<Resource>>, Failure> {
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
    /// Returns a `404` failure when the id is unknown or expired.
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
