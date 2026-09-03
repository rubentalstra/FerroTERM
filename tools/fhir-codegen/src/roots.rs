//! The terminology root set: what the generator emits from a package.
//!
//! FerroTERM is a terminology server, so the generator emits the resources of
//! the FHIR terminology module (<https://hl7.org/fhir/R4B/terminology-module.html>)
//! plus the infrastructure resources its operations exchange, and the
//! operations defined on `CodeSystem`, `ValueSet`, and `ConceptMap`. The
//! root set is declared here; the transitive closure of the datatypes those
//! roots reference is the emitter's job.

use std::collections::BTreeMap;

use crate::fhir::{OperationDefinition, StructureDefinition};
use crate::package::Package;

/// The resource types the generator emits, by name.
pub const ROOT_RESOURCES: [&str; 8] = [
    "Bundle",
    "CapabilityStatement",
    "CodeSystem",
    "ConceptMap",
    "OperationOutcome",
    "Parameters",
    "TerminologyCapabilities",
    "ValueSet",
];

/// The resource types whose operations are terminology operations.
pub const OPERATION_RESOURCES: [&str; 3] = ["CodeSystem", "ConceptMap", "ValueSet"];

/// A root resource the package does not define.
#[derive(Debug, thiserror::Error)]
#[error("the package defines no StructureDefinition named {name}")]
pub struct MissingRoot {
    /// The missing resource type name.
    pub name: String,
}

/// The root set selected from one package.
#[derive(Debug)]
pub struct RootSet<'a> {
    /// The root resource definitions, keyed by type name.
    pub resources: BTreeMap<&'static str, &'a StructureDefinition>,
    /// The terminology operations, keyed by canonical URL.
    pub operations: BTreeMap<&'a str, &'a OperationDefinition>,
}

impl<'a> RootSet<'a> {
    /// Selects the root resources and terminology operations of `package`.
    ///
    /// An operation is a terminology operation when it applies to at least
    /// one resource type and every type it applies to is one of
    /// [`OPERATION_RESOURCES`].
    ///
    /// # Errors
    ///
    /// Returns [`MissingRoot`] when the package defines no structure for one
    /// of [`ROOT_RESOURCES`].
    pub fn select(package: &'a Package) -> Result<Self, MissingRoot> {
        let mut resources = BTreeMap::new();
        for name in ROOT_RESOURCES {
            let definition =
                package
                    .structure_definition_named(name)
                    .ok_or_else(|| MissingRoot {
                        name: name.to_owned(),
                    })?;
            resources.insert(name, definition);
        }
        let operations = package
            .operation_definitions()
            .iter()
            .filter(|(_, operation)| is_terminology_operation(operation))
            .map(|(url, operation)| (url.as_str(), operation))
            .collect();
        Ok(Self {
            resources,
            operations,
        })
    }

    /// The terminology operation invoked as `$code` on `resource`, if any.
    #[must_use]
    pub fn operation(&self, resource: &str, code: &str) -> Option<&'a OperationDefinition> {
        self.operations.values().copied().find(|operation| {
            operation.code == code && operation.resource.iter().any(|r| r == resource)
        })
    }
}

fn is_terminology_operation(operation: &OperationDefinition) -> bool {
    !operation.resource.is_empty()
        && operation
            .resource
            .iter()
            .all(|resource| OPERATION_RESOURCES.contains(&resource.as_str()))
}
