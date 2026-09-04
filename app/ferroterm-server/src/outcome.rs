//! `OperationOutcome` responses
//! (<https://hl7.org/fhir/R4B/operationoutcome.html>). R4, R4B, and R5 declare
//! the same `issue` elements a failure fills, so one render serves every
//! version.
//!
//! Every failure a client can cause answers with an `OperationOutcome` whose
//! issue carries `severity`, `code` from the issue-type value set, and
//! `diagnostics`; the status is the one the operation layer chose.

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use fhir_terminology::operations::OperationError;
use fhir_terminology::operations::value_set_validate_code::TX_ISSUE_TYPE;
use fhir_types::codec::Json;
use fhir_types::r4b::codeable_concept::CodeableConcept;
use fhir_types::r4b::coding::Coding;
use fhir_types::r4b::operation_outcome::{OperationOutcome, OperationOutcomeIssue};
use http::StatusCode;
use http::header::CONTENT_TYPE;

/// The media type of every FHIR response.
pub const FHIR_JSON: &str = crate::wire::FHIR_JSON;

/// A failure to answer, as the wire sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The HTTP status.
    pub status: StatusCode,
    /// The `issue.code`.
    pub code: &'static str,
    /// The `issue.diagnostics` and `issue.details.text`.
    pub diagnostics: String,
    /// The `tx-issue-type` code in `issue.details.coding`, when one applies.
    pub kind: Option<&'static str>,
}

impl Failure {
    /// A failure with `status`, `code`, and `diagnostics`.
    #[must_use]
    pub fn new(status: StatusCode, code: &'static str, diagnostics: impl Into<String>) -> Self {
        Self {
            status,
            code,
            diagnostics: diagnostics.into(),
            kind: None,
        }
    }

    /// This failure with a `tx-issue-type` coding.
    #[must_use]
    pub fn kind(mut self, kind: &'static str) -> Self {
        self.kind = Some(kind);
        self
    }

    /// The `OperationOutcome` resource.
    ///
    /// The issue carries `details.text` and, when known, a `tx-issue-type`
    /// coding, the classification validators read
    /// (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).
    #[must_use]
    pub fn outcome(&self) -> OperationOutcome {
        OperationOutcome {
            // NOTE: the ecosystem's message ids live on the itemised `issues` of a
            // validation; the outcome of a refused request carries none (its test
            // cases expect a bare issue, <https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/>).
            issue: vec![OperationOutcomeIssue {
                severity: "error".into(),
                code: self.code.into(),
                details: Some(CodeableConcept {
                    coding: self.kind.map(tx_issue_coding).into_iter().collect(),
                    text: Some(self.diagnostics.as_str().into()),
                    ..Default::default()
                }),
                diagnostics: Some(self.diagnostics.as_str().into()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
}

fn tx_issue_coding(kind: &str) -> Coding {
    Coding {
        system: Some(TX_ISSUE_TYPE.into()),
        code: Some(kind.into()),
        ..Default::default()
    }
}

impl From<OperationError> for Failure {
    fn from(error: OperationError) -> Self {
        Self::new(error.status(), error.issue_code(), error.to_string()).kind(error.tx_issue_type())
    }
}

impl Failure {
    /// The response in `wire`: the `OperationOutcome` as FHIR JSON or XML (the
    /// R4B shape, which every served version declares alike).
    #[must_use]
    pub fn respond(&self, wire: crate::wire::Wire) -> Response {
        match self.outcome().to_json() {
            Ok(object) => wire.response(self.status, &object, &fhir_types::r4b::schema::SCHEMAS),
            // NOTE: encoding a hand-built OperationOutcome cannot fail; if the
            // codec ever refuses, a bare status still tells the client the truth.
            Err(_) => self.status.into_response(),
        }
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        self.respond(crate::wire::Wire::Json)
    }
}

/// A FHIR JSON response body with `status`.
pub fn fhir_json(status: StatusCode, value: &serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, FHIR_JSON)
        .body(Body::from(value.to_string()))
        .unwrap_or_else(|_| status.into_response())
}

/// The fallback for a path the server does not define, in the format `Accept`
/// asks for.
pub async fn not_found(headers: http::HeaderMap) -> Response {
    let wire = crate::wire::Wire::negotiate(&[], &headers).unwrap_or_default();
    Failure::new(
        StatusCode::NOT_FOUND,
        "not-found",
        "no such resource or operation on this server",
    )
    .respond(wire)
}
