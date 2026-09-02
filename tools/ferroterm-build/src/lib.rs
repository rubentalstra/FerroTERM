//! The offline build: an RF2 release in, the served artifacts out.
//!
//! Runs once per SNOMED CT edition, outside the server process. It reads the
//! RF2 Snapshot through `ferroterm-rf2` and writes the memory-mapped store, the
//! materialized graph, and the description index that `ferroterm-server` opens
//! read-only.
#![doc(test(attr(deny(warnings))))]

use std::path::PathBuf;

use clap::Parser;

/// The command line of `ferroterm-build`.
#[derive(Debug, Parser)]
#[command(name = "ferroterm-build", version, about)]
pub struct Cli {
    /// The RF2 release directory (the one holding `Snapshot/`).
    #[arg(long, value_name = "DIR")]
    pub rf2: PathBuf,
    /// The directory to write the artifacts into.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
}

/// A build failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The pipeline is not part of the tool yet.
    #[error("the build pipeline is not implemented yet (tracked in rubentalstra/FerroTERM#6)")]
    PipelineMissing,
}

/// Runs the build the CLI describes.
///
/// # Errors
///
/// Returns [`Error::PipelineMissing`] until the pipeline exists.
pub fn run(cli: &Cli) -> Result<(), Error> {
    // TODO(#6): stream the RF2 release and write the store, graph, and text artifacts.
    let Cli { rf2: _, out: _ } = cli;
    Err(Error::PipelineMissing)
}
