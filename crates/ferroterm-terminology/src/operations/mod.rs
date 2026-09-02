//! The FHIR terminology operations over the seam, one module each.
//!
//! Each operation takes the generated request type of a FHIR version and
//! returns the generated response type; the parameter set is exactly what
//! that version's `OperationDefinition` declares because the types are
//! generated from it. Every client input error is an [`OperationError`] the
//! server maps to an `OperationOutcome` issue and an HTTP status.

pub mod expand;
pub mod lookup;
pub mod subsumes;
pub mod validate_code;
pub mod value_set_validate_code;

use std::sync::Arc;

use http::StatusCode;

use crate::provider::{CodeSystemProvider, ProviderError};
use crate::registry::{Registry, ResolveError, Resolved};

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
    /// The value set is not known.
    #[error("value set `{0}` is not known")]
    UnknownValueSet(String),
    /// The value set cannot be expanded as defined.
    #[error("{0}")]
    ValueSetInvalid(String),
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
            Self::Invalid(_) | Self::ValueSetInvalid(_) => "invalid",
            Self::NotSupported(_) => "not-supported",
            Self::UnknownSystem(_)
            | Self::UnknownVersion { .. }
            | Self::UnknownCode { .. }
            | Self::UnknownValueSet(_) => "not-found",
            Self::Provider(_) => "exception",
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
            | Self::UnknownCode { .. }
            | Self::NotSupported(_) => StatusCode::BAD_REQUEST,
            Self::UnknownSystem(_) | Self::UnknownVersion { .. } | Self::UnknownValueSet(_) => {
                StatusCode::NOT_FOUND
            }
            // NOTE: 422 is the status for a resource that breaks the server's
            // rules (<https://hl7.org/fhir/R4B/http.html#status-codes>); a compose
            // the layer cannot evaluate is that resource.
            Self::ValueSetInvalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
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
            other => Self::Provider(other),
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
pub(crate) fn code_text(value: Option<&ferroterm_fhir::r4b::primitives::Code>) -> Option<&str> {
    value.and_then(|v| v.value.as_deref())
}

/// The value of an optional R4B `string`.
pub(crate) fn string_text(value: Option<&ferroterm_fhir::r4b::primitives::String>) -> Option<&str> {
    value.and_then(|v| v.value.as_deref())
}

/// The value of an optional R4B `uri`.
pub(crate) fn uri_text(value: Option<&ferroterm_fhir::r4b::primitives::Uri>) -> Option<&str> {
    value.and_then(|v| v.value.as_deref())
}

/// A `Coding`'s system, version, code, and display as text.
pub(crate) fn coding_parts(
    coding: &ferroterm_fhir::r4b::coding::Coding,
) -> (Option<&str>, Option<&str>, Option<&str>, Option<&str>) {
    (
        uri_text(coding.system.as_ref()),
        string_text(coding.version.as_ref()),
        code_text(coding.code.as_ref()),
        string_text(coding.display.as_ref()),
    )
}

impl From<crate::compose::ComposeError> for OperationError {
    fn from(error: crate::compose::ComposeError) -> Self {
        use crate::compose::ComposeError;
        match error {
            ComposeError::Resolve(error) => error.into(),
            ComposeError::Provider { source, .. } => source.into(),
            ComposeError::UnknownValueSet(url) => Self::UnknownValueSet(url),
            ComposeError::UnknownCode { .. }
            | ComposeError::NoSystemOrValueSet
            | ComposeError::CriteriaWithoutSystem
            | ComposeError::ConceptsAndFilters
            | ComposeError::Cycle(_) => Self::ValueSetInvalid(error.to_string()),
            ComposeError::NoResolver(_) => Self::NotSupported(error.to_string()),
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
