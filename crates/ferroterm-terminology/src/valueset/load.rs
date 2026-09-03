//! Reading `ValueSet` resources from a directory, beside the `CodeSystem`s.

use std::path::Path;

use ferroterm_fhir::codec::{Json, Path as ElementPath, expect_object};

use super::convert;
use super::model::ValueSetModel;
use crate::fhir_codesystem::load::{FhirVersion, LoadError, scan_json};

/// Loads every `ValueSet` resource in a directory.
///
/// Files whose `resourceType` is not `ValueSet` are skipped; the result is
/// sorted by file name so it is deterministic.
///
/// # Errors
///
/// Returns [`LoadError`] when the directory or a `ValueSet` file fails.
pub fn load_dir(dir: &Path, version: FhirVersion) -> Result<Vec<ValueSetModel>, LoadError> {
    let mut models = Vec::new();
    for (path, value) in scan_json(dir, "ValueSet")? {
        let mut element = ElementPath::root("ValueSet");
        let decode = |source| LoadError::Decode {
            path: path.clone(),
            version,
            resource_type: "ValueSet",
            source,
        };
        let object = expect_object(&value, &element).map_err(decode)?;
        let model = match version {
            FhirVersion::R4 => convert::r4::convert(
                &ferroterm_fhir::r4::value_set::ValueSet::from_json(object, &mut element)
                    .map_err(decode)?,
            ),
            FhirVersion::R4B => convert::r4b::convert(
                &ferroterm_fhir::r4b::value_set::ValueSet::from_json(object, &mut element)
                    .map_err(decode)?,
            ),
            FhirVersion::R5 => convert::r5::convert(
                &ferroterm_fhir::r5::value_set::ValueSet::from_json(object, &mut element)
                    .map_err(decode)?,
            ),
            FhirVersion::R6 => convert::r6::convert(
                &ferroterm_fhir::r6::value_set::ValueSet::from_json(object, &mut element)
                    .map_err(decode)?,
            ),
        };
        let model = model.map_err(|source| LoadError::ValueSet {
            path: path.clone(),
            source,
        })?;
        if model.url.is_empty() {
            return Err(LoadError::ValueSet {
                path: path.clone(),
                source: super::model::ModelError::NoUrl,
            });
        }
        models.push(model);
    }
    Ok(models)
}
