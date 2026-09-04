//! The `tx-resource` parameters of one version, converted to the engine's models.

macro_rules! resources {
    ($fhir:ident) => {
        pub mod resources {
            //! `tx-resource` parameters split off a `Parameters` and converted to
            //! the models the request scope layers
            //! (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).

            use fhir_types::$fhir::parameters::Parameters;
            use fhir_types::$fhir::resource::Resource;
            use fhir_terminology::{conceptmap, fhir_codesystem, valueset};
            use http::StatusCode;

            use crate::outcome::Failure;
            use crate::scope::{Loaded, TX_RESOURCE, UUID};

            /// Splits the `tx-resource` parameters off `parameters` and converts
            /// each carried resource; a `uuid` parameter is dropped.
            ///
            /// # Errors
            ///
            /// A `tx-resource` without a resource, with a resource of another type
            /// than `CodeSystem`, `ValueSet`, or `ConceptMap`, or with one the
            /// engine cannot serve is a 400.
            pub fn split_resources(
                parameters: Parameters,
            ) -> Result<(Parameters, Vec<Loaded>), Failure> {
                let mut own = Vec::with_capacity(parameters.parameter.len());
                let mut resources = Vec::new();
                for parameter in parameters.parameter {
                    if parameter.name.value.as_deref() == Some(UUID) {
                        continue;
                    }
                    if parameter.name.value.as_deref() == Some(TX_RESOURCE) {
                        let resource = parameter.resource.ok_or_else(|| {
                            Failure::new(
                                StatusCode::BAD_REQUEST,
                                "invalid",
                                format!("`{TX_RESOURCE}` must carry a resource"),
                            )
                        })?;
                        resources.push(loaded(resource)?);
                    } else {
                        own.push(parameter);
                    }
                }
                Ok((
                    Parameters {
                        parameter: own,
                        ..parameters
                    },
                    resources,
                ))
            }

            /// The model of a resource given as a JSON object of this version.
            ///
            /// # Errors
            ///
            /// Returns what the codec or the conversion refused, as text.
            pub fn model_of(object: &fhir_types::codec::Object) -> Result<Loaded, String> {
                loaded(resource_of(object)?).map_err(|failure| failure.diagnostics)
            }

            /// `object` read as a resource of this version and written back.
            ///
            /// # Errors
            ///
            /// Returns what the codec refused, as text: a resource stored in
            /// another FHIR version can carry an element this one does not define.
            pub fn round_trip(
                object: &fhir_types::codec::Object,
            ) -> Result<fhir_types::codec::Object, String> {
                fhir_types::codec::Json::to_json(&resource_of(object)?)
                    .map_err(|error| error.to_string())
            }

            /// `object` read as a resource of this version.
            ///
            /// # Errors
            ///
            /// Returns what the codec refused, as text.
            pub fn resource_of(object: &fhir_types::codec::Object) -> Result<Resource, String> {
                let mut path = fhir_types::codec::Path::root("Resource");
                fhir_types::codec::Json::from_json(object, &mut path)
                    .map_err(|error| error.to_string())
            }

            fn loaded(resource: Resource) -> Result<Loaded, Failure> {
                match resource {
                    Resource::CodeSystem(code_system) => {
                        fhir_codesystem::convert::$fhir::convert(&code_system)
                            .map(Loaded::CodeSystem)
                            .map_err(invalid)
                    }
                    Resource::ValueSet(value_set) => valueset::convert::$fhir::convert(&value_set)
                        .map(Loaded::ValueSet)
                        .map_err(invalid),
                    Resource::ConceptMap(concept_map) => {
                        conceptmap::convert::$fhir::convert(&concept_map)
                            .map(Loaded::ConceptMap)
                            .map_err(invalid)
                    }
                    other => Err(Failure::new(
                        StatusCode::BAD_REQUEST,
                        "not-supported",
                        format!(
                            "`{TX_RESOURCE}` carries a {}; only CodeSystem, ValueSet, and ConceptMap resources are accepted",
                            resource_type(&other)
                        ),
                    )),
                }
            }

            fn invalid(error: impl std::fmt::Display) -> Failure {
                Failure::new(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    format!("a `{TX_RESOURCE}` cannot be served: {error}"),
                )
            }

            fn resource_type(resource: &Resource) -> &'static str {
                match resource {
                    Resource::Bundle(_) => "Bundle",
                    Resource::CapabilityStatement(_) => "CapabilityStatement",
                    Resource::CodeSystem(_) => "CodeSystem",
                    Resource::ConceptMap(_) => "ConceptMap",
                    Resource::OperationOutcome(_) => "OperationOutcome",
                    Resource::Parameters(_) => "Parameters",
                    Resource::TerminologyCapabilities(_) => "TerminologyCapabilities",
                    Resource::ValueSet(_) => "ValueSet",
                    Resource::Unknown(_) => "resource of another type",
                }
            }
        }
    };
}

pub(crate) use resources;
