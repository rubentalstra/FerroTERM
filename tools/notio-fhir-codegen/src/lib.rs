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

pub mod model;
pub mod package;
pub mod roots;
pub mod snapshot;

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
    Emit,
}

/// A generator failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The emitter is not part of the generator yet.
    #[error("the `emit` command has no emitter yet (tracked in rubentalstra/notio#4)")]
    EmitterMissing,
}

/// Runs the command the CLI selected.
///
/// # Errors
///
/// Returns [`Error::EmitterMissing`] for `emit` until the emitter exists.
pub fn run(cli: &Cli) -> Result<(), Error> {
    // TODO(#4): load the vendored R4B package and emit the root-set types.
    match cli.command {
        Command::Emit => Err(Error::EmitterMissing),
    }
}
