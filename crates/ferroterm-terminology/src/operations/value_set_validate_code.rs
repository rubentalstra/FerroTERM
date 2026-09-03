//! `ValueSet/$validate-code` in the terms every served FHIR version shares.
//!
//! The operation pages: <https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>
//! and <https://hl7.org/fhir/R5/valueset-operation-validate-code.html>.
//!
//! The value set is inline, stored, or implicit, and is expanded whole; the
//! code is one of `code` with `system`, `coding`, or `codeableConcept`. A code
//! outside the value set is `result = false` with a message, never an error.
//! Beside `result`, `message`, and `display`, the validation carries the
//! `system`, `version`, and `code` echo and the `issues` a general-purpose
//! server of the terminology ecosystem returns (`tx-issue-type` codings); the
//! wire layer of each version emits what that version declares.

use std::sync::Arc;

use super::{CodingRef, Issue, OperationError, Sources};
use crate::compose::Item;
use crate::language;
use crate::provider::CodeSystemProvider;
use crate::valueset::model::{ModelError, ValueSetModel};
use crate::valueset::store::Resolver;
use crate::versioned::Versioned;

/// The `tx-issue-type` code system
/// (<https://build.fhir.org/ig/FHIR/fhir-tools-ig/CodeSystem-tx-issue-type.html>).
pub const TX_ISSUE_TYPE: &str = "http://hl7.org/fhir/tools/CodeSystem/tx-issue-type";

/// The outcome of a validation, in no version's types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validation {
    /// Whether the code is in the value set (and its display, when given, is valid).
    pub result: bool,
    /// Why the result is what it is, when there is something to say.
    pub message: Option<String>,
    /// The display the system prefers, when the code was found.
    pub display: Option<String>,
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
/// The input of `ValueSet/$validate-code`: the union of the parameters the
/// served versions declare.
#[derive(Debug, Default)]
pub struct ValueSetValidateInput {
    /// The value set URL (`url`).
    pub url: Option<String>,
    /// The value set version (`valueSetVersion`).
    pub value_set_version: Option<String>,
    /// The inline `valueSet`, converted by the wire layer of its version.
    pub inline_value_set: Option<Result<ValueSetModel, ModelError>>,
    /// `useSupplement` (R5): the supplements named; every supplied supplement
    /// is layered on its system already, so the names are accepted as is.
    pub use_supplement: Vec<String>,
    /// Whether `context` was given; not supported.
    pub context: bool,
    /// Whether `date` was given; not supported.
    pub date: bool,
    /// The code.
    pub code: Option<String>,
    /// The code system URI (`system`).
    pub system: Option<String>,
    /// The code system version (`systemVersion`).
    pub system_version: Option<String>,
    /// The display the client asserts.
    pub display: Option<String>,
    /// The coding, instead of `code`.
    pub coding: Option<CodingRef>,
    /// The codings of a `codeableConcept`, instead of `code`.
    pub codeable_concept: Option<Vec<CodingRef>>,
    /// The language of the display (a BCP 47 range list).
    pub display_language: Option<String>,
    /// `abstract`: whether an abstract code may be selected; the default is true.
    pub abstract_ok: Option<bool>,
}

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
    input: &ValueSetValidateInput,
) -> Result<Validation, OperationError> {
    if input.context {
        return Err(OperationError::NotSupported(String::from(
            "`context` is not supported; name the value set with `url` or `valueSet`",
        )));
    }
    if input.date {
        return Err(OperationError::NotSupported(String::from(
            "`date` is not supported: codes are validated against the versions served now",
        )));
    }
    let model = sources.value_set(
        input.inline_value_set.clone(),
        input.url.as_deref(),
        input.value_set_version.as_deref(),
    )?;
    let language = input.display_language.as_deref();
    let resolver = Resolver::new(sources.registry, sources.value_sets);
    let abstract_ok = input.abstract_ok.unwrap_or(true);
    let inputs = usize::from(input.code.is_some())
        + usize::from(input.coding.is_some())
        + usize::from(input.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    if let Some(code) = input.code.as_deref() {
        let subject = Subject {
            system: input.system.as_deref(),
            version: input.system_version.as_deref(),
            code,
            display: input.display.as_deref(),
            expression: "code",
        };
        return check(sources, &model, &resolver, &subject, language, abstract_ok);
    }
    if let Some(coding) = &input.coding {
        let code = coding
            .code
            .as_deref()
            .ok_or_else(|| OperationError::Required(String::from("`coding.code` is required")))?;
        let subject = Subject {
            system: coding.system.as_deref().or(input.system.as_deref()),
            version: coding
                .version
                .as_deref()
                .or(input.system_version.as_deref()),
            code,
            display: coding.display.as_deref().or(input.display.as_deref()),
            expression: "coding",
        };
        return check(sources, &model, &resolver, &subject, language, abstract_ok);
    }
    let codings = input
        .codeable_concept
        .as_ref()
        .ok_or_else(|| OperationError::Invalid(String::from("no code input")))?;
    if codings.is_empty() {
        return Err(OperationError::Required(String::from(
            "`codeableConcept` carries no `coding`",
        )));
    }
    // NOTE: a CodeableConcept validates when any of its codings is in the value
    // set (<https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).
    let mut last = None;
    for coding in codings {
        let Some(code) = coding.code.as_deref() else {
            continue;
        };
        let subject = Subject {
            system: coding.system.as_deref(),
            version: coding.version.as_deref(),
            code,
            display: coding.display.as_deref(),
            expression: "codeableConcept",
        };
        let validation = check(sources, &model, &resolver, &subject, language, abstract_ok)?;
        if validation.result {
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
        let display = provider.display(
            located.concept,
            language::for_provider(provider.as_ref(), language).as_deref(),
        )?;
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
    let display = provider.display(
        located.concept,
        language::for_provider(provider.as_ref(), language).as_deref(),
    )?;
    let result = !issues.iter().any(|issue| issue.severity == "error");
    let message = issues
        .iter()
        .map(|issue| issue.text.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    Ok(Validation {
        result,
        message: (!message.is_empty()).then_some(message),
        display,
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
    let display = provider.display(
        located.concept,
        language::for_provider(provider.as_ref(), language).as_deref(),
    )?;
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
        result: false,
        message: Some(issue.text.clone()),
        display: None,
        system,
        version,
        code: None,
        issues: vec![issue],
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
    validation.message = Some(unknown.text.clone());
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
    validation.display = display.map(str::to_owned);
    validation
}
