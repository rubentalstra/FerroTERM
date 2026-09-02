//! `GET /r4b/metadata`: the capability statements.
//!
//! Without `mode` (or `mode=full|normative`) the `CapabilityStatement`; with
//! `mode=terminology` the `TerminologyCapabilities`
//! (<https://hl7.org/fhir/R4B/http.html#capabilities>,
//! <https://hl7.org/fhir/R4B/terminologycapabilities.html>).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use ferroterm_fhir::codec::Json;
use ferroterm_fhir::r4b::capability_statement::{
    CapabilityStatement, CapabilityStatementImplementation, CapabilityStatementRest,
    CapabilityStatementRestResource, CapabilityStatementRestResourceInteraction,
    CapabilityStatementRestResourceOperation, CapabilityStatementSoftware,
};
use ferroterm_fhir::r4b::extension::{Extension, ExtensionValue};
use ferroterm_fhir::r4b::operations::code_system_lookup::CODE_SYSTEM_LOOKUP;
use ferroterm_fhir::r4b::operations::code_system_subsumes::CODE_SYSTEM_SUBSUMES;
use ferroterm_fhir::r4b::operations::code_system_validate_code::CODE_SYSTEM_VALIDATE_CODE;
use ferroterm_fhir::r4b::operations::concept_map_translate::CONCEPT_MAP_TRANSLATE;
use ferroterm_fhir::r4b::operations::value_set_expand::VALUE_SET_EXPAND;
use ferroterm_fhir::r4b::operations::value_set_validate_code::VALUE_SET_VALIDATE_CODE;
use ferroterm_fhir::r4b::terminology_capabilities::{
    TerminologyCapabilities, TerminologyCapabilitiesImplementation, TerminologyCapabilitiesSoftware,
};
use ferroterm_terminology::capabilities::Summary;
use http::StatusCode;

use crate::outcome::{Failure, fhir_json};
use crate::state::AppState;

/// The FHIR version this surface serves.
pub const FHIR_VERSION: &str = "4.3.0";
/// The canonical of this server's R4B capability statement (our own).
pub const CAPABILITY_URL: &str = "https://ferroterm.eu/fhir/CapabilityStatement/ferroterm-r4b";
/// The terminology server capability statement this one instantiates.
pub const TERMINOLOGY_SERVER: &str = "http://hl7.org/fhir/CapabilityStatement/terminology-server";
/// The application-feature extension (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/>).
const FEATURE: &str = "http://hl7.org/fhir/uv/application-feature/StructureDefinition/feature";

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

/// The current time as a FHIR `dateTime` (RFC 3339, UTC).
fn now() -> String {
    jiff::Timestamp::now().to_string()
}

/// Handles `GET /metadata` (`mode` `full`, `normative`, or `terminology`).
pub async fn metadata(
    State(state): State<Arc<AppState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
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
        Ok(object) => fhir_json(StatusCode::OK, &serde_json::Value::Object(object)),
        Err(e) => Failure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "exception",
            format!("cannot encode the capability statement: {e}"),
        )
        .into_response(),
    }
}

/// The `CapabilityStatement` (`kind = instance`) of this server.
#[must_use]
pub fn capability_statement(state: &AppState) -> CapabilityStatement {
    let operation = |name: &str, definition: &str| CapabilityStatementRestResourceOperation {
        // NOTE: R4B's own example instances name operations without the `$`
        // (<https://hl7.org/fhir/R4B/capabilitystatement-example.json.html>);
        // the element text mentions the URL form. The bare name is used.
        name: name.into(),
        definition: definition.into(),
        documentation: None,
        ..Default::default()
    };
    let interaction = |code: &str| CapabilityStatementRestResourceInteraction {
        code: code.into(),
        ..Default::default()
    };
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
        title: Some("FerroTERM terminology server (R4B)".into()),
        status: "active".into(),
        date: now().as_str().into(),
        kind: "instance".into(),
        instantiates: vec![TERMINOLOGY_SERVER.into()],
        fhir_version: FHIR_VERSION.into(),
        format: vec!["application/fhir+json".into(), "json".into()],
        software: Some(CapabilityStatementSoftware {
            name: "FerroTERM".into(),
            version: Some(state.software_version().into()),
            ..Default::default()
        }),
        implementation: Some(CapabilityStatementImplementation {
            description: "FerroTERM terminology server".into(),
            ..Default::default()
        }),
        rest: vec![CapabilityStatementRest {
            mode: "server".into(),
            resource: vec![
                CapabilityStatementRestResource {
                    r#type: "CodeSystem".into(),
                    operation: vec![
                        operation(CODE_SYSTEM_LOOKUP.code, CODE_SYSTEM_LOOKUP.url),
                        operation(
                            CODE_SYSTEM_VALIDATE_CODE.code,
                            CODE_SYSTEM_VALIDATE_CODE.url,
                        ),
                        operation(CODE_SYSTEM_SUBSUMES.code, CODE_SYSTEM_SUBSUMES.url),
                    ],
                    ..Default::default()
                },
                CapabilityStatementRestResource {
                    r#type: "ValueSet".into(),
                    interaction: vec![interaction("read"), interaction("search-type")],
                    operation: vec![
                        operation(VALUE_SET_EXPAND.code, VALUE_SET_EXPAND.url),
                        operation(VALUE_SET_VALIDATE_CODE.code, VALUE_SET_VALIDATE_CODE.url),
                    ],
                    ..Default::default()
                },
                CapabilityStatementRestResource {
                    r#type: "ConceptMap".into(),
                    operation: vec![operation(
                        CONCEPT_MAP_TRANSLATE.code,
                        CONCEPT_MAP_TRANSLATE.url,
                    )],
                    ..Default::default()
                },
            ],
            operation: vec![
                operation("versions", crate::r4b::system::VERSIONS_URL),
                operation("cache-control", crate::r4b::system::CACHE_CONTROL_URL),
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The `TerminologyCapabilities` of this server, from the loaded providers.
#[must_use]
pub fn terminology_capabilities(state: &AppState) -> TerminologyCapabilities {
    let mut capabilities = Summary::of(state.registry()).to_r4b(&now());
    capabilities.version = Some(state.software_version().into());
    capabilities.name = Some("FerroTERM".into());
    capabilities.title = Some("FerroTERM terminology capabilities (R4B)".into());
    capabilities.software = Some(TerminologyCapabilitiesSoftware {
        name: "FerroTERM".into(),
        version: Some(state.software_version().into()),
        ..Default::default()
    });
    capabilities.implementation = Some(TerminologyCapabilitiesImplementation {
        description: "FerroTERM terminology server".into(),
        ..Default::default()
    });
    capabilities
}
