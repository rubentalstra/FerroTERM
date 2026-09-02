//! The `notio-fhir` generator.
//!
//! Reads the vendored, pinned HL7 FHIR packages under `vendor/` and emits the
//! per-version Rust modules of `crates/notio-fhir`: the terminology root-set
//! types and the terminology operation contracts. The output is
//! byte-deterministic so the CI drift check can regenerate and compare.
//!
//! The pipeline is [`package::Package`] (read a package), then
//! [`snapshot::ResolvedStructure`] (resolve each structure's snapshot) and
//! [`roots::RootSet`] (select what to emit).
#![doc(test(attr(deny(warnings))))]

pub mod closure;
pub mod emit;
pub mod lower;
pub mod model;
pub mod naming;
pub mod package;
pub mod render;
pub mod roots;
pub mod snapshot;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The command line of `notio-fhir-codegen`.
#[derive(Debug, Parser)]
#[command(name = "notio-fhir-codegen", version, about)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The generator's subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Regenerates `crates/notio-fhir` from the vendored packages.
    Emit {
        /// Compare the generated crate with what the emitter produces and fail on any difference.
        #[arg(long)]
        check: bool,
        /// The vendored package directory.
        #[arg(long, value_name = "DIR", default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/hl7.fhir.r4b.core"))]
        package: PathBuf,
        /// The generated crate directory.
        #[arg(long, value_name = "DIR", default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/notio-fhir"))]
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

/// Runs the command the CLI selected.
///
/// # Errors
///
/// Returns [`Error::Emit`] when the pipeline fails or, in check mode, when the
/// generated crate differs from what the emitter produces.
pub fn run(cli: &Cli) -> Result<emit::EmitReport, Error> {
    match &cli.command {
        Command::Emit {
            check,
            package,
            out,
        } => Ok(emit::emit(&emit::EmitOptions {
            package_dir: package.clone(),
            crate_dir: out.clone(),
            version_module: String::from("r4b"),
            check: *check,
        })?),
    }
}
