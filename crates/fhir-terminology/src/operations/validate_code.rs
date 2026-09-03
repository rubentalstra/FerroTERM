//! `CodeSystem/$validate-code` in the terms every served FHIR version shares.
//!
//! One of `code` (with `url`), `coding`, or `codeableConcept` is checked
//! against a served system; the outcome says whether the code is in the
//! system, why not, and the display the system prefers
//! (<https://hl7.org/fhir/R4B/codesystem-operation-validate-code.html>,
//! <https://hl7.org/fhir/R5/codesystem-operation-validate-code.html>).

use super::{CodingRef, Invocation, Issue, OperationError, resolve};
use crate::language;
use crate::provider::CodeSystemProvider;
use crate::registry::Registry;

/// The input of `CodeSystem/$validate-code`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidateCodeInput {
    /// The code system URI (`url`).
    pub url: Option<String>,
    /// The code system version.
    pub version: Option<String>,
    /// Whether an inline `codeSystem` resource was given; not supported.
    pub inline_code_system: bool,
    /// The code.
    pub code: Option<String>,
    /// The display the client asserts.
    pub display: Option<String>,
    /// The coding, instead of `code`.
    pub coding: Option<CodingRef>,
    /// The codings of a `codeableConcept`, instead of `code`.
    pub codeable_concept: Option<Vec<CodingRef>>,
    /// The language of the display (a BCP 47 range list).
    pub display_language: Option<String>,
}

/// The outcome of `CodeSystem/$validate-code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationOutcome {
    /// Whether the code is valid in the system.
    pub result: bool,
    /// Why the result is what it is, when there is something to say.
    pub message: Option<String>,
    /// The display the system prefers, when the code was found.
    pub display: Option<String>,
    /// The code that was checked, when one was.
    pub code: Option<String>,
    /// The system it was checked against.
    pub system: Option<String>,
    /// The version it was checked against.
    pub version: Option<String>,
    /// The itemised issues, for the versions that declare `issues`
    /// (<https://hl7.org/fhir/R5/codesystem-operation-validate-code.html>).
    pub issues: Vec<Issue>,
}

/// Runs `$validate-code`.
///
/// # Errors
///
/// Returns [`OperationError`] for an inline `codeSystem`, none or more than
/// one of the code inputs, a coding whose system contradicts `url`, an
/// unknown system or version, or a provider failure. An unknown code is a
/// `false` result, never an error.
pub fn validate_code(
    registry: &Registry,
    invocation: &Invocation,
    input: &ValidateCodeInput,
) -> Result<ValidationOutcome, OperationError> {
    if input.inline_code_system {
        return Err(OperationError::NotSupported(String::from(
            "validating against an inline `codeSystem` resource is not supported; name a served system with `url`",
        )));
    }
    let inputs = usize::from(input.code.is_some())
        + usize::from(input.coding.is_some())
        + usize::from(input.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    let url = input.url.as_deref();
    let version = input.version.as_deref();
    let language = input.display_language.as_deref();
    if let Some(code) = input.code.as_deref() {
        let resolved = resolve(registry, invocation, url, version)?;
        return check(
            &resolved.provider,
            code,
            input.display.as_deref(),
            language,
            "code",
        );
    }
    if let Some(coding) = &input.coding {
        if let (Some(url), Some(system)) = (url, coding.system.as_deref())
            && url != system
        {
            return Err(OperationError::Invalid(format!(
                "`coding.system` `{system}` does not match `url` `{url}`"
            )));
        }
        let code = coding
            .code
            .as_deref()
            .ok_or_else(|| OperationError::Required(String::from("`coding.code` is required")))?;
        let resolved = resolve(
            registry,
            invocation,
            url.or(coding.system.as_deref()),
            coding.version.as_deref().or(version),
        )?;
        return check(
            &resolved.provider,
            code,
            coding.display.as_deref().or(input.display.as_deref()),
            language,
            "coding",
        );
    }
    let Some(codings) = &input.codeable_concept else {
        return Err(OperationError::Required(String::from(
            "provide one of `code`, `coding`, or `codeableConcept`",
        )));
    };
    let resolved = resolve(registry, invocation, url, version)?;
    check_codeable_concept(&resolved.provider, codings, language)
}

/// Validates the codings of a `CodeableConcept` that name the system: the
/// first valid one answers, else the failures are joined.
fn check_codeable_concept(
    provider: &std::sync::Arc<dyn CodeSystemProvider>,
    codings: &[CodingRef],
    language: Option<&str>,
) -> Result<ValidationOutcome, OperationError> {
    let identity = provider.identity();
    let target = identity.url.clone();
    let mut messages = Vec::new();
    let mut any = false;
    for coding in codings {
        if coding.system.as_deref().is_some_and(|s| s != target) {
            continue;
        }
        let Some(code) = coding.code.as_deref() else {
            continue;
        };
        any = true;
        let outcome = check(
            provider,
            code,
            coding.display.as_deref(),
            language,
            "codeableConcept",
        )?;
        if outcome.result {
            return Ok(outcome);
        }
        if let Some(message) = outcome.message {
            messages.push(message);
        }
    }
    let message = if any {
        messages.join("; ")
    } else {
        format!("no coding of the CodeableConcept is in code system `{target}`")
    };
    Ok(ValidationOutcome {
        result: false,
        message: Some(message.clone()),
        display: None,
        code: None,
        system: Some(target),
        version: Some(identity.version.clone()),
        issues: vec![Issue {
            severity: "error",
            code: "code-invalid",
            kind: "invalid-code",
            text: message,
            expression: Some("codeableConcept"),
        }],
    })
}

/// Checks one code against a system: found, its display matches when one was
/// asserted, and inactive is a warning, never a `false`.
fn check(
    provider: &std::sync::Arc<dyn CodeSystemProvider>,
    code: &str,
    display: Option<&str>,
    language: Option<&str>,
    expression: &'static str,
) -> Result<ValidationOutcome, OperationError> {
    let identity = provider.identity();
    let Some(located) = provider.locate(code)? else {
        let text = format!(
            "code `{code}` is not in code system `{}` version `{}`",
            identity.url, identity.version
        );
        return Ok(ValidationOutcome {
            result: false,
            message: Some(text.clone()),
            display: None,
            code: Some(code.to_owned()),
            system: Some(identity.url.clone()),
            version: Some(identity.version.clone()),
            issues: vec![Issue {
                severity: "error",
                code: "code-invalid",
                kind: "invalid-code",
                text,
                expression: Some(expression),
            }],
        });
    };
    let concept = located.concept;
    let language = language::for_provider(provider.as_ref(), language);
    let language = language.as_deref();
    let preferred = provider.display(concept, language)?;
    let mut messages = Vec::new();
    let mut issues = Vec::new();
    let mut result = true;
    if let Some(display) = display {
        let case_sensitive = provider.declaration().case_sensitive;
        let matches = |term: &str| {
            if case_sensitive {
                term == display
            } else {
                term.eq_ignore_ascii_case(display)
            }
        };
        let known = provider
            .designations(concept, None)?
            .iter()
            .any(|d| matches(&d.value))
            || preferred.as_deref().is_some_and(matches);
        if !known {
            result = false;
            let text = match &preferred {
                Some(preferred) => format!(
                    "the display `{display}` is not a valid display for `{}#{code}`; the display is `{preferred}`",
                    identity.url
                ),
                None => format!(
                    "the display `{display}` is not a valid display for `{}#{code}`",
                    identity.url
                ),
            };
            messages.push(text.clone());
            issues.push(Issue {
                severity: "error",
                code: "invalid",
                kind: "invalid-display",
                text,
                expression: Some(expression),
            });
        }
    }
    let status = provider.status(concept)?;
    if !status.active {
        let text = format!(
            "code `{code}` is inactive in `{}` version `{}`",
            identity.url, identity.version
        );
        messages.push(text.clone());
        issues.push(Issue {
            severity: "warning",
            code: "business-rule",
            kind: "status-check",
            text,
            expression: None,
        });
    }
    Ok(ValidationOutcome {
        result,
        message: (!messages.is_empty()).then(|| messages.join("; ")),
        display: preferred,
        code: Some(located.code),
        system: Some(identity.url.clone()),
        version: Some(identity.version.clone()),
        issues,
    })
}
