//! The RF2 release zip: the `Snapshot/` tree unpacked to a temporary
//! directory the pipeline reads, and nothing else.
//!
//! Releases are distributed as one zip holding a release folder
//! (`SnomedCT_<edition>_<date>/`) with `Full/`, `Snapshot/`, and `Delta/`
//! trees (the RF2 release file specification,
//! <https://docs.snomed.org/snomed-ct-specifications/release-file-specification>).
//! Only the Snapshot is unpacked; the temporary directory is removed with the
//! build, so no RF2 content persists outside the operator's index.

use std::fs::File;
use std::io;
use std::path::{Component, Path, PathBuf};

/// A failure to unpack a release zip.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The zip cannot be read.
    #[error("cannot read the release zip {path}")]
    Read {
        /// The zip.
        path: PathBuf,
        /// The cause.
        #[source]
        source: zip::result::ZipError,
    },
    /// An entry cannot be written out.
    #[error("cannot unpack `{entry}`")]
    Unpack {
        /// The entry name.
        entry: String,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The zip holds no `Snapshot/` tree (RF2) or no `Loinc.csv` (LOINC).
    #[error("{path} holds no Snapshot/ tree and no Loinc.csv")]
    NoSnapshot {
        /// The zip.
        path: PathBuf,
    },
    /// More than one release folder carries a `Snapshot/` tree.
    #[error("{path} holds several Snapshot/ trees")]
    SeveralSnapshots {
        /// The zip.
        path: PathBuf,
    },
}

/// Unpacks the `Snapshot/` tree of the release zip at `zip_path` under
/// `into`, returning the release root (the directory holding `Snapshot/`).
///
/// Entry names are taken through `enclosed_name`, so a name that escapes the
/// target directory is skipped rather than followed.
///
/// # Errors
///
/// Returns [`ArchiveError`] when the zip does not read, an entry cannot be
/// written, or the zip holds no single `Snapshot/` tree.
pub fn unpack_snapshot(zip_path: &Path, into: &Path) -> Result<PathBuf, ArchiveError> {
    let read = |source| ArchiveError::Read {
        path: zip_path.to_path_buf(),
        source,
    };
    let file = File::open(zip_path).map_err(|source| ArchiveError::Read {
        path: zip_path.to_path_buf(),
        source: zip::result::ZipError::Io(source),
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(read)?;
    let mut root: Option<PathBuf> = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(read)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(prefix) = snapshot_root(&name) else {
            continue;
        };
        if entry.is_dir()
            || !name
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
        {
            continue;
        }
        match &root {
            None => root = Some(prefix),
            Some(known) if *known != prefix => {
                return Err(ArchiveError::SeveralSnapshots {
                    path: zip_path.to_path_buf(),
                });
            }
            Some(_) => {}
        }
        let target = into.join(&name);
        let entry_name = entry.name().to_owned();
        let unpack = |source| ArchiveError::Unpack {
            entry: entry_name.clone(),
            source,
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(unpack)?;
        }
        let mut out = File::create(&target).map_err(unpack)?;
        io::copy(&mut entry, &mut out).map_err(unpack)?;
    }
    let root = root.ok_or_else(|| ArchiveError::NoSnapshot {
        path: zip_path.to_path_buf(),
    })?;
    Ok(into.join(root))
}

/// The path up to (not including) the `Snapshot` component of `name`, when
/// the entry lies under a `Snapshot/` tree.
fn snapshot_root(name: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in name.components() {
        match component {
            Component::Normal(part) if part == "Snapshot" => return Some(root),
            Component::Normal(part) => root.push(part),
            _ => return None,
        }
    }
    None
}

/// The files of a LOINC release the build reads, by file name.
const LOINC_FILES: [&str; 5] = [
    "Loinc.csv",
    "Part.csv",
    "ComponentHierarchyBySystem.csv",
    "AnswerList.csv",
    "LoincAnswerListLink.csv",
];

/// Unpacks the tables of the LOINC release zip at `zip_path` under `into`.
///
/// Returns the directory to read as the release: the term table, the parts
/// and hierarchy, the answer lists and links, and every linguistic variant;
/// the part-link tables (hundreds of megabytes) stay in the zip.
///
/// # Errors
///
/// Returns [`ArchiveError`] when the zip does not read, an entry cannot be
/// written, or the zip holds no `Loinc.csv`.
pub fn unpack_loinc(zip_path: &Path, into: &Path) -> Result<PathBuf, ArchiveError> {
    let read = |source| ArchiveError::Read {
        path: zip_path.to_path_buf(),
        source,
    };
    let file = File::open(zip_path).map_err(|source| ArchiveError::Read {
        path: zip_path.to_path_buf(),
        source: zip::result::ZipError::Io(source),
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(read)?;
    let mut found_terms = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(read)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(file_name) = name.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let wanted = LOINC_FILES
            .iter()
            .any(|f| f.eq_ignore_ascii_case(file_name))
            || file_name.ends_with("LinguisticVariant.csv");
        if entry.is_dir() || !wanted {
            continue;
        }
        found_terms |= file_name.eq_ignore_ascii_case("Loinc.csv");
        let target = into.join(&name);
        let entry_name = entry.name().to_owned();
        let unpack = |source| ArchiveError::Unpack {
            entry: entry_name.clone(),
            source,
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(unpack)?;
        }
        let mut out = File::create(&target).map_err(unpack)?;
        io::copy(&mut entry, &mut out).map_err(unpack)?;
    }
    if !found_terms {
        return Err(ArchiveError::NoSnapshot {
            path: zip_path.to_path_buf(),
        });
    }
    Ok(into.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::snapshot_root;

    #[test]
    fn the_release_root_precedes_the_snapshot_tree() {
        assert_eq!(
            snapshot_root(Path::new(
                "SnomedCT_X/Snapshot/Terminology/sct2_Concept_Snapshot_INT_20240101.txt"
            )),
            Some(PathBuf::from("SnomedCT_X"))
        );
        assert_eq!(
            snapshot_root(Path::new("Snapshot/Terminology/x.txt")),
            Some(PathBuf::new())
        );
        assert_eq!(
            snapshot_root(Path::new("SnomedCT_X/Full/Terminology/x.txt")),
            None
        );
    }
}
