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
pub mod loinc;
pub mod pipeline;

use std::path::PathBuf;

use clap::Parser;

/// The command line of `ferroterm-build`.
#[derive(Debug, Parser)]
#[command(name = "ferroterm-build", version, about)]
pub struct Cli {
    /// The SNOMED CT RF2 release: the directory holding `Snapshot/`, or the release zip.
    #[arg(
        long,
        value_name = "DIR_OR_ZIP",
        conflicts_with = "loinc",
        required_unless_present = "loinc"
    )]
    pub rf2: Option<PathBuf>,
    /// The LOINC release: the unpacked `Loinc_<version>` directory, or the release zip.
    #[arg(long, value_name = "DIR_OR_ZIP")]
    pub loinc: Option<PathBuf>,
    /// The LOINC version to record when the release does not say (`2.82`).
    #[arg(long, value_name = "VERSION", requires = "loinc")]
    pub loinc_version: Option<String>,
    /// The directory to write the artifacts into.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
}

/// Runs the build the CLI describes.
///
/// A zip is unpacked (the Snapshot tree of an RF2 release, the tables of a
/// LOINC release) to a temporary directory that is removed when the build
/// ends; a directory is read in place.
///
/// # Errors
///
/// Returns [`RunError`] when the zip does not unpack, the release does not
/// read, the edition cannot be identified, or an artifact cannot be written.
pub fn run(cli: &Cli) -> Result<Report, RunError> {
    if let Some(loinc) = &cli.loinc {
        let scratch;
        let root = if loinc.is_file() {
            scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
            archive::unpack_loinc(loinc, scratch.path())?
        } else {
            loinc.clone()
        };
        let version = cli
            .loinc_version
            .clone()
            .or_else(|| loinc::version_from_name(loinc));
        return Ok(Report::Loinc(loinc::build(
            &root,
            version.as_deref(),
            &cli.out,
        )?));
    }
    let Some(rf2) = &cli.rf2 else {
        return Err(RunError::NoInput);
    };
    if rf2.is_file() {
        let scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
        let root = archive::unpack_snapshot(rf2, scratch.path())?;
        return Ok(Report::Snomed(pipeline::build(&root, &cli.out)?));
    }
    Ok(Report::Snomed(pipeline::build(rf2, &cli.out)?))
}

/// What a build wrote.
#[derive(Debug)]
pub enum Report {
    /// A SNOMED CT edition.
    Snomed(pipeline::Report),
    /// A LOINC release.
    Loinc(loinc::Report),
}

/// A failure of the command as a whole.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// Neither `--rf2` nor `--loinc` was given.
    #[error("give `--rf2` or `--loinc`")]
    NoInput,
    /// The release zip does not unpack.
    #[error(transparent)]
    Archive(#[from] archive::ArchiveError),
    /// No temporary directory for the unpacked release.
    #[error("cannot create a temporary directory for the release")]
    Scratch(#[source] std::io::Error),
    /// The SNOMED CT build failed.
    #[error(transparent)]
    Build(#[from] pipeline::Error),
    /// The LOINC build failed.
    #[error(transparent)]
    Loinc(#[from] loinc::Error),
}
