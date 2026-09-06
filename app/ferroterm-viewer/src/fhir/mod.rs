//! The FHIR client: the one module in the viewer that issues HTTP.
//!
//! Every request goes to the origin the bundle was served from, so the viewer
//! is a client of this server's public API and nothing else. No component
//! calls `fetch`, and no second client exists.

pub(crate) mod capability;
pub(crate) mod error;
pub(crate) mod outcome;
pub(crate) mod version;

use gloo_net::http::Request;
use gloo_net::http::Response;
use http::StatusCode;
use serde::de::DeserializeOwned;

use crate::fhir::capability::CapabilityStatement;
use crate::fhir::error::FhirError;
use crate::fhir::outcome::OperationOutcome;
use crate::fhir::version::FhirVersion;
use crate::url::RequestUrl;

/// The media type a FHIR JSON request asks for.
///
/// The RESTful API defines `application/fhir+json` as the JSON representation
/// (<https://hl7.org/fhir/R4B/http.html#mime-type>).
const FHIR_JSON: &str = "application/fhir+json";

/// How much of an unparseable failure body is kept as evidence.
const BODY_EXCERPT_BYTES: usize = 2_000;

/// The path the bundle is served under, which the server root sits above.
const UI_PREFIX: &str = "/ui";

/// A client for the FerroTERM server that served this bundle.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct FhirClient {
    root: String,
}

impl FhirClient {
    /// Builds a client from the address of the page the bundle came from.
    ///
    /// The base is never compiled in: one bundle serves every deployment, and
    /// a build-time address would make the artifact deployment-specific.
    pub(crate) fn from_document() -> Self {
        let location = web_sys::window().map(|window| window.location());
        let origin = location
            .as_ref()
            .and_then(|location| location.origin().ok())
            .unwrap_or_default();
        let pathname = location
            .as_ref()
            .and_then(|location| location.pathname().ok())
            .unwrap_or_default();
        Self {
            root: server_root(&origin, &pathname),
        }
    }

    /// The FHIR base of one served version, as a reader would type it.
    pub(crate) fn version_base(&self, version: FhirVersion) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .render(&self.root)
    }

    /// Probes `GET /health` and returns the status the server answered.
    ///
    /// `/health` sits outside the FHIR base, so no content negotiation applies
    /// and the answer carries no body to read.
    ///
    /// # Errors
    ///
    /// Returns [`FhirError::Transport`] when the browser could not reach the
    /// server, and [`FhirError::Status`] when it answered a failure.
    pub(crate) async fn health(&self) -> Result<StatusCode, FhirError> {
        let url = RequestUrl::new().segment("health").render(&self.root);
        let response = send(Request::get(&url), &url).await?;
        let status = status_of(&response, &url)?;
        if status.is_success() {
            Ok(status)
        } else {
            Err(failure(&response, status, &url).await)
        }
    }

    /// Reads the `CapabilityStatement` of one served FHIR version.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn capability_statement(
        &self,
        version: FhirVersion,
    ) -> Result<CapabilityStatement, FhirError> {
        let url = RequestUrl::new()
            .segment(version.segment())
            .segment("metadata")
            .render(&self.root);
        self.get_json(&url).await
    }

    /// Sends a FHIR JSON `GET` and decodes the resource it answers.
    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, FhirError> {
        let response = send(Request::get(url).header("Accept", FHIR_JSON), url).await?;
        let status = status_of(&response, url)?;
        if !status.is_success() {
            return Err(failure(&response, status, url).await);
        }
        let body = response.text().await.map_err(|error| FhirError::Decode {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        serde_json::from_str(&body).map_err(|error| FhirError::Decode {
            url: url.to_owned(),
            message: error.to_string(),
        })
    }
}

/// Sends a built request, turning a browser-level failure into an error.
async fn send(request: gloo_net::http::RequestBuilder, url: &str) -> Result<Response, FhirError> {
    request.send().await.map_err(|error| FhirError::Transport {
        url: url.to_owned(),
        message: error.to_string(),
    })
}

/// Reads the answered status as a [`StatusCode`].
///
/// The Fetch standard allows a status of 0 for an opaque answer
/// (<https://fetch.spec.whatwg.org/#concept-response-status>), which is not an
/// HTTP status, so the conversion is fallible and says so.
fn status_of(response: &Response, url: &str) -> Result<StatusCode, FhirError> {
    let code = response.status();
    StatusCode::from_u16(code).map_err(|_invalid| FhirError::Transport {
        url: url.to_owned(),
        message: format!("the browser reported a response status of {code}"),
    })
}

/// Turns a non-success answer into the refusal the reader is shown.
async fn failure(response: &Response, status: StatusCode, url: &str) -> FhirError {
    let Ok(body) = response.text().await else {
        return FhirError::Status {
            url: url.to_owned(),
            status,
            body: String::new(),
        };
    };
    match serde_json::from_str::<OperationOutcome>(&body) {
        // A body that parses but carries no issue says nothing, so the body
        // itself stays the evidence.
        Ok(outcome) if !outcome.issue.is_empty() => FhirError::Refused {
            url: url.to_owned(),
            status,
            outcome,
        },
        Ok(_) | Err(_) => FhirError::Status {
            url: url.to_owned(),
            status,
            body: excerpt(&body),
        },
    }
}

/// Keeps the first part of a body, cut on a character boundary.
fn excerpt(body: &str) -> String {
    body.char_indices()
        .take_while(|(index, _)| *index < BODY_EXCERPT_BYTES)
        .map(|(_, character)| character)
        .collect()
}

/// Derives the server root from the address of the page the bundle came from.
///
/// The bundle is served under `/ui`, so everything before that prefix is where
/// the server is mounted, which keeps the viewer working behind a proxy that
/// mounts it below the origin. Trunk is told the same prefix with `public_url`
/// (<https://github.com/trunk-rs/trunk/blob/main/guide/src/configuration/index.md>).
fn server_root(origin: &str, pathname: &str) -> String {
    let mount = pathname
        .find(UI_PREFIX)
        .and_then(|index| pathname.get(..index))
        .unwrap_or_default();
    format!("{origin}{mount}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_at_the_origin_root_finds_the_server_at_the_origin() {
        assert_eq!(
            server_root("https://tx.example.org", "/ui/settings"),
            "https://tx.example.org"
        );
        assert_eq!(
            server_root("https://tx.example.org", "/ui/"),
            "https://tx.example.org"
        );
    }

    #[test]
    fn a_bundle_mounted_below_the_origin_keeps_the_prefix() {
        assert_eq!(
            server_root("https://hospital.example", "/terminology/ui/browse"),
            "https://hospital.example/terminology",
            "a proxy that mounts the server below the origin still works"
        );
    }

    #[test]
    fn a_page_outside_the_bundle_prefix_falls_back_to_the_origin() {
        assert_eq!(
            server_root("https://tx.example.org", "/"),
            "https://tx.example.org",
            "the root path names no mount point"
        );
    }

    #[test]
    fn an_absent_window_yields_relative_urls_that_stay_same_origin() {
        assert_eq!(
            server_root("", ""),
            "",
            "an empty root renders `/health`, which the browser resolves itself"
        );
    }

    #[test]
    fn a_version_base_is_the_root_plus_the_version_segment() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.version_base(FhirVersion::R4B),
            "https://tx.example.org/r4b"
        );
    }

    #[test]
    fn a_long_failure_body_is_cut_on_a_character_boundary() {
        let body = "é".repeat(BODY_EXCERPT_BYTES);
        let kept = excerpt(&body);
        assert!(
            kept.len() <= BODY_EXCERPT_BYTES + 1,
            "the excerpt stops at the bound, not past it: {} bytes",
            kept.len()
        );
        assert!(
            body.starts_with(&kept),
            "the excerpt is a prefix of the body the server sent"
        );
    }
}
