//! The `ferroterm-fhir` generator.
//!
//! Reads the vendored, pinned HL7 FHIR packages under `vendor/` and emits the
//! per-version Rust modules of `crates/ferroterm-fhir`: the terminology root-set
//! types and the terminology operation contracts. The output is
//! byte-deterministic so the CI drift check can regenerate and compare.
//!
//! The pipeline is [`package::Package`] (read a package), then
//! [`snapshot::ResolvedStructure`] (resolve each structure's snapshot),
//! [`roots::RootSet`] (select what to emit), [`closure::TypeClosure`] (the
//! root-set closure), [`lower::VersionModule`] (the generated module), [`render`]
//! (source text), and [`emit`] (write or check).
#![doc(test(attr(deny(warnings))))]

pub mod closure;
pub mod emit;
pub mod fhir;
pub mod lower;
pub mod naming;
pub mod operations;
pub mod package;
pub mod render;
pub mod render_codec;
pub mod roots;
pub mod snapshot;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

/// The FHIR versions the generator emits: module name and vendored package.
///
/// Every entry is emitted on each run so the generated crate always carries
/// the whole set (`codegen.md`).
pub const VERSIONS: [(&str, &str); 3] = [
    ("r4", "hl7.fhir.r4.core"),
    ("r4b", "hl7.fhir.r4b.core"),
    ("r5", "hl7.fhir.r5.core"),
];

/// The command line of `ferroterm-fhir-codegen`.
#[derive(Debug, Parser)]
#[command(name = "ferroterm-fhir-codegen", version, about)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The generator's subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Regenerates `crates/ferroterm-fhir` from the vendored packages.
    Emit {
        /// Compare the generated crate with what the emitter produces and fail on any difference.
        #[arg(long)]
        check: bool,
        /// The directory holding the vendored packages.
        #[arg(long, value_name = "DIR", default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor"))]
        vendor: PathBuf,
        /// The generated crate directory.
        #[arg(long, value_name = "DIR", default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/ferroterm-fhir"))]
        out: PathBuf,
    },
}

/// A generator failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The emit pipeline failed.
    #[error(transparent)]
    Emit(#[from] emit::EmitError),
}

/// The emit inputs for every entry of [`VERSIONS`] under `vendor`.
#[must_use]
pub fn version_inputs(vendor: &Path) -> Vec<emit::VersionInput> {
    VERSIONS
        .iter()
        .map(|(module, package)| emit::VersionInput {
            module: (*module).to_owned(),
            package_dir: vendor.join(package),
        })
        .collect()
}

/// Runs the command the CLI selected.
///
/// # Errors
///
/// Returns [`Error::Emit`] when the pipeline fails or, in check mode, when the
/// generated crate differs from what the emitter produces.
pub fn run(cli: &Cli) -> Result<emit::EmitReport, Error> {
    match &cli.command {
        Command::Emit { check, vendor, out } => Ok(emit::emit(&emit::EmitOptions {
            versions: version_inputs(vendor),
            crate_dir: out.clone(),
            check: *check,
        })?),
    }
}
