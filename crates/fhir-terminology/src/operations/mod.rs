//! The FHIR terminology operations over the seam, one module each.
//!
//! Each operation takes the generated request type of a FHIR version and
//! returns the generated response type; the parameter set is exactly what
//! that version's `OperationDefinition` declares because the types are
//! generated from it. Every client input error is an [`OperationError`] the
//! server maps to an `OperationOutcome` issue and an HTTP status.

pub mod display;
pub mod expand;
pub mod lookup;
pub mod subsumes;
pub mod translate;
pub mod validate_code;
pub mod value_set_validate_code;

use std::sync::Arc;

use http::StatusCode;

use crate::provider::{CodeSystemProvider, ProviderError};
use crate::registry::{Registry, ResolveError, Resolved};

/// A coding as an operation input names it, in no FHIR version's types: the
/// system, version, code, and display a `Coding` carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodingRef {
    /// The code system URI.
    pub system: Option<String>,
    /// The code system version.
    pub version: Option<String>,
    /// The code.
    pub code: Option<String>,
    /// The display the client sent.
    pub display: Option<String>,
}

/// Where an operation was invoked: on the type, or on one code system
/// instance the server already resolved from the URL.
#[derive(Debug, Clone)]
pub enum Invocation {
    /// `[base]/CodeSystem/$op`: the request names the system.
    Type,
    /// `[base]/CodeSystem/{id}/$op`: the instance is the system.
    Instance(Resolved),
}

/// A failure the server reports as an `OperationOutcome`.
///
/// Each variant knows its `issue.code` (<https://hl7.org/fhir/R4B/valueset-issue-type.html>)
/// and HTTP status (<https://hl7.org/fhir/R4B/operations.html#3.2.0.6.2>).
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    /// A required parameter (or one of a required group) is absent.
    #[error("{0}")]
    Required(String),
    /// Parameters contradict each other or the invocation.
    #[error("{0}")]
    Invalid(String),
    /// A `displayLanguage` that is not a valid BCP 47 language range list.
    #[error("Invalid displayLanguage: '{0}'")]
    InvalidLanguage(String),
    /// A `check-system-version` parameter forbids the version a value set
    /// uses (the ecosystem's `version-error`, an `exception`).
    #[error("{0}")]
    VersionCheck(String),
    /// A `useSupplement` (or a value set's `valueset-supplement`) names no
    /// loaded supplement.
    #[error(transparent)]
    UnknownSupplement(#[from] crate::registry::UnknownSupplement),
    /// The operation or a parameter combination is not supported here.
    #[error("{0}")]
    NotSupported(String),
    /// The code system is not served.
    #[error("code system `{0}` is not served")]
    UnknownSystem(String),
    /// The code system is served, this version is not.
    #[error("version `{version}` of code system `{url}` is not served")]
    UnknownVersion {
        /// The system.
        url: String,
        /// The version.
        version: String,
    },
    /// The code is not in the code system.
    #[error("code `{code}` is not in code system `{system}` version `{version}`")]
    UnknownCode {
        /// The system.
        system: String,
        /// The version.
        version: String,
        /// The code.
        code: String,
    },
    /// The code is malformed for the code system's grammar.
    #[error("code `{code}` is invalid: {reason}")]
    InvalidCode {
        /// The code as given.
        code: String,
        /// Why it is not a code.
        reason: String,
    },
    /// The value set is not known (the ecosystem's wording, so a validator
    /// recognises it).
    #[error("A definition for the value Set '{0}' could not be found")]
    UnknownValueSet(String),
    /// A value set another value set imports is not known.
    #[error("A definition for the value Set '{0}' could not be found")]
    UnknownImport(String),
    /// The concept map is not known.
    #[error("concept map `{0}` is not known")]
    UnknownConceptMap(String),
    /// The value set cannot be expanded as defined.
    #[error("{0}")]
    ValueSetInvalid(String),
    /// The expansion is larger than the server returns without paging.
    #[error("{0}")]
    TooCostly(String),
    /// The system cannot decide the relationship asked of it.
    #[error("{0}")]
    CannotDetermine(String),
    /// The provider failed.
    #[error("the code system provider failed")]
    Provider(#[source] ProviderError),
}

impl OperationError {
    /// The `OperationOutcome.issue.code`.
    #[must_use]
    pub const fn issue_code(&self) -> &'static str {
        match self {
            Self::Required(_) => "required",
            Self::InvalidCode { .. } => "code-invalid",
            Self::Invalid(_) | Self::ValueSetInvalid(_) => "invalid",
            Self::InvalidLanguage(_) => "processing",
            Self::NotSupported(_) | Self::CannotDetermine(_) => "not-supported",
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownCode { .. }
            | Self::UnknownValueSet(_)
            | Self::UnknownImport(_)
            | Self::UnknownSupplement(_)
            | Self::UnknownConceptMap(_) => "not-found",
            Self::TooCostly(_) => "too-costly",
            Self::VersionCheck(_) | Self::Provider(_) => "exception",
        }
    }

    /// The `tx-issue-type` code of the failure, for `issue.details.coding`
    /// (<https://build.fhir.org/ig/FHIR/fhir-tools-ig/CodeSystem-tx-issue-type.html>).
    #[must_use]
    pub const fn tx_issue_type(&self) -> &'static str {
        match self {
            Self::Required(_) | Self::Invalid(_) => "invalid-data",
            Self::InvalidLanguage(_) => "invalid-display",
            Self::NotSupported(_) | Self::Provider(_) => "not-supported",
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownValueSet(_)
            | Self::UnknownImport(_)
            | Self::UnknownSupplement(_)
            | Self::UnknownConceptMap(_) => "not-found",
            Self::UnknownCode { .. } | Self::InvalidCode { .. } => "invalid-code",
            Self::ValueSetInvalid(_) => "vs-invalid",
            Self::VersionCheck(_) => "version-error",
            Self::TooCostly(_) => "too-costly",
            Self::CannotDetermine(_) => "cannot-determine",
        }
    }

    /// The message id the terminology ecosystem attaches to an outcome of
    /// this failure (the reference server's message key; #189).
    #[must_use]
    pub fn message_id(&self) -> &'static str {
        match self {
            // NOTE: the ecosystem names a pinned import it cannot find on `$expand`
            // by its own key (its `default-valueset-version` cases).
            Self::UnknownImport(url) if url.contains('|') => "VS_EXP_IMPORT_UNK_PINNED",
            Self::UnknownValueSet(_) | Self::UnknownImport(_) => "Unable_to_resolve_value_Set_",
            Self::UnknownVersion { .. } => "UNKNOWN_CODESYSTEM_VERSION_EXP",
            Self::UnknownSystem(_) => "UNKNOWN_CODESYSTEM",
            Self::UnknownSupplement(_) => "VALUESET_SUPPLEMENT_MISSING",
            Self::VersionCheck(_) => "VALUESET_VERSION_CHECK",
            Self::TooCostly(_) => "VALUESET_TOO_COSTLY",
            Self::NotSupported(_) => "CODESYSTEM_NOT_ENUMERABLE",
            Self::UnknownCode { .. } | Self::InvalidCode { .. } => "Unknown_Code_in_Version",
            Self::ValueSetInvalid(_) => "VALUESET_CIRCULAR_REFERENCE",
            Self::InvalidLanguage(_) => "INVALID_DISPLAY_NAME",
            Self::Required(_)
            | Self::Invalid(_)
            | Self::CannotDetermine(_)
            | Self::UnknownConceptMap(_)
            | Self::Provider(_) => "TX_GENERAL_ERROR",
        }
    }

    /// The HTTP status.
    ///
    /// An unknown code answers `400` as the R4B `$lookup` page's own example
    /// does; the normative text asks only for a 4xx or 5xx with an
    /// `OperationOutcome` (no spec fixes the number: our own choice).
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::Required(_)
            | Self::Invalid(_)
            | Self::InvalidLanguage(_)
            | Self::VersionCheck(_)
            | Self::UnknownCode { .. }
            | Self::InvalidCode { .. }
            | Self::NotSupported(_) => StatusCode::BAD_REQUEST,
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownValueSet(_)
            | Self::UnknownImport(_)
            | Self::UnknownSupplement(_)
            | Self::UnknownConceptMap(_) => StatusCode::NOT_FOUND,
            // NOTE: 422 is the status for a resource that breaks the server's
            // rules (<https://hl7.org/fhir/R4B/http.html#status-codes>); a compose
            // the layer cannot evaluate is that resource.
            Self::ValueSetInvalid(_) | Self::TooCostly(_) | Self::CannotDetermine(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::Provider(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ResolveError> for OperationError {
    fn from(error: ResolveError) -> Self {
        match error {
            ResolveError::UnknownSystem(url) => Self::UnknownSystem(url),
            ResolveError::UnknownVersion { url, version } => Self::UnknownVersion { url, version },
        }
    }
}

impl From<ProviderError> for OperationError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::IncompleteContent { .. } | ProviderError::NotEnumerable => {
                Self::NotSupported(error.to_string())
            }
            // NOTE: a filter the system cannot evaluate, an unknown filter value, or
            // a bad regular expression is a defect of the value set, a 422
            // (<https://hl7.org/fhir/R4B/http.html#status-codes>), never a 500.
            ProviderError::UnsupportedFilter { .. }
            | ProviderError::InvalidFilterValue { .. }
            | ProviderError::Regex(_)
            | ProviderError::UnknownCode(_)
            | ProviderError::MalformedImplicitValueSet { .. } => {
                Self::ValueSetInvalid(error.to_string())
            }
            ProviderError::CannotDetermine(_) => Self::CannotDetermine(error.to_string()),
            ProviderError::InvalidCode { code, reason } => Self::InvalidCode { code, reason },
            other @ ProviderError::Storage(_) => Self::Provider(other),
        }
    }
}

/// Resolves the code system an operation works on: the instance when invoked
/// on one, else the `system` (and `version`) the request names.
///
/// # Errors
///
/// Returns [`OperationError::Required`] when a type-level invocation names no
/// system, [`OperationError::Invalid`] when an instance-level invocation names
/// another system, and the unknown-system or unknown-version errors.
pub fn resolve(
    registry: &Registry,
    invocation: &Invocation,
    system: Option<&str>,
    version: Option<&str>,
) -> Result<Resolved, OperationError> {
    match invocation {
        Invocation::Instance(resolved) => {
            let identity = resolved.provider.identity();
            if let Some(system) = system
                && system != identity.url
            {
                return Err(OperationError::Invalid(format!(
                    "the request names code system `{system}`, the instance is `{}`",
                    identity.url
                )));
            }
            if let Some(version) = version
                && version != identity.version
            {
                return Err(OperationError::Invalid(format!(
                    "the request names version `{version}`, the instance is `{}`",
                    identity.version
                )));
            }
            Ok(resolved.clone())
        }
        Invocation::Type => {
            let system = system.ok_or_else(|| {
                OperationError::Required(String::from(
                    "a code system is required: the `system` parameter, a `coding.system`, or an instance-level invocation",
                ))
            })?;
            Ok(registry.resolve(system, version)?)
        }
    }
}

/// Locates `code` in `provider`, as the unknown-code error when absent.
///
/// # Errors
///
/// Returns [`OperationError::UnknownCode`] or the provider's error.
pub fn locate(
    provider: &Arc<dyn CodeSystemProvider>,
    code: &str,
) -> Result<crate::provider::Located, OperationError> {
    provider.locate(code)?.ok_or_else(|| {
        let identity = provider.identity();
        OperationError::UnknownCode {
            system: identity.url.clone(),
            version: identity.version.clone(),
            code: code.to_owned(),
        }
    })
}

/// The value of an optional R4B `code`.
/// The value of an optional R4B `string`.
/// The value of an optional R4B `uri`.
/// A `Coding`'s system, version, code, and display as text.
impl From<crate::compose::ComposeError> for OperationError {
    fn from(error: crate::compose::ComposeError) -> Self {
        use crate::compose::ComposeError;
        match error {
            ComposeError::Resolve(error) => error.into(),
            ComposeError::Provider {
                system,
                source: ProviderError::NotEnumerable,
            } => Self::NotSupported(format!(
                "The code system '{system}' cannot be expanded because its codes cannot be iterated or enumerated in any meaningful sense"
            )),
            ComposeError::Provider { source, .. } => source.into(),
            ComposeError::UnknownValueSet(url) => Self::UnknownImport(url),
            ComposeError::UnknownCode { .. }
            | ComposeError::NoSystemOrValueSet
            | ComposeError::CriteriaWithoutSystem
            | ComposeError::ConceptsAndFilters
            | ComposeError::Cycle(_) => Self::ValueSetInvalid(error.to_string()),
            ComposeError::NoResolver(_) => Self::NotSupported(error.to_string()),
            ComposeError::Negotiation(error) => error.into(),
        }
    }
}

impl From<crate::valueset::negotiation::NegotiationError> for OperationError {
    fn from(error: crate::valueset::negotiation::NegotiationError) -> Self {
        match error {
            crate::valueset::negotiation::NegotiationError::SystemVersion { .. } => {
                Self::VersionCheck(error.to_string())
            }
            crate::valueset::negotiation::NegotiationError::ValueSetVersion { .. } => {
                Self::Invalid(error.to_string())
            }
        }
    }
}

/// Where an operation finds its code systems and value sets.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// The code systems.
    pub registry: &'a Registry,
    /// The value sets.
    pub value_sets: &'a crate::valueset::store::ValueSetStore,
    /// The concept maps.
    pub concept_maps: &'a crate::conceptmap::store::ConceptMapStore,
}

impl<'a> Sources<'a> {
    /// These sources with the dormant supplements `wanted` names layered over
    /// their systems, when any is named; the borrowed sources otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`OperationError::UnknownSupplement`] for a name no loaded
    /// supplement answers to.
    pub fn with_supplements(
        &self,
        wanted: &[String],
    ) -> Result<std::borrow::Cow<'a, Registry>, OperationError> {
        if wanted.is_empty() {
            return Ok(std::borrow::Cow::Borrowed(self.registry));
        }
        Ok(std::borrow::Cow::Owned(
            self.registry.with_supplements(wanted)?,
        ))
    }

    /// The value set an operation names: inline, stored by `url` (and
    /// `version`), or a provider's implicit form.
    ///
    /// # Errors
    ///
    /// Returns [`OperationError::Invalid`] for both an inline and a `url`,
    /// [`OperationError::Required`] for neither, and
    /// [`OperationError::UnknownValueSet`] when nothing answers the `url`.
    pub fn value_set(
        &self,
        inline: Option<
            Result<crate::valueset::model::ValueSetModel, crate::valueset::model::ModelError>,
        >,
        url: Option<&str>,
        version: Option<&str>,
    ) -> Result<Arc<crate::valueset::model::ValueSetModel>, OperationError> {
        match (inline, url) {
            (Some(_), Some(_)) => Err(OperationError::Invalid(String::from(
                "provide either `url` or an inline `valueSet`, not both",
            ))),
            (Some(model), None) => {
                Ok(Arc::new(model.map_err(|e| {
                    OperationError::ValueSetInvalid(e.to_string())
                })?))
            }
            (None, Some(url)) => {
                if let Some(model) = self.value_sets.resolve(url, version) {
                    return Ok(model);
                }
                match self.registry.implicit_value_set(url) {
                    Some(Ok(compose)) => {
                        let metadata = self.registry.implicit_metadata(url);
                        Ok(Arc::new(crate::valueset::model::ValueSetModel {
                            expansion_parameters: Vec::new(),
                            language: None,
                            url: url.to_owned(),
                            version: metadata.version,
                            supplements: Vec::new(),
                            standards_status: None,
                            name: metadata.name,
                            title: metadata.title,
                            status: String::from("active"),
                            experimental: metadata.experimental,
                            date: metadata.date,
                            publisher: None,
                            description: None,
                            immutable: None,
                            compose,
                        }))
                    }
                    Some(Err(source)) => Err(source.into()),
                    None => Err(OperationError::UnknownValueSet(match version {
                        Some(version) => format!("{url}|{version}"),
                        None => url.to_owned(),
                    })),
                }
            }
            (None, None) => Err(OperationError::Required(String::from(
                "a value set is required: the `url` parameter or an inline `valueSet`",
            ))),
        }
    }
}

/// The extension that names an issue's message
/// (<https://hl7.org/fhir/extensions/StructureDefinition-operationoutcome-message-id.html>).
pub const MESSAGE_ID_URL: &str =
    "http://hl7.org/fhir/StructureDefinition/operationoutcome-message-id";

/// One `OperationOutcome.issue` of a validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// `issue.severity`: `error`, `warning`, or `information`.
    pub severity: &'static str,
    /// `issue.code` from the issue-type value set.
    pub code: &'static str,
    /// The `tx-issue-type` code in `issue.details.coding`.
    pub kind: &'static str,
    /// `issue.details.text`.
    pub text: String,
    /// `issue.expression`: the parameter at fault.
    pub expression: Option<String>,
}

impl Issue {
    /// The message id the terminology ecosystem attaches to this issue, the
    /// reference server's message key for its kind and wording (the
    /// ecosystem's test cases fix them; spec-silent, #189).
    #[must_use]
    pub fn message_id(&self) -> &'static str {
        let text = self.text.as_str();
        match self.kind {
            "invalid-code" if text.contains("labeled as a fragment") => "UNKNOWN_CODE_IN_FRAGMENT",
            "invalid-code" if text.contains(" version '") => "Unknown_Code_in_Version",
            "invalid-code" => "Unknown_Code_in",
            "not-in-vs" if text.starts_with("No valid coding") => "TX_GENERAL_CC_ERROR_MESSAGE",
            "not-in-vs" | "this-code-not-in-vs" => {
                "None_of_the_provided_codes_are_in_the_value_set_one"
            }
            "invalid-display" if text.starts_with("There are no valid display names") => {
                "NO_VALID_DISPLAY_FOUND_NONE_FOR_LANG_OK"
            }
            "invalid-display" if text.contains("There are no valid display names") => {
                "NO_VALID_DISPLAY_FOUND_NONE_FOR_LANG_ERR"
            }
            "invalid-display" if text.contains("the whitespace differs") => {
                "Display_Name_WS_for__should_be_one_of__instead_of"
            }
            "invalid-display" => "Display_Name_for__should_be_one_of__instead_of",
            "not-found" if text.starts_with("Required supplement") => "VALUESET_SUPPLEMENT_MISSING",
            "not-found" if text.contains("the value Set") => "Unable_to_resolve_value_Set_",
            "not-found" if text.contains("could not be found, so the value set") => {
                "UNKNOWN_CODESYSTEM_VERSION_EXP"
            }
            "not-found" if text.contains(" version '") => "UNKNOWN_CODESYSTEM_VERSION",
            "not-found" => "UNKNOWN_CODESYSTEM",
            "vs-invalid" if text.contains("for the versionless include") => {
                "VALUESET_VALUE_MISMATCH_DEFAULT"
            }
            "vs-invalid" if text.contains("resulting from the version") => {
                "VALUESET_VALUE_MISMATCH_CHANGED"
            }
            "vs-invalid" => "VALUESET_VALUE_MISMATCH",
            "version-error" => "VALUESET_VERSION_CHECK",
            "code-rule" if text.contains("is abstract") => "ABSTRACT_CODE_NOT_ALLOWED",
            "code-rule" if text.contains("by case") || text.contains("alternate form") => {
                "CODE_CASE_DIFFERENCE"
            }
            "code-rule" => "STATUS_CODE_WARNING_CODE",
            "code-comment" if text.contains("is marked with a status of deprecated") => {
                "CONCEPT_DEPRECATED_IN_VALUESET"
            }
            "code-comment" if text.contains("is deprecated") => "DEPRECATED_CONCEPT_FOUND",
            "code-comment" => "INACTIVE_CONCEPT_FOUND",
            "display-comment" => "INACTIVE_DISPLAY_FOUND",
            "status-check" if text.starts_with("Reference to draft") => "MSG_DRAFT",
            "status-check" if text.starts_with("Reference to deprecated") => "MSG_DEPRECATED",
            "status-check" if text.starts_with("Reference to withdrawn") => "MSG_WITHDRAWN",
            "status-check" => "MSG_EXPERIMENTAL",
            "cannot-infer" => "UNABLE_TO_INFER_CODESYSTEM",
            "invalid-data" if text.contains("is a supplement") => "CODESYSTEM_CS_NO_SUPPLEMENT",
            "invalid-data" if text.contains("references a value set") => {
                "Terminology_TX_System_ValueSet2"
            }
            "invalid-data" if text.contains("absolute reference") => {
                "Terminology_TX_System_Relative"
            }
            "invalid-data" => "Coding_has_no_system__cannot_validate",
            "not-supported" => "CODESYSTEM_NOT_ENUMERABLE",
            "too-costly" => "VALUESET_TOO_COSTLY",
            _ => "TX_GENERAL_ERROR",
        }
    }
}

/// A system the server does not serve, as `x-caused-by-unknown-system` names it.
///
/// Returns the canonical (`url` or `url|version`) and the issue that says so;
/// the wording is the ecosystem's
/// (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>, the
/// `$validate-code` return parameters).
#[must_use]
pub fn unknown_system(
    url: &str,
    version: Option<&str>,
    expression: Option<String>,
    valid: &[String],
) -> (String, Issue) {
    let (canonical, text) = match version {
        Some(version) => (
            format!("{url}|{version}"),
            format!(
                "A definition for CodeSystem '{url}' version '{version}' could not be found, so the code cannot be validated{}",
                valid_versions(valid)
            ),
        ),
        None => (
            url.to_owned(),
            format!(
                "A definition for CodeSystem '{url}' could not be found, so the code cannot be validated"
            ),
        ),
    };
    (
        canonical,
        Issue {
            severity: "error",
            code: "not-found",
            kind: "not-found",
            text,
            expression,
        },
    )
}

/// The `issue.expression` for `leaf` of the element at `base`.
///
/// A coding inside a `CodeableConcept` is addressed by its index and the part
/// at fault (`CodeableConcept.coding[1].display`), the ecosystem's shape; a
/// bare parameter is named as itself.
#[must_use]
pub fn at(base: &str, leaf: &str) -> Option<String> {
    Some(match base {
        "code" => leaf.to_owned(),
        "coding" => format!("Coding.{leaf}"),
        _ if base.starts_with("CodeableConcept.coding[") => format!("{base}.{leaf}"),
        _ => base.to_owned(),
    })
}

/// The warning and the status output an inactive concept earns.
///
/// The ecosystem asks for a `code-comment` warning beside `inactive = true`,
/// and for `status` when the system states one
/// The `code-comment` warning for an active concept whose standards status is
/// `deprecated`, with the `status` output it earns (the ecosystem's
/// `DEPRECATED_CONCEPT_FOUND`); `None` otherwise.
#[must_use]
pub fn deprecated_note(
    code: &str,
    status: &crate::provider::Status,
    expression: Option<String>,
) -> Option<(Issue, String)> {
    if !status.active || status.standards_status.as_deref() != Some("deprecated") {
        return None;
    }
    Some((
        Issue {
            severity: "warning",
            code: "business-rule",
            kind: "code-comment",
            text: format!("The concept '{code}' is deprecated and its use should be reviewed"),
            expression,
        },
        String::from("deprecated"),
    ))
}

/// (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>, "Inactive Codes").
#[must_use]
pub fn inactive_note(
    code: &str,
    status: &crate::provider::Status,
    expression: Option<String>,
) -> Option<(Issue, Option<String>)> {
    if status.active {
        return None;
    }
    let reason = status.inactive_reason.as_deref().unwrap_or("inactive");
    let (text, status_code) = if reason == "inactive" {
        (
            format!("The concept '{code}' has a status of inactive and its use should be reviewed"),
            None,
        )
    } else {
        (
            format!(
                "The concept '{code}' has a status of {reason} and inactive and its use should be reviewed"
            ),
            Some(reason.to_owned()),
        )
    };
    Some((
        Issue {
            severity: "warning",
            code: "business-rule",
            kind: "code-comment",
            text,
            expression,
        },
        status_code,
    ))
}

/// The `issue.expression` of the whole input element (`Coding`, `code`, or the
/// indexed concept coding), for a note about the concept itself.
#[must_use]
pub fn whole(base: &str) -> Option<String> {
    Some(match base {
        "coding" => String::from("Coding"),
        other => other.to_owned(),
    })
}

/// The `status-check` notes the ecosystem asks for when a referenced resource
/// is draft, experimental, deprecated, or withdrawn (its test cases).
#[must_use]
pub fn standing_note(
    kind: &str,
    canonical: &str,
    standing: &crate::provider::Standing,
) -> Option<Issue> {
    let word = if let Some(standards) = standing.standards_status.as_deref()
        && matches!(standards, "deprecated" | "withdrawn" | "draft")
    {
        standards.to_owned()
    } else if standing.status == "draft" {
        String::from("draft")
    } else if standing.experimental {
        String::from("experimental")
    } else {
        return None;
    };
    Some(Issue {
        severity: "information",
        code: "business-rule",
        kind: "status-check",
        text: format!("Reference to {word} {kind} {canonical}"),
        expression: None,
    })
}

/// The `Valid versions: a or b` tail of a not-found text, from the served
/// versions of the system; empty when none is served.
#[must_use]
pub fn valid_versions(versions: &[String]) -> String {
    match versions {
        [] => String::new(),
        [one] => format!(". Valid versions: {one}"),
        [head @ .., last] => format!(". Valid versions: {} or {last}", head.join(", ")),
    }
}
