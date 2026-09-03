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
    /// The canonicals of the systems the server does not serve
    /// (`x-caused-by-unknown-system`, the terminology ecosystem's output).
    pub unknown_systems: Vec<String>,
    /// The systems a `CodeableConcept`'s codings name that the server does not
    /// serve (`x-unknown-system`, the ecosystem's output for that input).
    pub x_unknown_systems: Vec<String>,
    /// The `codeableConcept` input, echoed (an R5 output, pre-adopted).
    pub codeable_concept: Option<Vec<CodingRef>>,
}

/// The `result = false` outcome for a system or version the server does not
/// serve: the code is echoed, and the system is named for the validator.
fn unserved(
    registry: &Registry,
    url: &str,
    version: Option<&str>,
    code: Option<&str>,
    expression: &str,
) -> ValidationOutcome {
    let valid: Vec<String> = registry
        .versions(url)
        .map(|p| p.identity().version.clone())
        .collect();
    let (canonical, issue) =
        super::unknown_system(url, version, super::at(expression, "system"), &valid);
    ValidationOutcome {
        result: false,
        message: Some(issue.text.clone()),
        display: None,
        code: code.map(str::to_owned),
        system: Some(url.to_owned()),
        version: version.map(str::to_owned),
        issues: vec![issue],
        unknown_systems: vec![canonical],
        x_unknown_systems: Vec::new(),
        codeable_concept: None,
    }
}

/// Resolves the system for a validation; a system or version the server does
/// not serve is a `false` result, never an error (the ecosystem's rule).
fn resolve_for(
    registry: &Registry,
    invocation: &Invocation,
    url: Option<&str>,
    version: Option<&str>,
    code: Option<&str>,
    expression: &str,
) -> Result<Result<super::Resolved, ValidationOutcome>, OperationError> {
    match resolve(registry, invocation, url, version) {
        Ok(resolved) => Ok(Ok(resolved)),
        Err(OperationError::UnknownSystem(url)) => {
            Ok(Err(unserved(registry, &url, None, code, expression)))
        }
        Err(OperationError::UnknownVersion { url, version }) => Ok(Err(unserved(
            registry,
            &url,
            Some(&version),
            code,
            expression,
        ))),
        Err(error) => Err(error),
    }
}

/// Runs `$validate-code`.
///
/// # Errors
///
/// Returns [`OperationError`] for an inline `codeSystem`, none or more than
/// one of the code inputs, a coding whose system contradicts `url`, or a
/// provider failure. An unknown code, system, or version is a `false`
/// result, never an error.
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
        let resolved = match resolve_for(registry, invocation, url, version, Some(code), "code")? {
            Ok(resolved) => resolved,
            Err(unserved) => return Ok(unserved),
        };
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
        let resolved = match resolve_for(
            registry,
            invocation,
            url.or(coding.system.as_deref()),
            coding.version.as_deref().or(version),
            Some(code),
            "coding",
        )? {
            Ok(resolved) => resolved,
            Err(unserved) => return Ok(unserved),
        };
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
    let resolved = match resolve_for(registry, invocation, url, version, None, "codeableConcept")? {
        Ok(resolved) => resolved,
        Err(unserved) => return Ok(unserved),
    };
    let mut outcome = check_codeable_concept(&resolved.provider, codings, language)?;
    outcome.codeable_concept = Some(codings.clone());
    Ok(outcome)
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
    for (index, coding) in codings.iter().enumerate() {
        if coding.system.as_deref().is_some_and(|s| s != target) {
            continue;
        }
        let Some(code) = coding.code.as_deref() else {
            continue;
        };
        any = true;
        let base = format!("CodeableConcept.coding[{index}]");
        let outcome = check(provider, code, coding.display.as_deref(), language, &base)?;
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
            expression: None,
        }],
        unknown_systems: Vec::new(),
        x_unknown_systems: Vec::new(),
        codeable_concept: None,
    })
}

/// Checks one code against a system: found, its display matches when one was
/// asserted, and inactive is a warning, never a `false`.
fn check(
    provider: &std::sync::Arc<dyn CodeSystemProvider>,
    code: &str,
    display: Option<&str>,
    language: Option<&str>,
    expression: &str,
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
                expression: super::at(expression, "code"),
            }],
            unknown_systems: Vec::new(),
            x_unknown_systems: Vec::new(),
            codeable_concept: None,
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
                expression: super::at(expression, "display"),
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
        unknown_systems: Vec::new(),
        x_unknown_systems: Vec::new(),
        codeable_concept: None,
    })
}
