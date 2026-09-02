//! `OperationOutcome` responses
//! (<https://hl7.org/fhir/R4B/operationoutcome.html>).
//!
//! Every failure a client can cause answers with an `OperationOutcome` whose
//! issue carries `severity`, `code` from the issue-type value set, and
//! `diagnostics`; the status is the one the operation layer chose.

use axum::body::Body;
use axum::response::{IntoResponse, Response};
use ferroterm_fhir::codec::Json;
use ferroterm_fhir::r4b::operation_outcome::{OperationOutcome, OperationOutcomeIssue};
use ferroterm_terminology::operations::OperationError;
use http::StatusCode;
use http::header::CONTENT_TYPE;

/// The media type of every FHIR response.
pub const FHIR_JSON: &str = "application/fhir+json; charset=utf-8";

/// A failure to answer, as the wire sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The HTTP status.
    pub status: StatusCode,
    /// The `issue.code`.
    pub code: &'static str,
    /// The `issue.diagnostics`.
    pub diagnostics: String,
}

impl Failure {
    /// A failure with `status`, `code`, and `diagnostics`.
    #[must_use]
    pub fn new(status: StatusCode, code: &'static str, diagnostics: impl Into<String>) -> Self {
        Self {
            status,
            code,
            diagnostics: diagnostics.into(),
        }
    }

    /// The `OperationOutcome` resource.
    #[must_use]
    pub fn outcome(&self) -> OperationOutcome {
        OperationOutcome {
            issue: vec![OperationOutcomeIssue {
                severity: "error".into(),
                code: self.code.into(),
                diagnostics: Some(self.diagnostics.as_str().into()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
}

impl From<OperationError> for Failure {
    fn from(error: OperationError) -> Self {
        Self::new(error.status(), error.issue_code(), error.to_string())
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        match self.outcome().to_json() {
            Ok(object) => fhir_json(self.status, &serde_json::Value::Object(object)),
            // NOTE: encoding a hand-built OperationOutcome cannot fail; if the
            // codec ever refuses, a bare status still tells the client the truth.
            Err(_) => self.status.into_response(),
        }
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

/// The fallback for a path the server does not define.
pub async fn not_found() -> Response {
    Failure::new(
        StatusCode::NOT_FOUND,
        "not-found",
        "no such resource or operation on this server",
    )
    .into_response()
}
