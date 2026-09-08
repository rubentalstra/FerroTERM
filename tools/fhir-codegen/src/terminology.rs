//! The `terminology` command: the terminology the FHIR specification defines,
//! from the vendored core packages into the bundles the server embeds.
//!
//! Each FHIR version publishes its own code systems and value sets in its core
//! package ("Code Systems Defined by this specification" and "Value Sets
//! Defined by this specification",
//! <https://hl7.org/fhir/R4B/terminologies-systems.html> and
//! <https://hl7.org/fhir/R4B/terminologies-valuesets.html>). This command
//! projects them onto the elements the terminology engine reads and writes one
//! bundle per version, byte-deterministic, so the server holds them without a
//! deployment supplying anything.
//!
//! Two rules bound what a bundle carries.
//!
//! A `CodeSystem` is FHIR's own content when its canonical is under
//! `http://hl7.org/fhir/`, it is not one of the `http://hl7.org/fhir/sid/`
//! identifiers the specification assigns to external code systems, and its
//! `content` is `complete`.
//!
//! A resource that states no `status`, which both resources require (1..1),
//! is left out and counted in the run's report, so the omission is visible.
//!
//! A `ValueSet` joins the bundle only when every system its compose names is
//! one of those code systems and every value set it references is in the
//! bundle too. Dozens of the specification's value sets enumerate SNOMED CT,
//! LOINC, or other licensed codes inline; this repository distributes none of
//! that content (`.claude/rules/vendored-inputs.md`), so the closure rule
//! keeps the bundles to the specification's own CC0 terminology by
//! construction.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::emit::VersionInput;

/// The canonical prefix of the terminology the FHIR specification defines.
const FHIR_PREFIX: &str = "http://hl7.org/fhir/";
/// The canonical prefix of the identifiers the specification assigns to code
/// systems it does not define (<https://hl7.org/fhir/R4B/identifier-registry.html>).
const SID_PREFIX: &str = "http://hl7.org/fhir/sid/";
/// The canonical prefix of the value sets the specification defines.
const VALUE_SET_PREFIX: &str = "http://hl7.org/fhir/ValueSet/";

/// The `CodeSystem` elements `fhir_terminology`'s code system model reads.
///
/// `extension` stays because the standards-status extension is read from it;
/// narrative and publication metadata the engine never reads are dropped.
const CODE_SYSTEM_ELEMENTS: [&str; 18] = [
    "caseSensitive",
    "compositional",
    "concept",
    "content",
    "experimental",
    "extension",
    "filter",
    "hierarchyMeaning",
    "language",
    "name",
    "property",
    "resourceType",
    "status",
    "supplements",
    "title",
    "url",
    "version",
    "versionNeeded",
];

/// The `ValueSet` elements `fhir_terminology`'s value set model reads.
const VALUE_SET_ELEMENTS: [&str; 16] = [
    "compose",
    "contained",
    "copyright",
    "date",
    "description",
    "experimental",
    "extension",
    "immutable",
    "language",
    "name",
    "publisher",
    "resourceType",
    "status",
    "title",
    "url",
    "version",
];

/// The file a version's code systems are written to.
pub const CODE_SYSTEMS_FILE: &str = "code-systems.json";
/// The file a version's value sets are written to.
pub const VALUE_SETS_FILE: &str = "value-sets.json";

/// What to bundle and where.
#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// The versions to bundle, in module-name order.
    pub versions: Vec<VersionInput>,
    /// The directory holding one subdirectory per version module.
    pub data_dir: PathBuf,
    /// Compare instead of writing.
    pub check: bool,
}

/// What one version's bundle holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BundleCounts {
    /// The code systems bundled.
    pub code_systems: usize,
    /// The value sets bundled.
    pub value_sets: usize,
    /// The resources left out because they state no `status`.
    pub without_status: usize,
    /// The code systems left out because they define one code twice.
    pub with_duplicate_code: usize,
}

/// What one bundling run produced, per version module.
#[derive(Debug)]
pub struct BundleReport {
    /// What each version's bundle holds, by module name.
    pub counts: BTreeMap<String, BundleCounts>,
}

/// A failure while bundling.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    /// No version was given.
    #[error("no FHIR version to bundle")]
    NoVersions,
    /// A file could not be read or written.
    #[error("cannot access {path}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// Two resources of one type share a canonical, so the bundle cannot
    /// hold both.
    #[error("{url} is defined twice in {package}")]
    DuplicateCanonical {
        /// The package directory.
        package: PathBuf,
        /// The canonical.
        url: String,
    },
    /// Check mode found a bundle out of date.
    #[error("the FHIR core terminology bundles are out of date; {} file(s) differ: {}", .paths.len(), .paths.join(", "))]
    Drift {
        /// The files that differ or are missing.
        paths: Vec<String>,
    },
}

/// Bundles every version per `options`.
///
/// # Errors
///
/// Returns [`BundleError`] for a read, parse, or write failure, and
/// [`BundleError::Drift`] in check mode when a bundle on disk differs.
pub fn bundle(options: &BundleOptions) -> Result<BundleReport, BundleError> {
    if options.versions.is_empty() {
        return Err(BundleError::NoVersions);
    }
    let mut versions: Vec<&VersionInput> = options.versions.iter().collect();
    versions.sort_by(|a, b| a.module.cmp(&b.module));

    let mut counts = BTreeMap::new();
    let mut drift = Vec::new();
    for version in versions {
        let package = version.package_dir.join("package");
        let Selection {
            code_systems,
            value_sets,
            without_status,
            with_duplicate_code,
        } = select(&package)?;
        counts.insert(
            version.module.clone(),
            BundleCounts {
                code_systems: code_systems.len(),
                value_sets: value_sets.len(),
                without_status,
                with_duplicate_code,
            },
        );
        let dir = options.data_dir.join(&version.module);
        for (name, resources, elements) in [
            (
                CODE_SYSTEMS_FILE,
                &code_systems,
                CODE_SYSTEM_ELEMENTS.as_slice(),
            ),
            (VALUE_SETS_FILE, &value_sets, VALUE_SET_ELEMENTS.as_slice()),
        ] {
            let text = render(resources, elements);
            let path = dir.join(name);
            if options.check {
                if fs::read_to_string(&path).ok() != Some(text) {
                    drift.push(path.display().to_string());
                }
            } else {
                write(&path, &text)?;
            }
        }
    }
    if !drift.is_empty() {
        return Err(BundleError::Drift { paths: drift });
    }
    Ok(BundleReport { counts })
}

/// What one package contributes, by canonical.
struct Selection {
    code_systems: BTreeMap<String, serde_json::Value>,
    value_sets: BTreeMap<String, serde_json::Value>,
    without_status: usize,
    with_duplicate_code: usize,
}

/// The code systems and value sets one package contributes, by canonical.
fn select(package: &Path) -> Result<Selection, BundleError> {
    let mut code_systems = BTreeMap::new();
    let mut value_sets = BTreeMap::new();
    let mut without_status = 0;
    let mut with_duplicate_code = 0;
    for resource in read_resources(package)? {
        let kind = resource
            .get("resourceType")
            .and_then(serde_json::Value::as_str);
        let target = match kind {
            Some("CodeSystem") => &mut code_systems,
            Some("ValueSet") => &mut value_sets,
            _ => continue,
        };
        let url = resource
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        // NOTE: a conformance resource without a canonical cannot be
        // referenced, so it is not terminology this server can serve
        // (<https://hl7.org/fhir/R4B/resource.html#canonical>).
        let Some(url) = url else { continue };
        if kind == Some("CodeSystem") && !is_fhir_code_system(&resource, &url) {
            continue;
        }
        if kind == Some("ValueSet") && !url.starts_with(VALUE_SET_PREFIX) {
            continue;
        }
        // NOTE: `status` is 1..1 (<https://hl7.org/fhir/R4B/codesystem.html>),
        // so a resource stating none is no terminology a server can serve.
        if resource
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_none()
        {
            without_status += 1;
            continue;
        }
        // NOTE: a code is unique in its code system
        // (<https://hl7.org/fhir/R4B/codesystem.html>), so a resource that
        // defines one twice names two concepts a request cannot tell apart.
        if kind == Some("CodeSystem") && !codes_are_unique(&resource) {
            with_duplicate_code += 1;
            continue;
        }
        if target.insert(url.clone(), resource).is_some() {
            return Err(BundleError::DuplicateCanonical {
                package: package.to_path_buf(),
                url,
            });
        }
    }
    let systems: BTreeSet<&str> = code_systems.keys().map(String::as_str).collect();
    retain_self_contained(&mut value_sets, &systems);
    Ok(Selection {
        code_systems,
        value_sets,
        without_status,
        with_duplicate_code,
    })
}

/// Whether every `concept.code` a `CodeSystem` defines appears once.
fn codes_are_unique(resource: &serde_json::Value) -> bool {
    let mut seen = BTreeSet::new();
    collect_codes(resource.get("concept"), &mut seen)
}

/// Walks a concept tree, returning `false` at the first repeated code.
fn collect_codes(concepts: Option<&serde_json::Value>, seen: &mut BTreeSet<String>) -> bool {
    let Some(concepts) = concepts.and_then(serde_json::Value::as_array) else {
        return true;
    };
    for concept in concepts {
        if let Some(code) = concept.get("code").and_then(serde_json::Value::as_str)
            && !seen.insert(code.to_owned())
        {
            return false;
        }
        if !collect_codes(concept.get("concept"), seen) {
            return false;
        }
    }
    true
}

/// Whether a `CodeSystem` is content the FHIR specification itself defines.
fn is_fhir_code_system(resource: &serde_json::Value, url: &str) -> bool {
    url.starts_with(FHIR_PREFIX)
        && !url.starts_with(SID_PREFIX)
        && resource.get("content").and_then(serde_json::Value::as_str) == Some("complete")
}

/// Drops every value set that reaches outside `systems` and the value sets
/// that survive, to the fixed point.
fn retain_self_contained(
    value_sets: &mut BTreeMap<String, serde_json::Value>,
    systems: &BTreeSet<&str>,
) {
    loop {
        let kept: BTreeSet<String> = value_sets.keys().cloned().collect();
        let dropped: Vec<String> = value_sets
            .iter()
            .filter(|(_, resource)| !self_contained(resource, systems, &kept))
            .map(|(url, _)| url.clone())
            .collect();
        if dropped.is_empty() {
            return;
        }
        for url in dropped {
            value_sets.remove(&url);
        }
    }
}

/// Whether every system a value set's compose names is in `systems` and every
/// value set it references is in `kept`.
fn self_contained(
    resource: &serde_json::Value,
    systems: &BTreeSet<&str>,
    kept: &BTreeSet<String>,
) -> bool {
    let Some(compose) = resource.get("compose") else {
        return true;
    };
    for group in ["include", "exclude"] {
        let Some(criteria) = compose.get(group).and_then(serde_json::Value::as_array) else {
            continue;
        };
        for criterion in criteria {
            if let Some(system) = criterion.get("system").and_then(serde_json::Value::as_str)
                && !systems.contains(system)
            {
                return false;
            }
            let referenced = criterion
                .get("valueSet")
                .and_then(serde_json::Value::as_array);
            for reference in referenced.into_iter().flatten() {
                let Some(reference) = reference.as_str() else {
                    return false;
                };
                let url = reference.split('|').next().unwrap_or(reference);
                if !kept.contains(url) {
                    return false;
                }
            }
        }
    }
    true
}

/// Every JSON resource under `package`, in file-name order.
fn read_resources(package: &Path) -> Result<Vec<serde_json::Value>, BundleError> {
    let mut paths: Vec<PathBuf> = fs::read_dir(package)
        .map_err(|source| BundleError::Io {
            path: package.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .collect();
    paths.sort();
    let mut resources = Vec::new();
    for path in paths {
        let text = fs::read_to_string(&path).map_err(|source| BundleError::Io {
            path: path.clone(),
            source,
        })?;
        // NOTE: a package directory holds non-resource JSON (package.json,
        // .index.json), which is not a parse failure of a resource.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if value.get("resourceType").is_some() {
            resources.push(value);
        }
    }
    Ok(resources)
}

/// The bundle text: a JSON array with one resource per line, each projected
/// onto `elements`, in canonical order.
fn render(resources: &BTreeMap<String, serde_json::Value>, elements: &[&str]) -> String {
    let mut lines = Vec::with_capacity(resources.len());
    for resource in resources.values() {
        let projected: serde_json::Map<String, serde_json::Value> = resource
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(name, _)| elements.contains(&name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        lines.push(serde_json::Value::Object(projected).to_string());
    }
    if lines.is_empty() {
        return String::from("[]\n");
    }
    format!("[\n{}\n]\n", lines.join(",\n"))
}

/// Writes `text` to `path`, creating the directory.
fn write(path: &Path, text: &str) -> Result<(), BundleError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| BundleError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, text).map_err(|source| BundleError::Io {
        path: path.to_path_buf(),
        source,
    })
}
