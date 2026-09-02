//! The `emit` command: packages in, generated crate out, byte-deterministic.
//!
//! One run emits every FHIR version in [`EmitOptions::versions`]: each
//! package lowers to its own module, and one `lib.rs` declares them all. The
//! pipeline renders every file into a scratch directory, formats it with the
//! pinned `rustfmt`, and then either replaces the generated tree or, in check
//! mode, compares it with the tree on disk and reports every difference.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::closure::{ClosureError, TypeClosure};
use crate::lower::{LowerError, VersionModule};
use crate::package::{LoadError, Package};
use crate::render::{render_lib, render_module, render_version_mod};
use crate::roots::{MissingRoot, RootSet};

/// One FHIR version to emit: its module name and its vendored package.
#[derive(Debug, Clone)]
pub struct VersionInput {
    /// The module name, for example `r4b`.
    pub module: String,
    /// The vendored package directory (the one holding `package/`).
    pub package_dir: PathBuf,
}

/// What to emit and where.
#[derive(Debug, Clone)]
pub struct EmitOptions {
    /// The versions to emit, in module-name order.
    pub versions: Vec<VersionInput>,
    /// The generated crate directory (the one holding `Cargo.toml` and `src/`).
    pub crate_dir: PathBuf,
    /// Compare instead of writing.
    pub check: bool,
}

/// A failure while emitting.
#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    /// No version was given.
    #[error("no FHIR version to emit")]
    NoVersions,
    /// The package did not load.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// A root resource is missing.
    #[error(transparent)]
    MissingRoot(#[from] MissingRoot),
    /// The closure did not compute.
    #[error(transparent)]
    Closure(#[from] ClosureError),
    /// Lowering failed.
    #[error(transparent)]
    Lower(#[from] LowerError),
    /// Rendering to a string failed.
    #[error("rendering failed")]
    Render(#[from] std::fmt::Error),
    /// A file could not be read or written.
    #[error("cannot access {path}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// `rustfmt` did not run or rejected the output.
    #[error("rustfmt failed: {stderr}")]
    Rustfmt {
        /// What rustfmt printed.
        stderr: String,
    },
    /// Check mode found the tree out of date.
    #[error("the generated crate is out of date; {} file(s) differ: {}", .paths.len(), .paths.join(", "))]
    Drift {
        /// The files that differ, are missing, or are stale.
        paths: Vec<String>,
    },
}

/// What an emit run produced.
#[derive(Debug)]
pub struct EmitReport {
    /// The number of types per version module.
    pub types: BTreeMap<String, usize>,
    /// The files written or checked, relative to `src/`.
    pub files: Vec<String>,
}

/// Runs the pipeline per `options`.
///
/// # Errors
///
/// Returns [`EmitError`] for a load, closure, lowering, I/O, or `rustfmt`
/// failure, and [`EmitError::Drift`] in check mode when the tree differs.
pub fn emit(options: &EmitOptions) -> Result<EmitReport, EmitError> {
    if options.versions.is_empty() {
        return Err(EmitError::NoVersions);
    }
    let mut versions: Vec<&VersionInput> = options.versions.iter().collect();
    versions.sort_by(|a, b| a.module.cmp(&b.module));

    let mut models = Vec::with_capacity(versions.len());
    for version in &versions {
        let package = Package::open(&version.package_dir)?;
        let roots = RootSet::select(&package)?;
        let closure = TypeClosure::compute(&package, &roots)?;
        models.push(VersionModule::lower(
            &closure,
            &version.module,
            &package.manifest().name,
            &package.manifest().version,
        )?);
    }

    let mut files = BTreeMap::new();
    files.insert(String::from("lib.rs"), render_lib(&models)?);
    for model in &models {
        files.insert(format!("{}/mod.rs", model.name), render_version_mod(model)?);
        for (module, types) in model.modules() {
            files.insert(
                format!("{}/{module}.rs", model.name),
                render_module(model, module, &types)?,
            );
        }
    }

    let scratch = tempfile::tempdir().map_err(|source| EmitError::Io {
        path: PathBuf::from("(temporary directory)"),
        source,
    })?;
    let scratch_src = scratch.path().join("src");
    write_tree(&scratch_src, &files)?;
    rustfmt(&scratch_src, &files, &options.crate_dir)?;
    let formatted = read_tree(&scratch_src, &files)?;

    let module_names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
    let target_src = options.crate_dir.join("src");
    if options.check {
        let drift = compare(&target_src, &formatted, &module_names)?;
        if !drift.is_empty() {
            return Err(EmitError::Drift { paths: drift });
        }
    } else {
        for module in &module_names {
            let version_dir = target_src.join(module);
            if version_dir.exists() {
                fs::remove_dir_all(&version_dir).map_err(|source| EmitError::Io {
                    path: version_dir.clone(),
                    source,
                })?;
            }
        }
        write_tree(&target_src, &formatted)?;
    }
    Ok(EmitReport {
        types: models
            .iter()
            .map(|m| (m.name.clone(), m.types.len()))
            .collect(),
        files: formatted.keys().cloned().collect(),
    })
}

fn write_tree(root: &Path, files: &BTreeMap<String, String>) -> Result<(), EmitError> {
    for (relative, content) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| EmitError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, content).map_err(|source| EmitError::Io { path, source })?;
    }
    Ok(())
}

fn read_tree(
    root: &Path,
    files: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, EmitError> {
    let mut out = BTreeMap::new();
    for relative in files.keys() {
        let path = root.join(relative);
        let content = fs::read_to_string(&path).map_err(|source| EmitError::Io {
            path: path.clone(),
            source,
        })?;
        out.insert(relative.clone(), content);
    }
    Ok(out)
}

fn rustfmt(
    root: &Path,
    files: &BTreeMap<String, String>,
    crate_dir: &Path,
) -> Result<(), EmitError> {
    let config = crate_dir.join("../../rustfmt.toml");
    let mut command = Command::new("rustfmt");
    command.arg("--edition").arg("2024");
    if config.is_file() {
        command.arg("--config-path").arg(&config);
    }
    for relative in files.keys() {
        command.arg(root.join(relative));
    }
    let output = command.output().map_err(|source| EmitError::Io {
        path: PathBuf::from("rustfmt"),
        source,
    })?;
    if !output.status.success() {
        return Err(EmitError::Rustfmt {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

/// The relative paths that differ between the rendered files and the tree on
/// disk, including files under a version directory that the model no longer
/// produces.
fn compare(
    target_src: &Path,
    files: &BTreeMap<String, String>,
    version_modules: &[&str],
) -> Result<Vec<String>, EmitError> {
    let mut drift = Vec::new();
    for (relative, content) in files {
        let path = target_src.join(relative);
        match fs::read_to_string(&path) {
            Ok(existing) if existing == *content => {}
            Ok(_) => drift.push(relative.clone()),
            Err(_) => drift.push(format!("{relative} (missing)")),
        }
    }
    for module in version_modules {
        let version_dir = target_src.join(module);
        if !version_dir.is_dir() {
            continue;
        }
        let entries = fs::read_dir(&version_dir).map_err(|source| EmitError::Io {
            path: version_dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| EmitError::Io {
                path: version_dir.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{module}/{name}");
            if !files.contains_key(&relative) {
                drift.push(format!("{relative} (stale)"));
            }
        }
    }
    drift.sort();
    Ok(drift)
}
