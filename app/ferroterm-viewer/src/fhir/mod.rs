//! The FHIR client: the one module in the viewer that issues HTTP.
//!
//! Every request goes to the origin the bundle was served from, so the viewer
//! is a client of this server's public API and nothing else. No component
//! calls `fetch`, and no second client exists.

pub(crate) mod capability;
pub(crate) mod code_system;
pub(crate) mod concept_map;
pub(crate) mod error;
pub(crate) mod expansion;
pub(crate) mod facts;
pub(crate) mod outcome;
pub(crate) mod searchset;
pub(crate) mod terminology;
pub(crate) mod translate;
pub(crate) mod value_set;
pub(crate) mod version;

use gloo_net::http::Request;
use gloo_net::http::Response;
use http::StatusCode;
use serde::de::DeserializeOwned;

use crate::fhir::capability::CapabilityStatement;
use crate::fhir::code_system::CodeSystemSearch;
use crate::fhir::concept_map::PublishedConceptMap;
use crate::fhir::error::FhirError;
use crate::fhir::expansion::ExpandRequest;
use crate::fhir::expansion::ExpandedValueSet;
use crate::fhir::outcome::OperationOutcome;
use crate::fhir::searchset::SearchFilter;
use crate::fhir::searchset::SearchSet;
use crate::fhir::terminology::TerminologyCapabilities;
use crate::fhir::translate::TranslateAnswer;
use crate::fhir::translate::TranslateRequest;
use crate::fhir::value_set::PublishedValueSet;
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

/// The `ValueSet` resource type, as it appears in a request path.
pub(crate) const VALUE_SET: &str = "ValueSet";

/// The `ConceptMap` resource type, as it appears in a request path.
pub(crate) const CONCEPT_MAP: &str = "ConceptMap";

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

    /// The address of the `CapabilityStatement` of one served FHIR version.
    ///
    /// Every read has a paired builder, so a screen can disclose the exact
    /// request it issued without rebuilding the URL from parts of its own.
    pub(crate) fn metadata_url(&self, version: FhirVersion) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment("metadata")
            .render(&self.root)
    }

    /// The address of the `TerminologyCapabilities` of one served version.
    ///
    /// `mode=terminology` selects the terminology capabilities of the same
    /// `metadata` interaction
    /// (<https://hl7.org/fhir/R4B/terminologycapabilities.html>).
    pub(crate) fn terminology_metadata_url(&self, version: FhirVersion) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment("metadata")
            .query("mode", "terminology")
            .render(&self.root)
    }

    /// The address of the `CodeSystem` resources one root publishes for a
    /// canonical.
    ///
    /// `url` is the search parameter every definitional resource carries
    /// (<https://hl7.org/fhir/R4B/codesystem.html#search>), so this is the
    /// RESTful search interaction rather than an operation. The canonical is
    /// percent-encoded into the query, which matters because a code system URI
    /// can carry its own query string.
    pub(crate) fn code_system_search_url(&self, version: FhirVersion, system: &str) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment("CodeSystem")
            .query("url", system)
            .render(&self.root)
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
        self.get_json(&self.metadata_url(version)).await
    }

    /// Reads the `TerminologyCapabilities` of one served FHIR version.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn terminology_capabilities(
        &self,
        version: FhirVersion,
    ) -> Result<TerminologyCapabilities, FhirError> {
        self.get_json(&self.terminology_metadata_url(version)).await
    }

    /// Reads the `CodeSystem` resources one root publishes for a canonical.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn code_system_search(
        &self,
        version: FhirVersion,
        system: &str,
    ) -> Result<CodeSystemSearch, FhirError> {
        self.get_json(&self.code_system_search_url(version, system))
            .await
    }

    /// The address one `ValueSet/$expand` run reads.
    ///
    /// `$expand` takes its parameters in the query of a `GET`
    /// (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>), and every
    /// one of them is percent-encoded, so an implicit canonical carrying its
    /// own query string stays inside the parameter it belongs to.
    pub(crate) fn expand_url(&self, version: FhirVersion, request: &ExpandRequest) -> String {
        request
            .append(
                RequestUrl::new()
                    .segment(version.segment())
                    .segment("ValueSet")
                    .segment("$expand"),
            )
            .render(&self.root)
    }

    /// Runs `ValueSet/$expand` and reads the expansion it answers.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// selection the server refuses to expand, `too-costly` among them,
    /// arrives as [`FhirError::Refused`] carrying the server's own
    /// `OperationOutcome`.
    pub(crate) async fn expand(
        &self,
        version: FhirVersion,
        request: &ExpandRequest,
    ) -> Result<ExpandedValueSet, FhirError> {
        self.get_json(&self.expand_url(version, request)).await
    }

    /// The address of a RESTful search over one resource type.
    ///
    /// `url` and `version` are the two search parameters every definitional
    /// resource carries (<https://hl7.org/fhir/R4B/valueset.html#search>), and
    /// each is sent only when the reader named it, so an unfiltered search
    /// asks for everything the root holds. Both are percent-encoded, because a
    /// canonical can carry its own query string.
    pub(crate) fn search_url(
        &self,
        version: FhirVersion,
        resource_type: &str,
        filter: &SearchFilter,
    ) -> String {
        let mut url = RequestUrl::new()
            .segment(version.segment())
            .segment(resource_type);
        if !filter.url.is_empty() {
            url = url.query("url", &filter.url);
        }
        if !filter.version.is_empty() {
            url = url.query("version", &filter.version);
        }
        url.render(&self.root)
    }

    /// The address of one stored resource, as the read interaction takes it.
    ///
    /// The id is one percent-encoded path segment
    /// (<https://hl7.org/fhir/R4B/http.html#read>), so an address a reader
    /// typed cannot reach a route the viewer did not mean to ask for.
    pub(crate) fn resource_url(
        &self,
        version: FhirVersion,
        resource_type: &str,
        id: &str,
    ) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment(resource_type)
            .segment(id)
            .render(&self.root)
    }

    /// Searches the `ValueSet` resources this root holds.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn value_set_search(
        &self,
        version: FhirVersion,
        filter: &SearchFilter,
    ) -> Result<SearchSet<PublishedValueSet>, FhirError> {
        self.get_json(&self.search_url(version, VALUE_SET, filter))
            .await
    }

    /// Reads one `ValueSet` by its id.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. An id
    /// this root does not hold arrives as [`FhirError::Refused`] carrying the
    /// server's own `OperationOutcome`.
    pub(crate) async fn value_set_read(
        &self,
        version: FhirVersion,
        id: &str,
    ) -> Result<PublishedValueSet, FhirError> {
        self.get_json(&self.resource_url(version, VALUE_SET, id))
            .await
    }

    /// Searches the `ConceptMap` resources this root holds.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn concept_map_search(
        &self,
        version: FhirVersion,
        filter: &SearchFilter,
    ) -> Result<SearchSet<PublishedConceptMap>, FhirError> {
        self.get_json(&self.search_url(version, CONCEPT_MAP, filter))
            .await
    }

    /// Reads one `ConceptMap` by its id.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. An id
    /// this root does not hold arrives as [`FhirError::Refused`] carrying the
    /// server's own `OperationOutcome`.
    pub(crate) async fn concept_map_read(
        &self,
        version: FhirVersion,
        id: &str,
    ) -> Result<PublishedConceptMap, FhirError> {
        self.get_json(&self.resource_url(version, CONCEPT_MAP, id))
            .await
    }

    /// The address one `ConceptMap/$translate` run reads.
    ///
    /// `$translate` takes its parameters in the query of a `GET`
    /// (<https://hl7.org/fhir/R4B/conceptmap-operation-translate.html>), under
    /// the names the target version's own `OperationDefinition` declares, and
    /// every one of them is percent-encoded.
    pub(crate) fn translate_url(&self, version: FhirVersion, request: &TranslateRequest) -> String {
        request
            .append(
                RequestUrl::new()
                    .segment(version.segment())
                    .segment(CONCEPT_MAP)
                    .segment("$translate"),
                version,
            )
            .render(&self.root)
    }

    /// Runs `ConceptMap/$translate` and reads the `Parameters` it answers.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// refusal arrives as [`FhirError::Refused`] carrying the server's own
    /// `OperationOutcome`.
    pub(crate) async fn translate(
        &self,
        version: FhirVersion,
        request: &TranslateRequest,
    ) -> Result<TranslateAnswer, FhirError> {
        self.get_json(&self.translate_url(version, request)).await
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

/// The `curl` line that reproduces a request the viewer made.
///
/// Every screen shows this beside the URL it read, which is the cheapest
/// demonstration that the page did nothing a reader cannot do themselves.
pub(crate) fn curl_line(url: &str) -> String {
    format!(
        "curl -H {accept} {target}",
        accept = shell_quote(&format!("Accept: {FHIR_JSON}")),
        target = shell_quote(url),
    )
}

/// Quotes one argument for a POSIX shell, so a reader can paste it as it is.
///
/// Single quotes take everything literally and the only character they cannot
/// carry is a single quote itself, which is closed, escaped, and reopened
/// (<https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html>).
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
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
    fn the_metadata_addresses_are_the_ones_a_reader_would_type() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.metadata_url(FhirVersion::R5),
            "https://tx.example.org/r5/metadata"
        );
        assert_eq!(
            client.terminology_metadata_url(FhirVersion::R6),
            "https://tx.example.org/r6/metadata?mode=terminology"
        );
    }

    #[test]
    fn a_code_system_search_encodes_the_canonical_it_asks_about() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.code_system_search_url(
                FhirVersion::R4B,
                "https://terminology.example/x?edition=2031"
            ),
            "https://tx.example.org/r4b/CodeSystem?url=https%3A%2F%2Fterminology.example%2Fx%3Fedition%3D2031",
            "a canonical carrying its own query string cannot truncate the search"
        );
    }

    #[test]
    fn an_expansion_address_carries_an_implicit_canonical_whole() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        let request = ExpandRequest {
            // An implicit value set canonical carries its own query string,
            // and an unencoded one would truncate the request around it.
            url: "http://snomed.info/sct?fhir_vs=isa/404684003".to_owned(),
            count: Some(20),
            offset: Some(40),
            ..ExpandRequest::default()
        };
        assert_eq!(
            client.expand_url(FhirVersion::R4B, &request),
            "https://tx.example.org/r4b/ValueSet/$expand\
             ?url=http%3A%2F%2Fsnomed.info%2Fsct%3Ffhir_vs%3Disa%2F404684003&count=20&offset=40",
            "the operation name survives the path and the canonical survives the query"
        );
    }

    #[test]
    fn a_search_sends_only_the_parameters_the_reader_named() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.search_url(FhirVersion::R4B, VALUE_SET, &SearchFilter::default()),
            "https://tx.example.org/r4b/ValueSet",
            "an unfiltered search asks for everything the root holds"
        );
        assert_eq!(
            client.search_url(
                FhirVersion::R5,
                CONCEPT_MAP,
                &SearchFilter {
                    url: "https://terminology.example/cm?a=b".to_owned(),
                    version: "2031".to_owned(),
                }
            ),
            "https://tx.example.org/r5/ConceptMap\
             ?url=https%3A%2F%2Fterminology.example%2Fcm%3Fa%3Db&version=2031",
            "a canonical carrying its own query string cannot truncate the search"
        );
    }

    #[test]
    fn a_resource_read_escapes_the_id_into_its_own_segment() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.resource_url(FhirVersion::R6, VALUE_SET, "a/b"),
            "https://tx.example.org/r6/ValueSet/a%2Fb",
            "an id a reader typed cannot reach a route the viewer did not mean to ask for"
        );
    }

    #[test]
    fn a_translation_sends_the_names_the_target_version_declares() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        let request = TranslateRequest {
            system: "http://snomed.info/sct".to_owned(),
            code: "404684003".to_owned(),
            ..TranslateRequest::default()
        };
        assert_eq!(
            client.translate_url(FhirVersion::R4B, &request),
            "https://tx.example.org/r4b/ConceptMap/$translate\
             ?system=http%3A%2F%2Fsnomed.info%2Fsct&code=404684003"
        );
        assert_eq!(
            client.translate_url(FhirVersion::R6, &request),
            "https://tx.example.org/r6/ConceptMap/$translate\
             ?sourceSystem=http%3A%2F%2Fsnomed.info%2Fsct&sourceCode=404684003",
            "the R6 ballot renamed `system` and `code`"
        );
    }

    #[test]
    fn the_curl_line_asks_for_the_media_type_the_client_asks_for() {
        assert_eq!(
            curl_line("https://tx.example.org/r4b/metadata?mode=terminology"),
            "curl -H 'Accept: application/fhir+json' 'https://tx.example.org/r4b/metadata?mode=terminology'",
            "the line reproduces the request the browser made"
        );
    }

    #[test]
    fn a_quote_in_a_url_cannot_break_out_of_the_curl_line() {
        assert_eq!(
            shell_quote("a'b"),
            r"'a'\''b'",
            "the quote is closed, escaped, and reopened"
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
