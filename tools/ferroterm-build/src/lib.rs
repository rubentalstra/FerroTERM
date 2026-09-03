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
pub mod classification;
pub mod loinc;
pub mod pipeline;
pub mod rxnorm;

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
        conflicts_with_all = ["loinc", "claml", "icd10cm", "rxnorm"],
        required_unless_present_any = ["loinc", "claml", "icd10cm", "rxnorm"]
    )]
    pub rf2: Option<PathBuf>,
    /// The LOINC release: the unpacked `Loinc_<version>` directory, or the release zip.
    #[arg(long, value_name = "DIR_OR_ZIP", conflicts_with_all = ["claml", "icd10cm", "rxnorm"])]
    pub loinc: Option<PathBuf>,
    /// The LOINC version to record when the release does not say (`2.82`).
    #[arg(long, value_name = "VERSION", requires = "loinc")]
    pub loinc_version: Option<String>,
    /// A `ClaML` classification (WHO ICD-10, a national ICD-10, ICPC-2): the XML file, or a zip holding it.
    #[arg(long, value_name = "XML_OR_ZIP", conflicts_with_all = ["icd10cm", "rxnorm"], requires = "system")]
    pub claml: Option<PathBuf>,
    /// The code system URI the `ClaML` classification is served as (`http://hl7.org/fhir/sid/icd-10`).
    #[arg(long, value_name = "URI", requires = "claml")]
    pub system: Option<String>,
    /// The version to record when the `ClaML` title does not say (`2021`).
    #[arg(long, value_name = "VERSION", requires = "claml")]
    pub claml_version: Option<String>,
    /// The ICD-10-CM release: the directories or zips holding the tabular XML and the order file (repeatable).
    #[arg(long, value_name = "DIR_OR_ZIP", action = clap::ArgAction::Append, conflicts_with = "rxnorm")]
    pub icd10cm: Vec<PathBuf>,
    /// The `RxNorm` release: the unpacked directory holding `rrf/`, or the release zip (the full release or the Current Prescribable Content).
    #[arg(long, value_name = "DIR_OR_ZIP")]
    pub rxnorm: Option<PathBuf>,
    /// The release date to record when the release does not say (`09082026`).
    #[arg(long, value_name = "MMDDYYYY", requires = "rxnorm")]
    pub rxnorm_version: Option<String>,
    /// The `RxNorm` sources (`SAB`) whose names are kept beside the unrestricted `RXNORM` and `MTHSPL` (a full release under a UMLS licence).
    #[arg(long, value_name = "SAB", value_delimiter = ',', action = clap::ArgAction::Append, requires = "rxnorm")]
    pub rxnorm_sources: Vec<String>,
    /// The directory to write the artifacts into.
    #[arg(long, value_name = "DIR")]
    pub out: PathBuf,
}

/// Runs the build the CLI describes.
///
/// A zip is unpacked (the Snapshot tree of an RF2 release, the tables of a
/// LOINC release, the XML of a `ClaML` classification, the two files of an
/// ICD-10-CM release, the `RRF` tables of an `RxNorm` release) to a temporary directory that is removed when the
/// build ends; a directory is read in place.
///
/// # Errors
///
/// Returns [`RunError`] when the zip does not unpack, the release does not
/// read, the edition cannot be identified, or an artifact cannot be written.
pub fn run(cli: &Cli) -> Result<Report, RunError> {
    if let Some(claml) = &cli.claml {
        let system = cli.system.as_deref().ok_or(RunError::NoSystem)?;
        let scratch;
        let file = if claml
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        {
            scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
            archive::unpack_claml(claml, scratch.path())?
        } else {
            claml.clone()
        };
        let classification = ferroterm_classification::claml::read_file(&file)?;
        return Ok(Report::Classification(classification::build(
            &classification,
            system,
            cli.claml_version.as_deref(),
            &cli.out,
        )?));
    }
    if let Some(rxnorm) = &cli.rxnorm {
        let scratch;
        let root = if rxnorm.is_file() {
            scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
            archive::unpack_rxnorm(rxnorm, scratch.path())?
        } else {
            rxnorm.clone()
        };
        let version = cli.rxnorm_version.clone().or_else(|| {
            rxnorm
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.trim_end_matches(".zip"))
                .and_then(|n| n.rsplit_once('_').map(|(_, tail)| tail.to_owned()))
                .filter(|tail| tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()))
        });
        return Ok(Report::RxNorm(rxnorm::build(
            &root,
            version.as_deref(),
            &cli.rxnorm_sources,
            &cli.out,
        )?));
    }
    if !cli.icd10cm.is_empty() {
        let scratch = tempfile::tempdir().map_err(RunError::Scratch)?;
        let mut roots = Vec::new();
        for (i, path) in cli.icd10cm.iter().enumerate() {
            if path.is_file() {
                let into = scratch.path().join(i.to_string());
                roots.push(archive::unpack_icd10cm(path, &into)?);
            } else {
                roots.push(path.clone());
            }
        }
        let files = ferroterm_classification::icd10cm::locate(&roots)?;
        let classification = ferroterm_classification::icd10cm::read(&files)?;
        return Ok(Report::Classification(classification::build(
            &classification,
            classification::ICD10CM_SYSTEM,
            None,
            &cli.out,
        )?));
    }
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
    /// A classification: a `ClaML` document or the ICD-10-CM release.
    Classification(classification::Report),
    /// An `RxNorm` release.
    RxNorm(rxnorm::Report),
}

/// A failure of the command as a whole.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// No input was given.
    #[error("give `--rf2`, `--loinc`, `--claml`, `--icd10cm`, or `--rxnorm`")]
    NoInput,
    /// `--claml` without `--system`.
    #[error("`--claml` needs `--system`")]
    NoSystem,
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
    /// The `ClaML` document does not read.
    #[error(transparent)]
    Claml(#[from] ferroterm_classification::claml::ClamlError),
    /// The ICD-10-CM release does not read.
    #[error(transparent)]
    Icd10cm(#[from] ferroterm_classification::icd10cm::Icd10cmError),
    /// The classification build failed.
    #[error(transparent)]
    Classification(#[from] classification::Error),
    /// The `RxNorm` build failed.
    #[error(transparent)]
    RxNorm(#[from] rxnorm::Error),
}
