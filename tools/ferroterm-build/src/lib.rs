//! The offline build: an RF2 release in, the served artifacts out.
//!
//! Runs once per SNOMED CT edition, outside the server process. It reads the
//! RF2 Snapshot through `ferroterm-rf2` and writes one `redb` store holding the
//! concepts, designations, acceptabilities, and properties, with the hierarchy
//! (`ferroterm-graph`) and the designation index (`ferroterm-text`) in its blob
//! slots, plus a manifest naming the edition the store was built from. Two runs
//! over the same release write byte-identical files: every collection is
//! sorted by identifier before it is numbered, and nothing records a clock.
#![doc(test(attr(deny(warnings))))]

pub mod pipeline;

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

/// Runs the build the CLI describes.
///
/// # Errors
///
/// Returns [`pipeline::Error`] when the release does not read, the edition
/// cannot be identified, or an artifact cannot be written.
pub fn run(cli: &Cli) -> Result<pipeline::Report, pipeline::Error> {
    pipeline::build(&cli.rf2, &cli.out)
}
