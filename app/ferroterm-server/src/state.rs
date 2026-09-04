//! The loaded server state: a registry of code system providers and the ids
//! their `CodeSystem` instances answer on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fhir_terminology::artifact::{self, ArtifactError};
use fhir_terminology::classification::{self, ClassificationProvider};
use fhir_terminology::conceptmap;
use fhir_terminology::conceptmap::store::ConceptMapStore;
use fhir_terminology::fhir_codesystem::load::{FhirVersion, load_dir, package_version};
use fhir_terminology::fhir_codesystem::model::CodeSystemModel;
use fhir_terminology::fhir_codesystem::provider::{BuildError, FhirCodeSystem};
use fhir_terminology::icd11::{self, Icd11Provider};
use fhir_terminology::loinc::{self, LoincProvider};
use fhir_terminology::operations::Sources;
use fhir_terminology::provider::{CodeSystemProvider, ContentMode, ProviderError};
use fhir_terminology::registries::ucum::provider::UcumProvider;
use fhir_terminology::registries::{bcp13, bcp47, iso3166};
use fhir_terminology::registry::{RegisterError, Registry, Resolved};
use fhir_terminology::rxnorm::{self, RxNormProvider};
use fhir_terminology::snomed::{self, OpenError, SnomedProvider};
use fhir_terminology::supplement::Supplement;
use fhir_terminology::valueset;
use fhir_terminology::valueset::model::ValueSetModel;
use fhir_terminology::valueset::store::ValueSetStore;
use fhir_terminology::versioned::Duplicate;

use crate::config::Config;
use crate::scope::Caches;

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
    /// A LOINC artifact directory does not open.
    #[error("cannot open the LOINC artifact at {path}")]
    OpenLoinc {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<loinc::OpenError>,
    },
    /// An `RxNorm` artifact directory does not open.
    #[error("cannot open the RxNorm artifact at {path}")]
    OpenRxNorm {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<rxnorm::OpenError>,
    },
    /// An ICD-11 artifact directory does not open.
    #[error("cannot open the ICD-11 artifact at {path}")]
    OpenIcd11 {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<icd11::OpenError>,
    },
    /// A classification artifact directory does not open.
    #[error("cannot open the classification artifact at {path}")]
    OpenClassification {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<classification::OpenError>,
    },
    /// An artifact's manifest does not say which system it serves.
    #[error("cannot read the artifact at {path}")]
    Artifact {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: ArtifactError,
    },
    /// An artifact serves a system this server has no provider for.
    #[error("the artifact at {path} serves `{system}`, which this server cannot open")]
    UnknownArtifact {
        /// The directory.
        path: PathBuf,
        /// The system.
        system: String,
    },
    /// A directory of `CodeSystem` resources does not load.
    #[error("cannot load the CodeSystem resources at {path}")]
    CodeSystems {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: Box<fhir_terminology::fhir_codesystem::load::LoadError>,
    },
    /// A `CodeSystem` resource does not build into a provider.
    #[error("cannot serve the CodeSystem `{url}` from {path}")]
    Build {
        /// The directory.
        path: PathBuf,
        /// The system.
        url: String,
        /// The cause.
        #[source]
        source: BuildError,
    },
    /// The vendored registry data of a registry code system does not build.
    #[error("cannot build the registry code system `{url}`")]
    Registry {
        /// The system.
        url: String,
        /// The cause.
        #[source]
        source: iso3166::DataError,
    },
    /// A supplement names a code system that is not loaded.
    #[error("the supplement `{url}` supplements `{target}`, which is not loaded")]
    SupplementTarget {
        /// The supplement.
        url: String,
        /// `CodeSystem.supplements`.
        target: String,
    },
    /// A supplement names no `supplements` canonical.
    #[error("the supplement `{url}` names no code system to supplement")]
    SupplementWithoutTarget {
        /// The supplement.
        url: String,
    },
    /// Two sources serve the same system version.
    #[error(transparent)]
    Register(#[from] RegisterError),
    /// Two sources carry the same value set or concept map version.
    #[error(transparent)]
    Duplicate(#[from] Duplicate),
}

/// One loaded code system version, for the startup summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSummary {
    /// The `CodeSystem` instance id.
    pub id: String,
    /// The system URI.
    pub url: String,
    /// The version.
    pub version: String,
    /// The concept count, when the system enumerates its concepts.
    pub concepts: Option<u64>,
    /// The designation languages.
    pub languages: Vec<String>,
    /// The directory the system was loaded from, when loaded from one.
    pub path: Option<PathBuf>,
}

/// What the handlers share.
#[derive(Debug)]
pub struct AppState {
    registry: Registry,
    value_sets: ValueSetStore,
    concept_maps: ConceptMapStore,
    /// `ValueSet` instance id to (url, version).
    value_set_instances: BTreeMap<String, (String, Option<String>)>,
    caches: Caches,
    /// `CodeSystem` instance id to (system, version).
    instances: BTreeMap<String, (String, String)>,
    /// The directory each version was loaded from.
    paths: BTreeMap<(String, String), PathBuf>,
    /// The software version reported in the capability statements.
    software_version: &'static str,
    /// The authentication the deployment declares, as codes of the FHIR
    /// `restful-security-service` value set.
    security_services: Vec<String>,
}

/// A provider before registration, with where it came from.
struct Loaded {
    path: PathBuf,
    provider: Arc<dyn CodeSystemProvider>,
}

impl AppState {
    /// Loads every artifact and every `CodeSystem` directory `config` names
    /// into a registry, supplements applied to the systems they name.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when a source does not load, a supplement names
    /// a system that is not loaded, or two sources serve the same system
    /// version. A server never starts on a bad index.
    pub fn load(config: &Config) -> Result<Self, LoadError> {
        let mut loaded = Vec::new();
        for path in &config.index {
            loaded.push(Loaded {
                path: path.clone(),
                provider: open_artifact(path, config)?,
            });
        }
        // NOTE: the registry systems ship with the server, so a validator finds BCP 47,
        // BCP 13, UCUM, and ISO 3166 without configuration
        // (<https://hl7.org/fhir/R4B/terminologies-systems.html>).
        loaded.push(Loaded {
            path: PathBuf::new(),
            provider: Arc::new(bcp47::Bcp47Provider::new()),
        });
        loaded.push(Loaded {
            path: PathBuf::new(),
            provider: Arc::new(bcp13::Bcp13Provider::new()),
        });
        loaded.push(Loaded {
            path: PathBuf::new(),
            provider: Arc::new(UcumProvider::new()),
        });
        loaded.push(Loaded {
            path: PathBuf::new(),
            provider: Arc::new(iso3166::provider().map_err(|source| LoadError::Registry {
                url: iso3166::URL.to_owned(),
                source,
            })?),
        });
        let mut supplements = Vec::new();
        let mut value_sets = ValueSetStore::new();
        let mut concept_maps = ConceptMapStore::new();
        for path in &config.code_systems {
            load_code_systems(
                path,
                &mut loaded,
                &mut supplements,
                &mut value_sets,
                &mut concept_maps,
            )?;
        }
        check_supplement_targets(&loaded, &supplements)?;
        let mut registry = Registry::new();
        let mut paths = BTreeMap::new();
        for Loaded { path, provider } in loaded {
            let identity = provider.identity();
            if !path.as_os_str().is_empty() {
                paths.insert((identity.url.clone(), identity.version.clone()), path);
            }
            registry.register(provider)?;
        }
        // NOTE: a loaded supplement stays dormant until a request names it
        // (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>).
        for (target, supplement) in supplements {
            registry.register_supplement(target, supplement);
        }
        let mut state = Self::from_registry(registry);
        state.paths = paths;
        state
            .security_services
            .clone_from(&config.security_services);
        for model in value_sets.iter() {
            let id = unique_id_of(
                &state.value_set_instances,
                instance_id(&model.url, model.version.as_deref().unwrap_or_default()),
            );
            state
                .value_set_instances
                .insert(id, (model.url.clone(), model.version.clone()));
        }
        state.value_sets = value_sets;
        state.concept_maps = concept_maps;
        Ok(state)
    }

    /// Wraps an already-built registry (tests and embedders).
    #[must_use]
    pub fn from_registry(registry: Registry) -> Self {
        let mut instances = BTreeMap::new();
        for url in registry.systems() {
            for provider in registry.versions(url) {
                let identity = provider.identity();
                let id = unique_id(&instances, instance_id(&identity.url, &identity.version));
                instances.insert(id, (identity.url.clone(), identity.version.clone()));
            }
        }
        Self {
            registry,
            value_sets: ValueSetStore::new(),
            concept_maps: ConceptMapStore::new(),
            value_set_instances: BTreeMap::new(),
            caches: Caches::default(),
            instances,
            paths: BTreeMap::new(),
            software_version: env!("CARGO_PKG_VERSION"),
            security_services: Vec::new(),
        }
    }

    /// What is loaded, one entry per code system version, sorted by id.
    ///
    /// # Errors
    ///
    /// Returns the provider's error when a concept count cannot be read.
    pub fn summaries(&self) -> Result<Vec<InstanceSummary>, ProviderError> {
        let mut out = Vec::new();
        for (id, (url, version)) in &self.instances {
            let Ok(resolved) = self.registry.resolve(url, Some(version)) else {
                continue;
            };
            let concepts = match resolved.provider.all() {
                Ok(set) => Some(set.len()),
                Err(ProviderError::NotEnumerable) => None,
                Err(error) => return Err(error),
            };
            out.push(InstanceSummary {
                id: id.clone(),
                url: url.clone(),
                version: version.clone(),
                concepts,
                languages: resolved.provider.declaration().languages.clone(),
                path: self.paths.get(&(url.clone(), version.clone())).cloned(),
            });
        }
        Ok(out)
    }

    /// The registry.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The loaded value sets.
    #[must_use]
    pub fn value_sets(&self) -> &ValueSetStore {
        &self.value_sets
    }

    /// The loaded concept maps.
    #[must_use]
    pub fn concept_maps(&self) -> &ConceptMapStore {
        &self.concept_maps
    }

    /// The caches `$cache-control` started.
    #[must_use]
    pub fn caches(&self) -> &Caches {
        &self.caches
    }

    /// The `ValueSet` instance ids and what they serve, sorted by id.
    pub fn value_set_instances(&self) -> impl Iterator<Item = (&str, &str, Option<&str>)> {
        self.value_set_instances
            .iter()
            .map(|(id, (url, version))| (id.as_str(), url.as_str(), version.as_deref()))
    }

    /// Resolves a `ValueSet` instance id.
    #[must_use]
    pub fn value_set_instance(&self, id: &str) -> Option<Arc<ValueSetModel>> {
        let (url, version) = self.value_set_instances.get(id)?;
        self.value_sets.resolve(url, version.as_deref())
    }

    /// The code systems and value sets an operation works over.
    #[must_use]
    pub fn sources(&self) -> Sources<'_> {
        Sources {
            registry: &self.registry,
            value_sets: &self.value_sets,
            concept_maps: &self.concept_maps,
        }
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

    /// The authentication the deployment declares, as codes of the FHIR
    /// `restful-security-service` value set; empty when it declares none.
    #[must_use]
    pub fn security_services(&self) -> &[String] {
        &self.security_services
    }

    /// The default provider of a system, for callers that need one.
    #[must_use]
    pub fn provider(&self, url: &str) -> Option<Arc<dyn CodeSystemProvider>> {
        self.registry.resolve(url, None).ok().map(|r| r.provider)
    }
}

/// Loads the `CodeSystem`, `ValueSet`, and `ConceptMap` resources in `path`:
/// complete systems become providers, supplements are collected for
/// dormant registration, value sets and concept maps go to their stores.
///
/// The FHIR version is the one the directory's `package.json` declares;
/// a plain directory of resources is read as R4B, the version the server
/// serves.
fn load_code_systems(
    path: &Path,
    loaded: &mut Vec<Loaded>,
    supplements: &mut Vec<(String, Supplement)>,
    value_sets: &mut ValueSetStore,
    concept_maps: &mut ConceptMapStore,
) -> Result<(), LoadError> {
    let failed = |source| LoadError::CodeSystems {
        path: path.to_path_buf(),
        source: Box::new(source),
    };
    let version = package_version(path)
        .map_err(failed)?
        .unwrap_or(FhirVersion::R4B);
    for model in load_dir(path, version).map_err(failed)? {
        if model.content == ContentMode::Supplement {
            let target =
                model
                    .supplements
                    .clone()
                    .ok_or_else(|| LoadError::SupplementWithoutTarget {
                        url: model.url.clone(),
                    })?;
            supplements.push((target, supplement_of(&model)));
            continue;
        }
        let url = model.url.clone();
        let provider = FhirCodeSystem::new(model).map_err(|source| LoadError::Build {
            path: path.to_path_buf(),
            url,
            source,
        })?;
        loaded.push(Loaded {
            path: path.to_path_buf(),
            provider: Arc::new(provider),
        });
    }
    for model in valueset::load::load_dir(path, version).map_err(failed)? {
        value_sets.insert(model)?;
    }
    for model in conceptmap::load::load_dir(path, version).map_err(failed)? {
        concept_maps.insert(model)?;
    }
    Ok(())
}

/// The supplement a `CodeSystem` resource describes.
pub(crate) fn supplement_of(model: &CodeSystemModel) -> Supplement {
    Supplement::from_code_system(model)
}

/// Every supplement names a loaded system (`CodeSystem.supplements`, a `url`
/// or `url|version` canonical,
/// <https://hl7.org/fhir/R4B/codesystem-definitions.html#CodeSystem.supplements>).
fn check_supplement_targets(
    loaded: &[Loaded],
    supplements: &[(String, Supplement)],
) -> Result<(), LoadError> {
    for (target, supplement) in supplements {
        let (url, version) = split_canonical(target);
        let served = loaded.iter().any(|l| {
            let identity = l.provider.identity();
            identity.url == url && version.is_none_or(|v| identity.version == v)
        });
        if !served {
            return Err(LoadError::SupplementTarget {
                url: supplement.url.clone(),
                target: target.clone(),
            });
        }
    }
    Ok(())
}

/// `url|version` split into its parts.
fn split_canonical(canonical: &str) -> (&str, Option<&str>) {
    match canonical.split_once('|') {
        Some((url, version)) => (url, Some(version)),
        None => (canonical, None),
    }
}

/// `wanted`, or `wanted` with a numeric suffix when the reduced id is taken.
fn unique_id(taken: &BTreeMap<String, (String, String)>, wanted: String) -> String {
    unique_id_of(taken, wanted)
}

/// `wanted`, or `wanted` with a numeric suffix when the reduced id is a key
/// of `taken`.
fn unique_id_of<V>(taken: &BTreeMap<String, V>, wanted: String) -> String {
    if !taken.contains_key(&wanted) {
        return wanted;
    }
    let stem: String = wanted.chars().take(60).collect();
    (2..=taken.len().saturating_add(2))
        .map(|n| format!("{stem}-{n}"))
        .find(|candidate| !taken.contains_key(candidate))
        .unwrap_or(wanted)
}

/// A FHIR resource id for a code system version.
///
/// The version URI when it carries the system (a SNOMED CT edition), otherwise
/// the system URL and the version, reduced to the id alphabet (`[A-Za-z0-9.-]`,
/// at most 64 characters, <https://hl7.org/fhir/R4B/datatypes.html#id>),
/// scheme dropped. No spec governs how a server names its instances: our own design.
#[must_use]
pub fn instance_id(url: &str, version: &str) -> String {
    let text = if version.starts_with(url) {
        version.to_owned()
    } else {
        format!("{url}-{version}")
    };
    let stripped = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(&text);
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

/// Opens the artifact directory `path` with the provider its manifest calls
/// for: SNOMED CT, LOINC, and `RxNorm` by system, a classification by kind.
fn open_artifact(path: &Path, config: &Config) -> Result<Arc<dyn CodeSystemProvider>, LoadError> {
    let described = artifact::describe(path).map_err(|source| LoadError::Artifact {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(match described.system.as_str() {
        snomed::SYSTEM => Arc::new(
            SnomedProvider::open(path, &config.default_language).map_err(|source| {
                LoadError::Open {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                }
            })?,
        ),
        loinc::SYSTEM => {
            Arc::new(
                LoincProvider::open(path).map_err(|source| LoadError::OpenLoinc {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                })?,
            )
        }
        rxnorm::SYSTEM => {
            Arc::new(
                RxNormProvider::open(path).map_err(|source| LoadError::OpenRxNorm {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                })?,
            )
        }
        _ if described.kind.as_deref() == Some(icd11::KIND) => Arc::new(
            Icd11Provider::open(path).map_err(|source| LoadError::OpenIcd11 {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?,
        ),
        _ if described.kind.as_deref() == Some(classification::KIND) => {
            Arc::new(ClassificationProvider::open(path).map_err(|source| {
                LoadError::OpenClassification {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                }
            })?)
        }
        other => {
            return Err(LoadError::UnknownArtifact {
                path: path.to_path_buf(),
                system: other.to_owned(),
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{instance_id, split_canonical};

    #[test]
    fn instance_ids_fit_the_fhir_id_alphabet() {
        assert_eq!(
            instance_id(
                "http://snomed.info/sct",
                "http://snomed.info/sct/11000146104/version/20260630"
            ),
            "snomed.info-sct-11000146104-version-20260630"
        );
        assert_eq!(
            instance_id("http://terminology.hl7.org/CodeSystem/v2-0001", "2.0.0"),
            "terminology.hl7.org-CodeSystem-v2-0001-2.0.0"
        );
        assert!(instance_id("http://example.org/x", &"x".repeat(100)).len() <= 64);
    }

    #[test]
    fn canonicals_split_on_the_version_bar() {
        assert_eq!(
            split_canonical("http://a.example/cs|2.0"),
            ("http://a.example/cs", Some("2.0"))
        );
        assert_eq!(
            split_canonical("http://a.example/cs"),
            ("http://a.example/cs", None)
        );
    }
}
