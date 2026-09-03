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

pub mod archive;
pub mod pipeline;

use std::path::PathBuf;

use clap::Parser;

/// The command line of `ferroterm-build`.
#[derive(Debug, Parser)]
#[command(name = "ferroterm-build", version, about)]
pub struct Cli {
    /// The RF2 release: the directory holding `Snapshot/`, or the release zip.
    #[arg(long, value_name = "DIR_OR_ZIP")]
    pub rf2: PathBuf,
    /// The directory to write the artifacts into.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
}

/// Runs the build the CLI describes.
///
/// A zip is unpacked (its `Snapshot/` tree only) to a temporary directory
/// that is removed when the build ends; a directory is read in place.
///
/// # Errors
///
/// Returns [`RunError`] when the zip does not unpack, the release does not
/// read, the edition cannot be identified, or an artifact cannot be written.
pub fn run(cli: &Cli) -> Result<pipeline::Report, RunError> {
    if cli.rf2.is_file() {
        let scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
        let root = archive::unpack_snapshot(&cli.rf2, scratch.path())?;
        return Ok(pipeline::build(&root, &cli.out)?);
    }
    Ok(pipeline::build(&cli.rf2, &cli.out)?)
}

/// A failure of the command as a whole.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The release zip does not unpack.
    #[error(transparent)]
    Archive(#[from] archive::ArchiveError),
    /// No temporary directory for the unpacked Snapshot.
    #[error("cannot create a temporary directory for the release")]
    Scratch(#[source] std::io::Error),
    /// The build failed.
    #[error(transparent)]
    Build(#[from] pipeline::Error),
}
