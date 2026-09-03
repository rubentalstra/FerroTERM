//! What an artifact directory holds, before a provider opens it.

use std::path::{Path, PathBuf};

/// The manifest cannot say which system the artifact serves.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    /// The manifest does not read.
    #[error("cannot read {path}")]
    Io {
        /// The manifest path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The manifest is not JSON with a `system`.
    #[error("{path} is not an artifact manifest")]
    Manifest {
        /// The manifest path.
        path: PathBuf,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
    /// The manifest names no `system`.
    #[error("{path} names no system")]
    NoSystem {
        /// The manifest path.
        path: PathBuf,
    },
}

/// What a manifest says about its artifact before a provider opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// The code system URI served.
    pub system: String,
    /// The pipeline that built it (`classification`), when the manifest says.
    pub kind: Option<String>,
}

/// The system and kind of the artifact under `dir`, from its manifest.
///
/// # Errors
///
/// Returns [`ArtifactError`] when the manifest does not read or names no system.
pub fn describe(dir: &Path) -> Result<Description, ArtifactError> {
    let path = dir.join("manifest.json");
    let text = std::fs::read_to_string(&path).map_err(|source| ArtifactError::Io {
        path: path.clone(),
        source,
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| ArtifactError::Manifest {
            path: path.clone(),
            source,
        })?;
    let system = value
        .get("system")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or(ArtifactError::NoSystem { path })?;
    Ok(Description {
        system,
        kind: value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}
