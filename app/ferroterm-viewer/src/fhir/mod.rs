//! The FHIR client: the one module in the viewer that issues HTTP.
//!
//! Every request goes to the origin the bundle was served from, so the viewer
//! is a client of this server's public API and nothing else. No component
//! calls `fetch`, and no second client exists.

pub(crate) mod capability;
pub(crate) mod code_system;
pub(crate) mod concept;
pub(crate) mod concept_map;
pub(crate) mod error;
pub(crate) mod expansion;
pub(crate) mod facts;
pub(crate) mod named;
pub(crate) mod outcome;
pub(crate) mod searchset;
pub(crate) mod smart;
pub(crate) mod terminology;
pub(crate) mod translate;
pub(crate) mod validation;
pub(crate) mod value_set;
pub(crate) mod version;
#[expect(
    dead_code,
    reason = "the write seam's callers are the editor screens of #631"
)]
pub(crate) mod write;

use gloo_net::http::Request;
use gloo_net::http::Response;
use http::StatusCode;
use serde::de::DeserializeOwned;

use crate::fhir::capability::CapabilityStatement;
use crate::fhir::code_system::CodeSystemSearch;
use crate::fhir::concept::ConceptQuery;
use crate::fhir::concept::LookupAnswer;
use crate::fhir::concept::LookupRequest;
use crate::fhir::concept_map::PublishedConceptMap;
use crate::fhir::error::FhirError;
use crate::fhir::expansion::ExpandRequest;
use crate::fhir::expansion::ExpandedValueSet;
use crate::fhir::named::NamedSearch;
use crate::fhir::outcome::OperationOutcome;
use crate::fhir::searchset::SearchFilter;
use crate::fhir::searchset::SearchSet;
use crate::fhir::smart::SignIn;
use crate::fhir::smart::SmartConfiguration;
use crate::fhir::smart::TokenAnswer;
use crate::fhir::terminology::TerminologyCapabilities;
use crate::fhir::translate::TranslateAnswer;
use crate::fhir::translate::TranslateRequest;
use crate::fhir::validation::ParametersAnswer;
use crate::fhir::validation::SubsumesRequest;
use crate::fhir::validation::ValidateRequest;
use crate::fhir::value_set::PublishedValueSet;
use crate::fhir::version::FhirVersion;
use crate::fhir::write::History;
use crate::fhir::write::Written;
use crate::fhir::write::if_match;
use crate::routes::CALLBACK_PATH;
use crate::url::RequestUrl;

/// The media type a FHIR JSON request asks for.
///
/// The RESTful API defines `application/fhir+json` as the JSON representation
/// (<https://hl7.org/fhir/R4B/http.html#mime-type>).
const FHIR_JSON: &str = "application/fhir+json";

/// The media type the SMART discovery document is served as.
///
/// The specification fixes it, whatever the request asks for
/// (<https://hl7.org/fhir/smart-app-launch/conformance.html>).
const SMART_JSON: &str = "application/json";

/// The media type an OAuth token or revocation request sends (RFC 6749 §4.1.3).
const FORM_ENCODED: &str = "application/x-www-form-urlencoded";

/// How much of an unparseable failure body is kept as evidence.
const BODY_EXCERPT_BYTES: usize = 2_000;

/// The path the bundle is served under, which the server root sits above.
const UI_PREFIX: &str = "/ui";

/// The `ValueSet` resource type, as it appears in a request path.
pub(crate) const VALUE_SET: &str = "ValueSet";

/// The `CodeSystem` resource type, as it appears in a request path.
pub(crate) const CODE_SYSTEM: &str = "CodeSystem";

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

    /// The address the name of every published resource of one type is read
    /// from.
    ///
    /// `_elements` narrows the answer to the four fields a picker needs. A
    /// published resource may carry its whole content inline, so the whole
    /// search is a quarter of a megabyte where this is four kilobytes
    /// (<https://hl7.org/fhir/R5/search.html#elements>).
    pub(crate) fn published_names_url(&self, version: FhirVersion, resource_type: &str) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment(resource_type)
            .query("_elements", "url,name,title,version")
            .render(&self.root)
    }

    /// Reads the name every published resource of one type carries.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn published_names(
        &self,
        version: FhirVersion,
        resource_type: &str,
    ) -> Result<NamedSearch, FhirError> {
        self.get_json(&self.published_names_url(version, resource_type))
            .await
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

    /// The address one `CodeSystem/$lookup` reads.
    ///
    /// `$lookup` takes its parameters in the query of a `GET`
    /// (<https://hl7.org/fhir/R4B/codesystem-operation-lookup.html>), and each
    /// one is percent-encoded, so a code carrying a separator stays inside the
    /// parameter it belongs to.
    pub(crate) fn lookup_url(&self, version: FhirVersion, request: &LookupRequest) -> String {
        request
            .append(
                RequestUrl::new()
                    .segment(version.segment())
                    .segment("CodeSystem")
                    .segment("$lookup"),
            )
            .render(&self.root)
    }

    /// Reads one concept with `CodeSystem/$lookup`.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A code
    /// the system does not hold arrives as [`FhirError::Refused`] carrying the
    /// server's own `OperationOutcome`.
    pub(crate) async fn lookup(
        &self,
        version: FhirVersion,
        request: &LookupRequest,
    ) -> Result<LookupAnswer, FhirError> {
        self.get_json(&self.lookup_url(version, request)).await
    }

    /// The address a `ValueSet/$expand` sent as a `POST` is addressed to.
    ///
    /// An operation invoked by `POST` carries every parameter in its
    /// `Parameters` body (<https://hl7.org/fhir/R4B/operations.html#request>),
    /// so the address is the operation itself and nothing else.
    pub(crate) fn expand_post_url(&self, version: FhirVersion) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment("ValueSet")
            .segment("$expand")
            .render(&self.root)
    }

    /// Expands a value set the browser sends inline, and reads the answer.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn expand_inline(
        &self,
        version: FhirVersion,
        query: &ConceptQuery,
    ) -> Result<ExpandedValueSet, FhirError> {
        self.post_json(&self.expand_post_url(version), &query.body())
            .await
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

    /// The address one `$validate-code` run reads.
    ///
    /// `$validate-code` takes its parameters in the query of a `GET`
    /// (<https://hl7.org/fhir/R4B/codesystem-operation-validate-code.html>),
    /// and each one is percent-encoded, so a canonical carrying its own query
    /// string stays inside the parameter it belongs to. An instance-level run
    /// puts the resource id in the path as one encoded segment, which is what
    /// the read interaction does with an id
    /// (<https://hl7.org/fhir/R4B/http.html#read>).
    pub(crate) fn validate_code_url(
        &self,
        version: FhirVersion,
        request: &ValidateRequest,
    ) -> String {
        let mut url = RequestUrl::new()
            .segment(version.segment())
            .segment(request.on.resource_type());
        if request.instance() {
            url = url.segment(&request.id);
        }
        request
            .append(url.segment("$validate-code"))
            .render(&self.root)
    }

    /// Runs `$validate-code` and reads the `Parameters` it answers.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// refusal arrives as [`FhirError::Refused`] carrying the server's own
    /// `OperationOutcome`. A code the system does not hold is not a refusal:
    /// the operation answers `result` false and says why.
    pub(crate) async fn validate_code(
        &self,
        version: FhirVersion,
        request: &ValidateRequest,
    ) -> Result<ParametersAnswer, FhirError> {
        self.get_json(&self.validate_code_url(version, request))
            .await
    }

    /// The address one `CodeSystem/$subsumes` run reads.
    ///
    /// `$subsumes` takes its parameters in the query of a `GET`
    /// (<https://hl7.org/fhir/R4B/codesystem-operation-subsumes.html>).
    pub(crate) fn subsumes_url(&self, version: FhirVersion, request: &SubsumesRequest) -> String {
        let mut url = RequestUrl::new()
            .segment(version.segment())
            .segment(CODE_SYSTEM);
        if request.instance() {
            url = url.segment(&request.id);
        }
        request.append(url.segment("$subsumes")).render(&self.root)
    }

    /// Runs `CodeSystem/$subsumes` and reads the `Parameters` it answers.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// system that answers no subsumption arrives as [`FhirError::Refused`]
    /// carrying the server's own `OperationOutcome`.
    pub(crate) async fn subsumes(
        &self,
        version: FhirVersion,
        request: &SubsumesRequest,
    ) -> Result<ParametersAnswer, FhirError> {
        self.get_json(&self.subsumes_url(version, request)).await
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

    /// The address the identity provider redirects a sign-in back to.
    ///
    /// It is derived from the page, like the FHIR base, so one bundle serves
    /// every deployment. The same value is sent with the authorization request
    /// and with the token request, which RFC 6749 §4.1.3 requires, and it is
    /// what the operator registers with the identity provider.
    pub(crate) fn redirect_uri(&self) -> String {
        RequestUrl::new()
            .segment(UI_PREFIX.trim_start_matches('/'))
            .segment(CALLBACK_PATH)
            .render(&self.root)
    }

    /// What a sign-in against this root needs, when it offers one.
    ///
    /// The capability statement is asked first, because SMART names
    /// `SMART-on-FHIR` in `rest.security.service` and serves its discovery
    /// document only where it does
    /// (<https://hl7.org/fhir/smart-app-launch/conformance.html>). A
    /// deployment that configured no issuer serves no document, and probing
    /// for one would put a `404` in every reader's browser console.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A root
    /// that offers no sign-in answers `Ok(None)`, which is a different answer
    /// from a read that failed.
    pub(crate) async fn sign_in_offer(
        &self,
        version: FhirVersion,
    ) -> Result<Option<SignIn>, FhirError> {
        // The two reads are boxed so this future holds a pointer to each
        // rather than both state machines inlined into it.
        let statement = Box::pin(self.capability_statement(version)).await?;
        if !statement.declares_smart() {
            return Ok(None);
        }
        let document = Box::pin(self.smart_configuration(version)).await?;
        Ok(document.sign_in())
    }

    /// The address the served version's SMART discovery document sits at.
    ///
    /// SMART puts the document under the FHIR base of the server it describes,
    /// so it is read per version, same-origin, like every other request here
    /// (<https://hl7.org/fhir/smart-app-launch/conformance.html>).
    pub(crate) fn smart_configuration_url(&self, version: FhirVersion) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment(".well-known")
            .segment("smart-configuration")
            .render(&self.root)
    }

    /// Reads the SMART discovery document of one served version.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// deployment that configured no issuer serves nothing here, which arrives
    /// as a `404` refusal and means the viewer offers no sign-in.
    pub(crate) async fn smart_configuration(
        &self,
        version: FhirVersion,
    ) -> Result<SmartConfiguration, FhirError> {
        let url = self.smart_configuration_url(version);
        let response = send(Request::get(&url).header("Accept", SMART_JSON), &url).await?;
        self.read_json(response, &url).await
    }
}

/// The write client: create, update, delete, and the version history.
///
/// The server carries these interactions on `CodeSystem`, `ValueSet`, and
/// `ConceptMap` (<https://hl7.org/fhir/R4B/http.html>), gated on a SMART scope
/// where the deployment configured an issuer.
// TODO(#631): the editor screens are the callers; until they land the seam has
// none, and the expectation below reports itself the moment that changes.
#[expect(
    dead_code,
    reason = "the write seam's callers are the editor screens of #631"
)]
impl FhirClient {
    /// The address a create posts to.
    pub(crate) fn create_url(&self, version: FhirVersion, resource_type: &str) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment(resource_type)
            .render(&self.root)
    }

    /// The address one resource's version history is read from.
    ///
    /// `_history` on an instance answers a `history` `Bundle`
    /// (<https://hl7.org/fhir/R4B/http.html#history>).
    pub(crate) fn history_url(
        &self,
        version: FhirVersion,
        resource_type: &str,
        id: &str,
    ) -> String {
        RequestUrl::new()
            .segment(version.segment())
            .segment(resource_type)
            .segment(id)
            .segment("_history")
            .render(&self.root)
    }

    /// Creates a resource (<https://hl7.org/fhir/R4B/http.html#create>).
    ///
    /// `token` is the bearer to present, when one is held; without it the
    /// request goes unauthenticated and a server that gates its writes answers
    /// `401`.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// refusal arrives as [`FhirError::Refused`] carrying the server's own
    /// `OperationOutcome`, which the screen renders verbatim.
    pub(crate) async fn create(
        &self,
        version: FhirVersion,
        resource_type: &str,
        body: &str,
        token: Option<&str>,
    ) -> Result<Written, FhirError> {
        let url = self.create_url(version, resource_type);
        let request = bearing(Request::post(&url), token)
            .header("Accept", FHIR_JSON)
            .header("Content-Type", FHIR_JSON);
        self.write(request, body, &url).await
    }

    /// Updates a resource (<https://hl7.org/fhir/R4B/http.html#update>).
    ///
    /// `version_id` is the version being replaced, sent as `If-Match`, so a
    /// resource another reader has changed since is refused with `412` rather
    /// than overwritten (<https://hl7.org/fhir/R4B/http.html#concurrency>). A
    /// caller that holds no version sends none, which is an unconditional
    /// update.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn update(
        &self,
        version: FhirVersion,
        resource_type: &str,
        id: &str,
        body: &str,
        version_id: Option<&str>,
        token: Option<&str>,
    ) -> Result<Written, FhirError> {
        let url = self.resource_url(version, resource_type, id);
        let mut request = bearing(Request::put(&url), token)
            .header("Accept", FHIR_JSON)
            .header("Content-Type", FHIR_JSON);
        if let Some(held) = version_id {
            request = request.header("If-Match", &if_match(held));
        }
        self.write(request, body, &url).await
    }

    /// Deletes a resource (<https://hl7.org/fhir/R4B/http.html#delete>).
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong. A
    /// server that refuses says why in an `OperationOutcome`.
    pub(crate) async fn delete(
        &self,
        version: FhirVersion,
        resource_type: &str,
        id: &str,
        token: Option<&str>,
    ) -> Result<(), FhirError> {
        let url = self.resource_url(version, resource_type, id);
        let request = bearing(Request::delete(&url), token).header("Accept", FHIR_JSON);
        let response = send(request, &url).await?;
        let status = status_of(&response, &url)?;
        if status.is_success() {
            Ok(())
        } else {
            Err(failure(&response, status, &url).await)
        }
    }

    /// Reads one resource's version history.
    ///
    /// # Errors
    ///
    /// Returns the variant of [`FhirError`] describing what went wrong.
    pub(crate) async fn history(
        &self,
        version: FhirVersion,
        resource_type: &str,
        id: &str,
        token: Option<&str>,
    ) -> Result<History, FhirError> {
        let url = self.history_url(version, resource_type, id);
        let request = bearing(Request::get(&url), token).header("Accept", FHIR_JSON);
        let response = send(request, &url).await?;
        self.read_json(response, &url).await
    }

    /// Sends one write and reads what the server committed.
    async fn write(
        &self,
        request: gloo_net::http::RequestBuilder,
        body: &str,
        url: &str,
    ) -> Result<Written, FhirError> {
        let built = request.body(body).map_err(|error| FhirError::Transport {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        let response = built.send().await.map_err(|error| FhirError::Transport {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        let status = status_of(&response, url)?;
        if !status.is_success() {
            return Err(failure(&response, status, url).await);
        }
        let headers = response.headers();
        let etag = headers.get("etag");
        let location = headers
            .get("location")
            .or_else(|| headers.get("content-location"));
        let text = response.text().await.map_err(|error| FhirError::Decode {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        // A server asked for a minimal return answers no body, which is a
        // success and not a decode failure
        // (<https://hl7.org/fhir/R4B/http.html#return>).
        let resource = if text.trim().is_empty() {
            None
        } else {
            Some(
                serde_json::from_str(&text).map_err(|error| FhirError::Decode {
                    url: url.to_owned(),
                    message: error.to_string(),
                })?,
            )
        };
        Ok(Written {
            resource,
            etag,
            location,
        })
    }
}

impl FhirClient {
    /// Sends a FHIR JSON `GET` and decodes the resource it answers.
    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, FhirError> {
        let response = send(Request::get(url).header("Accept", FHIR_JSON), url).await?;
        self.read_json(response, url).await
    }

    /// Sends a FHIR JSON `POST` and decodes the resource it answers.
    async fn post_json<T: DeserializeOwned>(&self, url: &str, body: &str) -> Result<T, FhirError> {
        let request = Request::post(url)
            .header("Accept", FHIR_JSON)
            .header("Content-Type", FHIR_JSON)
            .body(body)
            .map_err(|error| FhirError::Transport {
                url: url.to_owned(),
                message: error.to_string(),
            })?;
        let response = request.send().await.map_err(|error| FhirError::Transport {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        self.read_json(response, url).await
    }

    /// Turns an answer into the resource it carries, or into the refusal.
    async fn read_json<T: DeserializeOwned>(
        &self,
        response: Response,
        url: &str,
    ) -> Result<T, FhirError> {
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

/// Adds the bearer credential to `request`, when one is held (RFC 6750 §2.1).
#[expect(
    dead_code,
    reason = "the write seam's callers are the editor screens of #631"
)]
fn bearing(
    request: gloo_net::http::RequestBuilder,
    token: Option<&str>,
) -> gloo_net::http::RequestBuilder {
    match token {
        Some(held) => request.header("Authorization", &format!("Bearer {held}")),
        None => request,
    }
}

/// Exchanges an authorization code for an access token.
///
/// The request is a public client's: the `code_verifier` and the `client_id`
/// are what bind it to the authorization that issued the code, and no client
/// secret travels (RFC 6749 §4.1.3, RFC 7636 §4.5, and
/// <https://hl7.org/fhir/smart-app-launch/app-launch.html>). The endpoint is
/// the one the server's own discovery document named, so nothing here is
/// configured into the bundle.
///
/// # Errors
///
/// Returns the variant of [`FhirError`] describing what went wrong. An issuer
/// that refuses answers RFC 6749 §5.2, which carries no `OperationOutcome`, so
/// it arrives as [`FhirError::Status`] with the body as the evidence.
pub(crate) async fn exchange_code(
    sign_in: &SignIn,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenAnswer, FhirError> {
    let body = sign_in.token_request_body(code, redirect_uri, verifier);
    post_form(&sign_in.token_endpoint, &body).await
}

/// Revokes the held token at the issuer's revocation endpoint (RFC 7009 §2.1).
///
/// # Errors
///
/// Returns the variant of [`FhirError`] describing what went wrong. RFC 7009
/// §2.2 makes revoking a token the issuer does not know a success, so a refusal
/// here is the issuer's own and is reported rather than hidden.
pub(crate) async fn revoke(sign_in: &SignIn, token: &str) -> Result<(), FhirError> {
    let Some(endpoint) = sign_in.revocation_endpoint.as_deref() else {
        return Ok(());
    };
    let body = sign_in.revocation_request_body(token);
    let request = Request::post(endpoint)
        .header("Content-Type", FORM_ENCODED)
        .body(&body)
        .map_err(|error| FhirError::Transport {
            url: endpoint.to_owned(),
            message: error.to_string(),
        })?;
    let response = request.send().await.map_err(|error| FhirError::Transport {
        url: endpoint.to_owned(),
        message: error.to_string(),
    })?;
    let status = status_of(&response, endpoint)?;
    if status.is_success() {
        Ok(())
    } else {
        Err(failure(&response, status, endpoint).await)
    }
}

/// Posts a form-encoded body to an OAuth endpoint and decodes the answer.
async fn post_form<T: DeserializeOwned>(url: &str, body: &str) -> Result<T, FhirError> {
    let request = Request::post(url)
        .header("Accept", SMART_JSON)
        .header("Content-Type", FORM_ENCODED)
        .body(body)
        .map_err(|error| FhirError::Transport {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
    let response = request.send().await.map_err(|error| FhirError::Transport {
        url: url.to_owned(),
        message: error.to_string(),
    })?;
    let status = status_of(&response, url)?;
    if !status.is_success() {
        return Err(failure(&response, status, url).await);
    }
    let text = response.text().await.map_err(|error| FhirError::Decode {
        url: url.to_owned(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|error| FhirError::Decode {
        url: url.to_owned(),
        message: error.to_string(),
    })
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
    fn a_lookup_address_is_the_one_a_reader_would_type() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        let request = LookupRequest {
            system: "http://snomed.info/sct".to_owned(),
            code: "404684003".to_owned(),
            ..LookupRequest::default()
        };
        assert_eq!(
            client.lookup_url(FhirVersion::R5, &request),
            "https://tx.example.org/r5/CodeSystem/$lookup\
             ?system=http%3A%2F%2Fsnomed.info%2Fsct&code=404684003",
            "the operation name survives the path and the canonical survives the query"
        );
    }

    #[test]
    fn an_inline_expansion_addresses_the_operation_and_carries_its_body() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.expand_post_url(FhirVersion::R4B),
            "https://tx.example.org/r4b/ValueSet/$expand",
            "a POST carries its parameters in the body, so the address has no query"
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
    fn a_validation_addresses_the_resource_type_it_is_invoked_on() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        let request = ValidateRequest {
            url: "https://terminology.example/x?edition=2031".to_owned(),
            code: "cat".to_owned(),
            ..ValidateRequest::default()
        };
        assert_eq!(
            client.validate_code_url(FhirVersion::R4B, &request),
            "https://tx.example.org/r4b/CodeSystem/$validate-code\
             ?url=https%3A%2F%2Fterminology.example%2Fx%3Fedition%3D2031&code=cat",
            "the operation name survives the path and the canonical survives the query"
        );
        assert_eq!(
            client.validate_code_url(
                FhirVersion::R5,
                &ValidateRequest {
                    on: validation::ValidateOn::ValueSet,
                    id: "a/b".to_owned(),
                    ..request
                }
            ),
            "https://tx.example.org/r5/ValueSet/a%2Fb/$validate-code?code=cat",
            "an id a reader typed cannot reach a route the viewer did not mean to ask for"
        );
    }

    #[test]
    fn a_subsumption_addresses_the_code_system_and_carries_both_codes() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        let request = SubsumesRequest {
            system: "http://snomed.info/sct".to_owned(),
            code_a: "404684003".to_owned(),
            code_b: "64572001".to_owned(),
            ..SubsumesRequest::default()
        };
        assert_eq!(
            client.subsumes_url(FhirVersion::R6, &request),
            "https://tx.example.org/r6/CodeSystem/$subsumes\
             ?system=http%3A%2F%2Fsnomed.info%2Fsct&codeA=404684003&codeB=64572001"
        );
        assert_eq!(
            client.subsumes_url(
                FhirVersion::R4,
                &SubsumesRequest {
                    id: "animals".to_owned(),
                    system: String::new(),
                    ..request
                }
            ),
            "https://tx.example.org/r4/CodeSystem/animals/$subsumes?codeA=404684003&codeB=64572001",
            "an instance run names the system in the path and nowhere else"
        );
    }

    #[test]
    fn the_sign_in_document_sits_under_the_base_it_describes() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.smart_configuration_url(FhirVersion::R4B),
            "https://tx.example.org/r4b/.well-known/smart-configuration",
            "SMART puts the document under the FHIR base of the server it describes"
        );
    }

    #[test]
    fn the_redirect_address_is_the_bundle_s_own_callback() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.redirect_uri(),
            "https://tx.example.org/ui/callback",
            "the same value is sent with the authorization and the token request"
        );
        let below = FhirClient {
            root: "https://hospital.example/terminology".to_owned(),
        };
        assert_eq!(
            below.redirect_uri(),
            "https://hospital.example/terminology/ui/callback",
            "a server mounted below the origin redirects back to its own mount"
        );
    }

    #[test]
    fn a_create_addresses_the_resource_type_and_a_history_the_instance() {
        let client = FhirClient {
            root: "https://tx.example.org".to_owned(),
        };
        assert_eq!(
            client.create_url(FhirVersion::R5, CODE_SYSTEM),
            "https://tx.example.org/r5/CodeSystem",
            "a create posts to the type, which is what assigns the id"
        );
        assert_eq!(
            client.history_url(FhirVersion::R6, VALUE_SET, "a/b"),
            "https://tx.example.org/r6/ValueSet/a%2Fb/_history",
            "an id a reader typed cannot reach a route the viewer did not mean to ask for"
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
