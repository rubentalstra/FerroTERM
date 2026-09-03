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
    /// A `check-system-version` parameter forbids the version a value set
    /// uses (the ecosystem's `version-error`, an `exception`).
    #[error("{0}")]
    VersionCheck(String),
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
            Self::NotSupported(_) | Self::CannotDetermine(_) => "not-supported",
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownCode { .. }
            | Self::UnknownValueSet(_)
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
            Self::NotSupported(_) | Self::Provider(_) => "not-supported",
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownValueSet(_)
            | Self::UnknownConceptMap(_) => "not-found",
            Self::UnknownCode { .. } | Self::InvalidCode { .. } => "invalid-code",
            Self::ValueSetInvalid(_) => "vs-invalid",
            Self::VersionCheck(_) => "version-error",
            Self::TooCostly(_) => "too-costly",
            Self::CannotDetermine(_) => "cannot-determine",
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
            | Self::VersionCheck(_)
            | Self::UnknownCode { .. }
            | Self::InvalidCode { .. }
            | Self::NotSupported(_) => StatusCode::BAD_REQUEST,
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownValueSet(_)
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
            ComposeError::UnknownValueSet(url) => Self::UnknownValueSet(url),
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

impl Sources<'_> {
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
                    Some(Ok(compose)) => Ok(Arc::new(crate::valueset::model::ValueSetModel {
                        url: url.to_owned(),
                        version: None,
                        name: None,
                        title: None,
                        status: String::from("active"),
                        experimental: None,
                        date: None,
                        publisher: None,
                        description: None,
                        immutable: None,
                        compose,
                    })),
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
