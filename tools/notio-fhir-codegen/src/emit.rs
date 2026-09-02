//! The `emit` command: package in, generated crate out, byte-deterministic.
//!
//! The pipeline renders every file into a scratch directory, formats it with
//! the pinned `rustfmt`, and then either replaces the generated tree or, in
//! check mode, compares it with the tree on disk and reports every
//! difference.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::closure::{ClosureError, TypeClosure};
use crate::lower::{LowerError, RustModel};
use crate::package::{LoadError, Package};
use crate::render::render_all;
use crate::roots::{MissingRoot, RootSet};

/// What to emit and where.
#[derive(Debug, Clone)]
pub struct EmitOptions {
    /// The vendored package directory (the one holding `package/`).
    pub package_dir: PathBuf,
    /// The generated crate directory (the one holding `Cargo.toml` and `src/`).
    pub crate_dir: PathBuf,
    /// The version module name, for example `r4b`.
    pub version_module: String,
    /// Compare instead of writing.
    pub check: bool,
}

/// A failure while emitting.
#[derive(Debug, thiserror::Error)]
pub enum EmitError {
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
    /// The number of types in the model.
    pub types: usize,
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
    let package = Package::open(&options.package_dir)?;
    let roots = RootSet::select(&package)?;
    let closure = TypeClosure::compute(&package, &roots)?;
    let model = RustModel::lower(
        &closure,
        &options.version_module,
        &package.manifest().name,
        &package.manifest().version,
    )?;
    let files = render_all(&model)?;

    let scratch = tempfile::tempdir().map_err(|source| EmitError::Io {
        path: PathBuf::from("(temporary directory)"),
        source,
    })?;
    let scratch_src = scratch.path().join("src");
    write_tree(&scratch_src, &files)?;
    rustfmt(&scratch_src, &files, &options.crate_dir)?;
    let formatted = read_tree(&scratch_src, &files)?;

    let target_src = options.crate_dir.join("src");
    if options.check {
        let drift = compare(&target_src, &formatted, &options.version_module)?;
        if !drift.is_empty() {
            return Err(EmitError::Drift { paths: drift });
        }
    } else {
        let version_dir = target_src.join(&options.version_module);
        if version_dir.exists() {
            fs::remove_dir_all(&version_dir).map_err(|source| EmitError::Io {
                path: version_dir.clone(),
                source,
            })?;
        }
        write_tree(&target_src, &formatted)?;
    }
    Ok(EmitReport {
        types: model.types.len(),
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
/// disk, including generated files on disk that the model no longer produces.
fn compare(
    target_src: &Path,
    files: &BTreeMap<String, String>,
    version_module: &str,
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
    let version_dir = target_src.join(version_module);
    if version_dir.is_dir() {
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
            let relative = format!("{version_module}/{name}");
            if !files.contains_key(&relative) {
                drift.push(format!("{relative} (stale)"));
            }
        }
    }
    drift.sort();
    Ok(drift)
}
