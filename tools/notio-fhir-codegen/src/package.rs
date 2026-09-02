//! Reading a vendored FHIR package from disk.
//!
//! A package directory is `<package>/package/*.json` as the FHIR package
//! registry ships it (<https://hl7.org/fhir/packages.html>). The loader
//! reads every JSON resource, keeps the conformance resources the generator
//! consumes, and indexes them by canonical URL in ordered maps so every walk
//! over the package is deterministic.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{
    CodeSystem, Derivation, OperationDefinition, ResourceHeader, StructureDefinition, ValueSet,
};

/// A failure while reading a package.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The package directory could not be listed or a file could not be read.
    #[error("cannot read {path}")]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// A resource file is not the JSON the model expects.
    #[error("cannot parse {path}")]
    Json {
        /// The file that failed to parse.
        path: PathBuf,
        /// The underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// Two resources of one type share a canonical URL.
    #[error("{resource_type} {url} is defined twice: {first} and {second}")]
    DuplicateCanonical {
        /// The resource type.
        resource_type: &'static str,
        /// The duplicated canonical URL.
        url: String,
        /// The file first defining it.
        first: PathBuf,
        /// The file defining it again.
        second: PathBuf,
    },
    /// The directory holds no `package.json`, so it is not a FHIR package.
    #[error("{path} is not a FHIR package directory (no package/package.json)")]
    NotAPackage {
        /// The directory that was opened.
        path: PathBuf,
    },
}

/// The `package.json` manifest fields the loader reads.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The package name, for example `hl7.fhir.r4b.core`.
    pub name: String,
    /// The package version, for example `4.3.0`.
    pub version: String,
    /// The FHIR versions the package content targets.
    #[serde(default)]
    pub fhir_versions: Vec<String>,
}

/// A loaded FHIR package: its manifest and the conformance resources.
#[derive(Debug)]
pub struct Package {
    root: PathBuf,
    manifest: Manifest,
    structure_definitions: BTreeMap<String, StructureDefinition>,
    operation_definitions: BTreeMap<String, OperationDefinition>,
    value_sets: BTreeMap<String, ValueSet>,
    code_systems: BTreeMap<String, CodeSystem>,
    sources: BTreeMap<String, PathBuf>,
}

impl Package {
    /// Opens the package vendored at `root` (the directory holding `package/`).
    ///
    /// # Errors
    ///
    /// Returns [`LoadError::NotAPackage`] when `package/package.json` is
    /// missing, an I/O or JSON error naming the file that failed, and
    /// [`LoadError::DuplicateCanonical`] when two files define one URL.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, LoadError> {
        let root = root.as_ref().to_path_buf();
        let content = root.join("package");
        let manifest_path = content.join("package.json");
        if !manifest_path.is_file() {
            return Err(LoadError::NotAPackage { path: root });
        }
        let manifest: Manifest = read_json(&manifest_path)?;

        let mut package = Self {
            root,
            manifest,
            structure_definitions: BTreeMap::new(),
            operation_definitions: BTreeMap::new(),
            value_sets: BTreeMap::new(),
            code_systems: BTreeMap::new(),
            sources: BTreeMap::new(),
        };
        for path in resource_files(&content)? {
            package.load_file(&path)?;
        }
        Ok(package)
    }

    fn load_file(&mut self, path: &Path) -> Result<(), LoadError> {
        let text = fs::read_to_string(path).map_err(|source| LoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let header: ResourceHeader = parse_json(path, &text)?;
        match header.resource_type.as_str() {
            "StructureDefinition" => {
                let resource: StructureDefinition = parse_json(path, &text)?;
                insert_unique(
                    &mut self.structure_definitions,
                    &mut self.sources,
                    "StructureDefinition",
                    resource.url.clone(),
                    resource,
                    path,
                )
            }
            "OperationDefinition" => {
                let resource: OperationDefinition = parse_json(path, &text)?;
                insert_unique(
                    &mut self.operation_definitions,
                    &mut self.sources,
                    "OperationDefinition",
                    resource.url.clone(),
                    resource,
                    path,
                )
            }
            "ValueSet" => {
                let resource: ValueSet = parse_json(path, &text)?;
                insert_unique(
                    &mut self.value_sets,
                    &mut self.sources,
                    "ValueSet",
                    resource.url.clone(),
                    resource,
                    path,
                )
            }
            "CodeSystem" => {
                let resource: CodeSystem = parse_json(path, &text)?;
                insert_unique(
                    &mut self.code_systems,
                    &mut self.sources,
                    "CodeSystem",
                    resource.url.clone(),
                    resource,
                    path,
                )
            }
            _ => Ok(()),
        }
    }

    /// The directory the package was opened from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The `package.json` manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Every `StructureDefinition`, keyed by canonical URL.
    #[must_use]
    pub fn structure_definitions(&self) -> &BTreeMap<String, StructureDefinition> {
        &self.structure_definitions
    }

    /// Every `OperationDefinition`, keyed by canonical URL.
    #[must_use]
    pub fn operation_definitions(&self) -> &BTreeMap<String, OperationDefinition> {
        &self.operation_definitions
    }

    /// Every `ValueSet`, keyed by canonical URL.
    #[must_use]
    pub fn value_sets(&self) -> &BTreeMap<String, ValueSet> {
        &self.value_sets
    }

    /// Every `CodeSystem`, keyed by canonical URL.
    #[must_use]
    pub fn code_systems(&self) -> &BTreeMap<String, CodeSystem> {
        &self.code_systems
    }

    /// The file a resource was loaded from, by resource type and canonical URL.
    #[must_use]
    pub fn source_of(&self, resource_type: &str, url: &str) -> Option<&Path> {
        self.sources
            .get(&source_key(resource_type, url))
            .map(PathBuf::as_path)
    }

    /// The type definition named `name`, for example `ValueSet`.
    ///
    /// Core packages name each type's defining structure after the type. A
    /// constraint profile may share the name (the `rendering-xhtml` extension
    /// is named `xhtml`), so only specializations and base definitions match.
    #[must_use]
    pub fn structure_definition_named(&self, name: &str) -> Option<&StructureDefinition> {
        self.structure_definitions.values().find(|definition| {
            definition.name == name && definition.derivation != Some(Derivation::Constraint)
        })
    }
}

/// The JSON resource files of a package content directory, in sorted order.
///
/// `package.json` and the registry's `.index.json` are not resources.
fn resource_files(content: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let entries = fs::read_dir(content).map_err(|source| LoadError::Io {
        path: content.to_path_buf(),
        source,
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LoadError::Io {
            path: content.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let is_resource = path.extension().is_some_and(|ext| ext == "json")
            && path
                .file_name()
                .is_some_and(|name| name != "package.json" && name != ".index.json");
        if is_resource && path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, LoadError> {
    let text = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_json(path, &text)
}

fn parse_json<T: for<'de> Deserialize<'de>>(path: &Path, text: &str) -> Result<T, LoadError> {
    serde_json::from_str(text).map_err(|source| LoadError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, T>,
    sources: &mut BTreeMap<String, PathBuf>,
    resource_type: &'static str,
    url: String,
    resource: T,
    path: &Path,
) -> Result<(), LoadError> {
    let key = source_key(resource_type, &url);
    if let Some(first) = sources.get(&key) {
        return Err(LoadError::DuplicateCanonical {
            resource_type,
            url,
            first: first.clone(),
            second: path.to_path_buf(),
        });
    }
    sources.insert(key, path.to_path_buf());
    map.insert(url, resource);
    Ok(())
}

fn source_key(resource_type: &str, url: &str) -> String {
    format!("{resource_type} {url}")
}
