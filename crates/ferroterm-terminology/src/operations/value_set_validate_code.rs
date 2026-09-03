//! `ValueSet/$validate-code` on R4B
//! (<https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).
//!
//! The value set is inline, stored, or implicit, and is expanded whole; the
//! code is one of `code` with `system`, `coding`, or `codeableConcept`. A code
//! outside the value set is `result = false` with a message, never an error.
//! Beside the R4B outputs, the validation carries the `system`, `version`, and
//! `code` echo and the `issues` a general-purpose server of the terminology
//! ecosystem returns (`tx-issue-type` codings), which the wire layer appends.

use std::sync::Arc;

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::value_set_validate_code::{
    ValueSetValidateCodeRequest, ValueSetValidateCodeResponse,
};

use super::{OperationError, Sources, code_text, coding_parts, string_text, uri_text};
use crate::compose::Item;
use crate::provider::CodeSystemProvider;
use crate::valueset::convert;
use crate::valueset::model::ValueSetModel;
use crate::valueset::store::Resolver;
use crate::versioned::Versioned;

/// The `tx-issue-type` code system
/// (<https://build.fhir.org/ig/FHIR/fhir-tools-ig/CodeSystem-tx-issue-type.html>).
pub const TX_ISSUE_TYPE: &str = "http://hl7.org/fhir/tools/CodeSystem/tx-issue-type";

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
    pub expression: Option<&'static str>,
}

/// The outcome of a validation: the R4B response and the ecosystem outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validation {
    /// `result`, `message`, and `display`.
    pub response: ValueSetValidateCodeResponse,
    /// The system the code was checked in, when one was determined.
    pub system: Option<String>,
    /// The served version of that system.
    pub version: Option<String>,
    /// The code as the system spells it, when the code is known.
    pub code: Option<String>,
    /// The issues behind the result.
    pub issues: Vec<Issue>,
}

/// The code under validation, from whichever parameter carried it.
struct Subject<'a> {
    system: Option<&'a str>,
    version: Option<&'a str>,
    code: &'a str,
    display: Option<&'a str>,
    expression: &'static str,
}

/// Runs `$validate-code`.
///
/// # Errors
///
/// Returns [`OperationError`] when the validation cannot be performed: no or
/// several code inputs, no value set, an unsupported parameter, an unknown or
/// invalid value set, or a provider failure.
pub fn validate_code(
    sources: &Sources<'_>,
    request: &ValueSetValidateCodeRequest,
) -> Result<Validation, OperationError> {
    if request.context.is_some() {
        return Err(OperationError::NotSupported(String::from(
            "`context` is not supported; name the value set with `url` or `valueSet`",
        )));
    }
    if request.date.is_some() {
        return Err(OperationError::NotSupported(String::from(
            "`date` is not supported: codes are validated against the versions served now",
        )));
    }
    let model = sources.value_set(
        request.value_set.as_ref().map(convert::r4b::convert),
        uri_text(request.url.as_ref()),
        string_text(request.value_set_version.as_ref()),
    )?;
    let language = code_text(request.display_language.as_ref());
    let resolver = Resolver::new(sources.registry, sources.value_sets);
    let abstract_ok = request
        .r#abstract
        .as_ref()
        .and_then(|b| b.value)
        .unwrap_or(true);
    let inputs = usize::from(request.code.is_some())
        + usize::from(request.coding.is_some())
        + usize::from(request.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    if let Some(code) = code_text(request.code.as_ref()) {
        let subject = Subject {
            system: uri_text(request.system.as_ref()),
            version: string_text(request.system_version.as_ref()),
            code,
            display: string_text(request.display.as_ref()),
            expression: "code",
        };
        return check(sources, &model, &resolver, &subject, language, abstract_ok);
    }
    if let Some(coding) = &request.coding {
        let (system, version, code, display) = coding_parts(coding);
        let code = code
            .ok_or_else(|| OperationError::Required(String::from("`coding.code` is required")))?;
        let subject = Subject {
            system: system.or(uri_text(request.system.as_ref())),
            version: version.or(string_text(request.system_version.as_ref())),
            code,
            display: display.or(string_text(request.display.as_ref())),
            expression: "coding",
        };
        return check(sources, &model, &resolver, &subject, language, abstract_ok);
    }
    let concept = request
        .codeable_concept
        .as_ref()
        .ok_or_else(|| OperationError::Invalid(String::from("no code input")))?;
    if concept.coding.is_empty() {
        return Err(OperationError::Required(String::from(
            "`codeableConcept` carries no `coding`",
        )));
    }
    // NOTE: a CodeableConcept validates when any of its codings is in the value
    // set (<https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).
    let mut last = None;
    for coding in &concept.coding {
        let (system, version, code, display) = coding_parts(coding);
        let Some(code) = code else { continue };
        let subject = Subject {
            system,
            version,
            code,
            display,
            expression: "codeableConcept",
        };
        let validation = check(sources, &model, &resolver, &subject, language, abstract_ok)?;
        if validation.response.result.value == Some(true) {
            return Ok(validation);
        }
        last = Some(validation);
    }
    last.ok_or_else(|| {
        OperationError::Required(String::from("`codeableConcept.coding.code` is required"))
    })
}

/// Validates one subject against the value set.
fn check(
    sources: &Sources<'_>,
    model: &ValueSetModel,
    resolver: &Resolver<'_>,
    subject: &Subject<'_>,
    language: Option<&str>,
    abstract_ok: bool,
) -> Result<Validation, OperationError> {
    let Some(system) = subject
        .system
        .map(str::to_owned)
        .or_else(|| infer_system(model))
    else {
        return Ok(failed(
            None,
            None,
            Issue {
                severity: "error",
                code: "invalid",
                kind: "cannot-infer",
                text: format!(
                    "the code `{}` names no system, and the value set `{}` draws on more than one",
                    subject.code,
                    model.canonical()
                ),
                expression: Some(subject.expression),
            },
        ));
    };
    let served = match sources.registry.resolve(&system, subject.version) {
        Ok(served) => served,
        Err(error) => {
            return Ok(failed(
                Some(system.clone()),
                None,
                Issue {
                    severity: "error",
                    code: "not-found",
                    kind: "not-found",
                    text: error.to_string(),
                    expression: Some(subject.expression),
                },
            ));
        }
    };
    let provider: &Arc<dyn CodeSystemProvider> = &served.provider;
    let version = provider.identity().version.clone();
    let Some(located) = provider.locate(subject.code)? else {
        return Ok(unknown_code(model, &system, version, subject));
    };
    let Some(item) = resolver.contains_compose(
        &model.canonical(),
        &model.compose,
        &system,
        subject.version,
        &located.code,
        language,
    )?
    else {
        let display = provider.display(located.concept, language)?;
        return Ok(outside_value_set(
            model,
            &system,
            version,
            &located,
            display.as_deref(),
            subject.expression,
        ));
    };
    let issues = assess(provider, &located, &item, subject, language, abstract_ok)?;
    let display = provider.display(located.concept, language)?;
    let result = !issues.iter().any(|issue| issue.severity == "error");
    let message = issues
        .iter()
        .map(|issue| issue.text.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    Ok(Validation {
        response: ValueSetValidateCodeResponse {
            result: result.into(),
            message: (!message.is_empty()).then(|| message.as_str().into()),
            display: display.as_deref().map(Into::into),
        },
        system: Some(system),
        version: Some(version).filter(|v| !v.is_empty()),
        code: Some(located.code),
        issues,
    })
}

/// The issues of a code the value set contains: abstract, inactive, and the
/// display check.
fn assess(
    provider: &Arc<dyn CodeSystemProvider>,
    located: &crate::provider::Located,
    item: &Item,
    subject: &Subject<'_>,
    language: Option<&str>,
    abstract_ok: bool,
) -> Result<Vec<Issue>, OperationError> {
    let mut issues = Vec::new();
    if item.abstract_concept && !abstract_ok {
        issues.push(Issue {
            severity: "error",
            code: "business-rule",
            kind: "code-rule",
            text: format!(
                "the code `{}` is abstract and cannot be selected",
                located.code
            ),
            expression: Some(subject.expression),
        });
    }
    if item.inactive {
        issues.push(Issue {
            severity: "warning",
            code: "business-rule",
            kind: "status-check",
            text: format!(
                "the code `{}` is inactive in `{}` version `{}`",
                located.code, item.system, item.version
            ),
            expression: None,
        });
    }
    let display = provider.display(located.concept, language)?;
    if let Some(given) = subject.display
        && !display_matches(provider, located.concept, given, display.as_deref())?
    {
        let text = match &display {
            Some(display) => format!(
                "the display `{given}` is not a valid display for `{}#{}`; the display is `{display}`",
                item.system, located.code
            ),
            None => format!(
                "the display `{given}` is not a valid display for `{}#{}`",
                item.system, located.code
            ),
        };
        issues.push(Issue {
            severity: "error",
            code: "invalid",
            kind: "invalid-display",
            text,
            expression: Some(subject.expression),
        });
    }
    Ok(issues)
}

/// Whether `given` is the display or one of the designations of `concept`,
/// compared without case and with whitespace collapsed.
fn display_matches(
    provider: &Arc<dyn CodeSystemProvider>,
    concept: crate::provider::Concept,
    given: &str,
    display: Option<&str>,
) -> Result<bool, OperationError> {
    let fold = |text: &str| {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let wanted = fold(given);
    if display.is_some_and(|d| fold(d) == wanted) {
        return Ok(true);
    }
    Ok(provider
        .designations(concept, None)?
        .iter()
        .any(|d| fold(&d.value) == wanted))
}

/// The one system a value set draws on, when there is exactly one.
fn infer_system(model: &ValueSetModel) -> Option<String> {
    let mut systems: Vec<&str> = model
        .compose
        .include
        .iter()
        .filter_map(|include| include.system.as_ref().map(|s| s.url.as_str()))
        .collect();
    systems.sort_unstable();
    systems.dedup();
    match systems.as_slice() {
        [one] => Some((*one).to_owned()),
        _ => None,
    }
}

fn system_code(system: &str, code: &str) -> String {
    format!("{system}#{code}")
}

/// The issue for a code the value set does not contain.
fn not_in_vs(model: &ValueSetModel, system_code: &str, expression: &'static str) -> Issue {
    Issue {
        severity: "error",
        code: "code-invalid",
        kind: "not-in-vs",
        text: format!(
            "the code `{system_code}` is not in the value set `{}`",
            model.canonical()
        ),
        expression: Some(expression),
    }
}

/// A `result = false` validation carrying `issue`.
fn failed(system: Option<String>, version: Option<String>, issue: Issue) -> Validation {
    let version = version.filter(|v| !v.is_empty());
    Validation {
        response: ValueSetValidateCodeResponse {
            result: false.into(),
            message: Some(issue.text.as_str().into()),
            display: None,
        },
        system,
        version,
        code: None,
        issues: vec![issue],
    }
}

/// The R4B `Coding` of a `tx-issue-type` code, for `issue.details.coding`.
#[must_use]
pub fn tx_issue_coding(kind: &str) -> Coding {
    Coding {
        system: Some(TX_ISSUE_TYPE.into()),
        code: Some(kind.into()),
        ..Default::default()
    }
}

/// The failed validation of a code the system does not have: not in the value
/// set, and unknown in the system.
fn unknown_code(
    model: &ValueSetModel,
    system: &str,
    version: String,
    subject: &Subject<'_>,
) -> Validation {
    let unknown = Issue {
        severity: "error",
        code: "code-invalid",
        kind: "invalid-code",
        text: format!(
            "unknown code `{}` in the code system `{system}` version `{version}`",
            subject.code
        ),
        expression: Some(subject.expression),
    };
    let mut validation = failed(
        Some(system.to_owned()),
        Some(version),
        not_in_vs(
            model,
            &system_code(system, subject.code),
            subject.expression,
        ),
    );
    validation.response.message = Some(unknown.text.as_str().into());
    validation.issues.push(unknown);
    // NOTE: the submitted code is echoed even when the system does not have it, the
    // shape the ecosystem expects (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/>).
    validation.code = Some(subject.code.to_owned());
    validation
}

/// The failed validation of a code the system has but the value set does
/// not: the code and its display are still echoed.
fn outside_value_set(
    model: &ValueSetModel,
    system: &str,
    version: String,
    located: &crate::provider::Located,
    display: Option<&str>,
    expression: &'static str,
) -> Validation {
    let mut validation = failed(
        Some(system.to_owned()),
        Some(version),
        not_in_vs(model, &system_code(system, &located.code), expression),
    );
    validation.code = Some(located.code.clone());
    validation.response.display = display.map(Into::into);
    validation
}
