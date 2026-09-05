//! `GET {root}/metadata` of one version: the `CapabilityStatement` and, with `mode=terminology`, the `TerminologyCapabilities`.

macro_rules! metadata {
    ($fhir:ident, $fhir_version:literal, $label:literal, $capabilities:ident) => {
        pub mod metadata {
            //! `GET {root}/metadata` of one version: the capability statements.
            //!
            //! Without `mode` (or `mode=full|normative`) the `CapabilityStatement`; with
            //! `mode=terminology` the `TerminologyCapabilities`
            //! (<https://hl7.org/fhir/R4B/http.html#capabilities>,
            //! <https://hl7.org/fhir/R4B/terminologycapabilities.html>).

            use std::sync::Arc;

            use axum::extract::{Query, State};
            use axum::response::{IntoResponse, Response};
            use fhir_types::codec::Json;
            use fhir_types::$fhir::capability_statement::{
                CapabilityStatement, CapabilityStatementImplementation, CapabilityStatementRest,
                CapabilityStatementRestInteraction, CapabilityStatementRestResource,
                CapabilityStatementRestResourceInteraction,
                CapabilityStatementRestResourceOperation, CapabilityStatementRestResourceSearchParam,
                CapabilityStatementRestSecurity,
                CapabilityStatementSoftware,
            };
            use fhir_types::$fhir::codeable_concept::CodeableConcept;
            use fhir_types::$fhir::coding::Coding;
            use fhir_types::$fhir::extension::{Extension, ExtensionValue};
            use fhir_types::operation::{Operation, ParameterSource, ParameterUse};
            use fhir_types::$fhir::operations::code_system_lookup::CODE_SYSTEM_LOOKUP;
            use fhir_types::$fhir::operations::code_system_subsumes::CODE_SYSTEM_SUBSUMES;
            use fhir_types::$fhir::operations::code_system_validate_code::CODE_SYSTEM_VALIDATE_CODE;
            use fhir_types::$fhir::operations::concept_map_translate::CONCEPT_MAP_TRANSLATE;
            use fhir_types::$fhir::operations::value_set_expand::VALUE_SET_EXPAND;
            use fhir_types::$fhir::operations::value_set_validate_code::VALUE_SET_VALIDATE_CODE;
            use fhir_types::$fhir::terminology_capabilities::{
                TerminologyCapabilities, TerminologyCapabilitiesImplementation, TerminologyCapabilitiesSoftware,
            };
            use fhir_terminology::capabilities::Summary;
            use http::StatusCode;

            use crate::outcome::Failure;
            use crate::config::{SECURITY_SERVICES, SECURITY_SERVICE_SYSTEM};
            use crate::state::AppState;

            /// The FHIR version this surface serves.
            pub const FHIR_VERSION: &str = $fhir_version;
            /// `rest.security.service`: the authentication in front of the server, which
            /// the terminology ecosystem requires a server to populate
            /// (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>, Metadata).
            ///
            /// The deployment declares it (`FERROTERM_SECURITY_SERVICE`); the server itself
            /// authenticates nobody, and the binding is extensible, so a deployment that
            /// declares none says so in text.
            fn security(state: &AppState) -> CapabilityStatementRestSecurity {
                let declared = state.security_services();
                let service = if declared.is_empty() {
                    CodeableConcept {
                        text: Some(
                            "The server requires no authentication of its own; a deployment puts its own in front of it."
                                .into(),
                        ),
                        ..Default::default()
                    }
                } else {
                    CodeableConcept {
                        coding: declared
                            .iter()
                            .map(|code| Coding {
                                system: Some(SECURITY_SERVICE_SYSTEM.into()),
                                code: Some(code.as_str().into()),
                                display: SECURITY_SERVICES
                                    .iter()
                                    .find(|(known, _)| known == code)
                                    .map(|(_, display)| (*display).into()),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }
                };
                CapabilityStatementRestSecurity {
                    service: vec![service],
                    ..Default::default()
                }
            }

            /// The canonical of this server's capability statement for the version (our own).
            pub const CAPABILITY_URL: &str =
                concat!("https://ferroterm.eu/fhir/CapabilityStatement/ferroterm-", stringify!($fhir));
            /// The terminology server capability statement this one instantiates.
            pub const TERMINOLOGY_SERVER: &str = "http://hl7.org/fhir/CapabilityStatement/terminology-server";
            /// The application-feature extension (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/>).
            const FEATURE: &str = "http://hl7.org/fhir/uv/application-feature/StructureDefinition/feature";
            /// The terminology ecosystem requirements the overlay rests on.
            const ECOSYSTEM_REQUIREMENTS: &str = "https://hl7.org/fhir/uv/tx-ecosystem/requirements.html";
            /// The release date of this version from the changelog (`build.rs`), `None`
            /// for an unreleased version.
            const RELEASE_DATE: Option<&str> = option_env!("FERROTERM_RELEASE_DATE");

            /// The terminology ecosystem overlay of `operation`, for the operation's
            /// `documentation`: what the server accepts and answers beyond the version's
            /// own definition, by source; `None` when the overlay adds nothing.
            fn overlay_documentation(operation: &Operation) -> Option<String> {
                let names = |usage: ParameterUse, source: ParameterSource| -> Vec<String> {
                    operation
                        .parameters_of(usage)
                        .filter(|p| p.source == source)
                        .map(|p| format!("`{}`", p.name))
                        .collect()
                };
                let mut sentences = Vec::new();
                for (source, phrase) in [
                    (
                        ParameterSource::PreAdopted,
                        "pre-adopted from the FHIR R6 ballot for the terminology ecosystem",
                    ),
                    (ParameterSource::Ecosystem, "defined by the terminology ecosystem"),
                ] {
                    let inputs = names(ParameterUse::In, source);
                    let outputs = names(ParameterUse::Out, source);
                    let mut clauses = Vec::new();
                    if !inputs.is_empty() {
                        clauses.push(format!("accepts {}", inputs.join(", ")));
                    }
                    if !outputs.is_empty() {
                        clauses.push(format!("answers {}", outputs.join(", ")));
                    }
                    if !clauses.is_empty() {
                        sentences.push(format!(
                            "Beyond the {} definition, the server {} ({phrase}).",
                            $label,
                            clauses.join(" and ")
                        ));
                    }
                }
                (!sentences.is_empty())
                    .then(|| format!("{} See <{ECOSYSTEM_REQUIREMENTS}>.", sentences.join(" ")))
            }

            /// One application feature: its definition and value.
            fn feature(definition: &str, value: ExtensionValue) -> Extension {
                Extension {
                    url: FEATURE.to_owned(),
                    extension: vec![
                        Extension {
                            url: String::from("definition"),
                            value: Some(ExtensionValue::Canonical(definition.into())),
                            ..Default::default()
                        },
                        Extension {
                            url: String::from("value"),
                            value: Some(value),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }
            }

            /// The note a ballot-tracking version carries: which ballot it serves, and
            /// that ballot-grounded behaviour is re-verified when the version publishes.
            fn ballot_note() -> Option<String> {
                FHIR_VERSION.contains("ballot").then(|| {
                    format!(
                        "Serves the FHIR {} (version {FHIR_VERSION}). Every behaviour grounded only in the ballot is re-verified against the published specification when it appears, and may change with it.",
                        $label
                    )
                })
            }

            /// The current time as a FHIR `dateTime` (RFC 3339, UTC).
            fn now() -> String {
                jiff::Timestamp::now().to_string()
            }

            /// Handles `GET /metadata` (`mode` `full`, `normative`, or `terminology`).
            pub async fn metadata(
                State(state): State<Arc<AppState>>,
                headers: http::HeaderMap,
                Query(query): Query<Vec<(String, String)>>,
            ) -> Response {
                let wire = match crate::wire::Wire::negotiate(&query, &headers) {
                    Ok(wire) => wire,
                    Err(failure) => return failure.into_response(),
                };
                let mode = query
                    .iter()
                    .find(|(name, _)| name == "mode")
                    .map_or("full", |(_, value)| value.as_str());
                let encoded = match mode {
                    "terminology" => terminology_capabilities(&state).to_json(),
                    "full" | "normative" => capability_statement(&state).to_json(),
                    other => {
                        return Failure::new(
                            StatusCode::BAD_REQUEST,
                            "value",
                            format!("`mode={other}` is not full, normative, or terminology"),
                        )
                        .into_response();
                    }
                };
                match encoded {
                    Ok(object) => wire.response(StatusCode::OK, &object, &fhir_types::$fhir::schema::SCHEMAS),
                    Err(e) => Failure::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "exception",
                        format!("cannot encode the capability statement: {e}"),
                    )
                    .into_response(),
                }
            }

            /// One operation of the capability statement.
            ///
            /// R4B's own example instances name operations without the `$`
            /// (<https://hl7.org/fhir/R4B/capabilitystatement-example.json.html>);
            /// the bare name is used.
            fn operation(name: &str, definition: &str) -> CapabilityStatementRestResourceOperation {
                CapabilityStatementRestResourceOperation {
                    name: name.into(),
                    definition: definition.into(),
                    documentation: None,
                    ..Default::default()
                }
            }

            /// One declared operation, with what the ecosystem overlay adds to it.
            fn declared(descriptor: &Operation) -> CapabilityStatementRestResourceOperation {
                CapabilityStatementRestResourceOperation {
                    documentation: overlay_documentation(descriptor).map(Into::into),
                    ..operation(descriptor.code, descriptor.url)
                }
            }

            /// The interactions this deployment answers on every stored resource type.
            ///
            /// A deployment that names no resource database refuses every write, so the
            /// write interactions are declared only where they are answered
            /// (<https://hl7.org/fhir/R4B/http.html>).
            fn interactions(state: &AppState) -> Vec<CapabilityStatementRestResourceInteraction> {
                let interaction = |code: &str| CapabilityStatementRestResourceInteraction {
                    code: code.into(),
                    ..Default::default()
                };
                let mut out = vec![interaction("read"), interaction("search-type")];
                if state.persists() {
                    out.push(interaction("vread"));
                    out.push(interaction("create"));
                    out.push(interaction("update"));
                    out.push(interaction("delete"));
                }
                out
            }

            /// The search parameters every stored resource type answers.
            fn search_params() -> Vec<CapabilityStatementRestResourceSearchParam> {
                let search_param = |name: &str, code: &str| {
                    CapabilityStatementRestResourceSearchParam {
                        name: name.into(),
                        r#type: code.into(),
                        ..Default::default()
                    }
                };
                vec![search_param("url", "uri"), search_param("version", "token")]
            }

            /// The three resource types this server serves, with their
            /// interactions, search parameters, and operations.
            fn rest_resources(state: &AppState) -> Vec<CapabilityStatementRestResource> {
                vec![
                    CapabilityStatementRestResource {
                        r#type: "CodeSystem".into(),
                        interaction: interactions(state),
                        search_param: search_params(),
                        operation: vec![
                            declared(&CODE_SYSTEM_LOOKUP),
                            declared(&CODE_SYSTEM_VALIDATE_CODE),
                            declared(&CODE_SYSTEM_SUBSUMES),
                        ],
                        ..Default::default()
                    },
                    CapabilityStatementRestResource {
                        r#type: "ValueSet".into(),
                        interaction: interactions(state),
                        search_param: search_params(),
                        operation: vec![
                            declared(&VALUE_SET_EXPAND),
                            declared(&VALUE_SET_VALIDATE_CODE),
                        ],
                        ..Default::default()
                    },
                    CapabilityStatementRestResource {
                        r#type: "ConceptMap".into(),
                        interaction: interactions(state),
                        search_param: search_params(),
                        operation: vec![declared(&CONCEPT_MAP_TRANSLATE)],
                        ..Default::default()
                    },
                ]
            }

            /// The system-level operations this version answers.
            ///
            /// `$closure` is here only where the version's package declares it: the
            /// R6 ballot ships no `ConceptMap-closure` definition.
            fn system_operations() -> Vec<CapabilityStatementRestResourceOperation> {
                let mut out = vec![
                    operation("versions", super::system::VERSIONS_URL),
                    operation("cache-control", super::system::CACHE_CONTROL_URL),
                ];
                if let Some(url) = super::CLOSURE_URL {
                    out.push(operation("closure", url));
                }
                out
            }

            /// The `CapabilityStatement` (`kind = instance`) of this server.
            #[must_use]
            pub fn capability_statement(state: &AppState) -> CapabilityStatement {

                // NOTE: the ecosystem runner reads these features to know what a server
                // accepts (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
                let features = vec![
                    feature(
                        "http://hl7.org/fhir/uv/tx-tests/FeatureDefinition/test-version",
                        ExtensionValue::Code(state.software_version().into()),
                    ),
                    feature(
                        "http://hl7.org/fhir/uv/tx-ecosystem/FeatureDefinition/CodeSystemAsParameter",
                        ExtensionValue::Boolean(true.into()),
                    ),
                ];
                CapabilityStatement {
                    extension: features,
                    url: Some(CAPABILITY_URL.into()),
                    version: Some(state.software_version().into()),
                    name: Some("FerroTERM".into()),
                    title: Some(concat!("FerroTERM terminology server (", $label, ")").into()),
                    status: "active".into(),
                    date: now().as_str().into(),
                    kind: "instance".into(),
                    instantiates: vec![TERMINOLOGY_SERVER.into()],
                    fhir_version: FHIR_VERSION.into(),
                    format: vec![
                        "application/fhir+json".into(),
                        "json".into(),
                        "application/fhir+xml".into(),
                        "xml".into(),
                    ],
                    software: Some(CapabilityStatementSoftware {
                        name: "FerroTERM".into(),
                        version: Some(state.software_version().into()),
                        release_date: RELEASE_DATE.map(Into::into),
                        ..Default::default()
                    }),
                    description: ballot_note().map(Into::into),
                    implementation: Some(CapabilityStatementImplementation {
                        description: "FerroTERM terminology server".into(),
                        ..Default::default()
                    }),
                    rest: vec![CapabilityStatementRest {
                        mode: "server".into(),
                        security: Some(security(state)),
                        // NOTE: `batch` is a system interaction, the one this server answers at
                        // its root (<https://hl7.org/fhir/R4B/http.html#transaction>).
                        interaction: vec![CapabilityStatementRestInteraction {
                            code: "batch".into(),
                            ..Default::default()
                        }],
                        resource: rest_resources(state),
                        operation: system_operations(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }
            }

            /// The `TerminologyCapabilities` of this server, from the loaded providers.
            #[must_use]
            pub fn terminology_capabilities(state: &AppState) -> TerminologyCapabilities {
                let layer = state.layer();
                let mut capabilities = Summary::of(layer.registry()).$capabilities(&now());
                capabilities.version = Some(state.software_version().into());
                capabilities.name = Some("FerroTERM".into());
                capabilities.title = Some(concat!("FerroTERM terminology capabilities (", $label, ")").into());
                capabilities.software = Some(TerminologyCapabilitiesSoftware {
                    name: "FerroTERM".into(),
                    version: Some(state.software_version().into()),
                    ..Default::default()
                });
                capabilities.description = ballot_note().map(Into::into);
                capabilities.implementation = Some(TerminologyCapabilitiesImplementation {
                    description: "FerroTERM terminology server".into(),
                    ..Default::default()
                });
                capabilities
            }
        }
    };
}

pub(crate) use metadata;
