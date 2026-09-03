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
use crate::registry::ResolveError;
use crate::valueset::model::{ModelError, ValueSetModel};
use crate::valueset::negotiation::Negotiation;
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
    /// The canonicals of the systems the server does not serve
    /// (`x-caused-by-unknown-system`, the terminology ecosystem's output).
    pub unknown_systems: Vec<String>,
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
    /// `system-version`: the version to use for a system the value set does
    /// not pin, as `[system]|[version]` canonicals (pre-adopted from R6).
    pub default_system_version: Vec<String>,
    /// `check-system-version`: a version a differing value set is refused
    /// against (pre-adopted from R6).
    pub check_system_version: Vec<String>,
    /// `force-system-version`: a version that overrides the value set's
    /// (pre-adopted from R6).
    pub force_system_version: Vec<String>,
    /// `default-valueset-version`: the version to use for a value set
    /// reference that names none (pre-adopted from R6).
    pub default_valueset_version: Vec<String>,
    /// `check-valueset-version`: a value set version a differing reference is
    /// refused against (pre-adopted from R6).
    pub check_valueset_version: Vec<String>,
    /// `force-valueset-version`: a value set version that overrides the
    /// reference's (pre-adopted from R6).
    pub force_valueset_version: Vec<String>,
    /// `inferSystem`: find the system of a bare `code` in the value set
    /// (pre-adopted from R6).
    pub infer_system: Option<bool>,
    /// `lenient-display-validation`: a wrong display is a warning, not a
    /// failure (pre-adopted from R6).
    pub lenient_display_validation: Option<bool>,
}

/// The value set at its negotiated version with its systems pinned, and the
/// negotiation itself, for the subjects and the imports.
fn prepare(
    sources: &Sources<'_>,
    input: &ValueSetValidateInput,
) -> Result<(Arc<ValueSetModel>, Negotiation), OperationError> {
    let negotiation = Negotiation::new(
        &input.default_system_version,
        &input.check_system_version,
        &input.force_system_version,
        &input.default_valueset_version,
        &input.check_valueset_version,
        &input.force_valueset_version,
    );
    let (url, version) = match input.url.as_deref() {
        Some(url) => {
            let (url, version) = negotiation.value_set(url, input.value_set_version.as_deref())?;
            (Some(url), version)
        }
        None => (None, input.value_set_version.clone()),
    };
    let model = sources.value_set(
        input.inline_value_set.clone(),
        url.as_deref(),
        version.as_deref(),
    )?;
    let model = Arc::new(ValueSetModel {
        compose: negotiation.pin(&model.compose)?,
        ..(*model).clone()
    });
    Ok((model, negotiation))
}

/// How one validation judges what it finds.
#[derive(Debug, Clone, Copy)]
struct Policy<'a> {
    /// The language of the display (a BCP 47 range list).
    language: Option<&'a str>,
    /// `abstract`: whether an abstract code may be selected.
    abstract_ok: bool,
    /// `lenient-display-validation`: a wrong display is a warning, and the
    /// result stays true (pre-adopted from R6).
    lenient_display: bool,
    /// `inferSystem`: find the system of a bare code by its membership in the
    /// value set (pre-adopted from R6).
    infer_system: bool,
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
    let (model, negotiation) = prepare(sources, input)?;
    let resolver =
        Resolver::new(sources.registry, sources.value_sets).with_negotiation(&negotiation);
    let policy = Policy {
        language: input.display_language.as_deref(),
        abstract_ok: input.abstract_ok.unwrap_or(true),
        lenient_display: input.lenient_display_validation.unwrap_or(false),
        infer_system: input.infer_system.unwrap_or(false),
    };
    let check = |subject: &Subject<'_>| -> Result<Validation, OperationError> {
        let version =
            negotiation.system_version(subject.system.unwrap_or_default(), subject.version)?;
        let subject = Subject {
            version: version.as_deref(),
            ..*subject
        };
        check(sources, &model, &resolver, &subject, &policy)
    };
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
        return check(&subject);
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
        return check(&subject);
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
        let validation = check(&subject)?;
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
    policy: &Policy<'_>,
) -> Result<Validation, OperationError> {
    let language = policy.language;
    let system = if let Some(system) = subject.system {
        system.to_owned()
    } else {
        let inferred = if policy.infer_system {
            infer_by_membership(sources, model, resolver, subject, language)?
        } else {
            infer_system(model)
        };
        match inferred {
            Some(system) => system,
            None => return Ok(cannot_infer(model, subject)),
        }
    };
    let served = match sources.registry.resolve(&system, subject.version) {
        Ok(served) => served,
        Err(error) => {
            let version = match &error {
                ResolveError::UnknownSystem(_) => None,
                ResolveError::UnknownVersion { .. } => subject.version,
            };
            let (canonical, issue) = super::unknown_system(&system, version, subject.expression);
            let mut validation = failed(Some(system.clone()), version.map(str::to_owned), issue);
            validation.code = Some(subject.code.to_owned());
            validation.unknown_systems.push(canonical);
            return Ok(validation);
        }
    };
    let provider: &Arc<dyn CodeSystemProvider> = &served.provider;
    let version = provider.identity().version.clone();
    let Some(located) = provider.locate(subject.code)? else {
        return Ok(unknown_code(model, &system, version, subject));
    };
    let contained = match resolver.contains_compose(
        &model.canonical(),
        &model.compose,
        &system,
        subject.version,
        &located.code,
        language,
    ) {
        Ok(contained) => contained,
        // NOTE: a value set the compose imports but the server does not hold is a
        // failed validation naming it, the ecosystem's shape (its test cases);
        // an unknown top-level value set stays an error.
        Err(crate::compose::ComposeError::UnknownValueSet(url)) => {
            return Ok(unknown_import(
                &system,
                version,
                &url,
                &located.code,
                subject.expression,
            ));
        }
        Err(crate::compose::ComposeError::Resolve(error)) => {
            return unknown_include(
                provider,
                &located,
                version,
                &error,
                subject.expression,
                language,
            );
        }
        Err(error) => return Err(error.into()),
    };
    let Some(item) = contained else {
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
    let issues = assess(provider, &located, &item, subject, policy)?;
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
        unknown_systems: Vec::new(),
    })
}

/// The issues of a code the value set contains: abstract, inactive, and the
/// display check.
fn assess(
    provider: &Arc<dyn CodeSystemProvider>,
    located: &crate::provider::Located,
    item: &Item,
    subject: &Subject<'_>,
    policy: &Policy<'_>,
) -> Result<Vec<Issue>, OperationError> {
    let language = policy.language;
    let mut issues = Vec::new();
    if item.abstract_concept && !policy.abstract_ok {
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
        // NOTE: under `lenient-display-validation` a wrong display does not fail the
        // result (<https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-validate-code.html>).
        issues.push(Issue {
            severity: if policy.lenient_display {
                "warning"
            } else {
                "error"
            },
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

/// The one system of the value set whose code `subject.code` is in the value
/// set (`inferSystem`); `None` when none or several are.
fn infer_by_membership(
    sources: &Sources<'_>,
    model: &ValueSetModel,
    resolver: &Resolver<'_>,
    subject: &Subject<'_>,
    language: Option<&str>,
) -> Result<Option<String>, OperationError> {
    let mut systems: Vec<&str> = model
        .compose
        .include
        .iter()
        .filter_map(|include| include.system.as_ref().map(|s| s.url.as_str()))
        .collect();
    systems.sort_unstable();
    systems.dedup();
    let mut matches = Vec::new();
    for system in systems {
        let Ok(served) = sources.registry.resolve(system, None) else {
            continue;
        };
        let Some(located) = served.provider.locate(subject.code)? else {
            continue;
        };
        let contained = resolver.contains_compose(
            &model.canonical(),
            &model.compose,
            system,
            None,
            &located.code,
            language,
        );
        match contained {
            Ok(Some(_)) => matches.push(system.to_owned()),
            Ok(None) | Err(crate::compose::ComposeError::UnknownValueSet(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(match matches.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    })
}

/// The failed validation of a bare code whose system cannot be determined:
/// not in the value set, and the system unknown (the ecosystem's wording).
fn cannot_infer(model: &ValueSetModel, subject: &Subject<'_>) -> Validation {
    let text = format!(
        "The System URI could not be determined for the code '{}' in the ValueSet '{}'",
        subject.code,
        model.canonical()
    );
    let mut validation = failed(
        None,
        None,
        not_in_vs(model, &system_code("", subject.code), subject.expression),
    );
    validation.message = Some(text.clone());
    validation.issues.push(Issue {
        severity: "error",
        code: "not-found",
        kind: "cannot-infer",
        text,
        expression: Some(subject.expression),
    });
    validation.code = Some(subject.code.to_owned());
    validation
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
        unknown_systems: Vec::new(),
    }
}

/// The failed validation over a compose whose include names a system or
/// version the server does not serve: the include is invalid and the system is
/// named for the validator (the ecosystem's test cases; `$expand` keeps
/// refusing it as an error).
fn unknown_include(
    provider: &Arc<dyn CodeSystemProvider>,
    located: &crate::provider::Located,
    version: String,
    error: &ResolveError,
    expression: &'static str,
    language: Option<&str>,
) -> Result<Validation, OperationError> {
    let system = provider.identity().url.as_str();
    let display = provider.display(
        located.concept,
        language::for_provider(provider.as_ref(), language).as_deref(),
    )?;
    let (canonical, text) = match error {
        ResolveError::UnknownSystem(url) => (
            url.clone(),
            format!("The code system '{url}' in the ValueSet include is not known"),
        ),
        ResolveError::UnknownVersion { url, version } => (
            format!("{url}|{version}"),
            format!(
                "The code system '{url}' version '{version}' in the ValueSet include is not known"
            ),
        ),
    };
    let mut validation = failed(
        Some(system.to_owned()),
        Some(version),
        Issue {
            severity: "error",
            code: "invalid",
            kind: "vs-invalid",
            text,
            expression: Some(expression),
        },
    );
    let (url, bad_version) = match error {
        ResolveError::UnknownSystem(url) => (url.as_str(), None),
        ResolveError::UnknownVersion { url, version } => (url.as_str(), Some(version.as_str())),
    };
    let (_, not_found) = super::unknown_system(url, bad_version, expression);
    validation.message = Some(not_found.text.clone());
    validation.issues.push(not_found);
    validation.unknown_systems.push(canonical);
    validation.code = Some(located.code.clone());
    validation.display = display;
    Ok(validation)
}

/// The failed validation over a value set the compose imports but the server
/// does not hold: the ecosystem's shape (its test cases), where an unknown
/// top-level value set stays an error.
fn unknown_import(
    system: &str,
    version: String,
    url: &str,
    code: &str,
    expression: &'static str,
) -> Validation {
    let mut validation = failed(
        Some(system.to_owned()),
        Some(version),
        Issue {
            severity: "error",
            code: "not-found",
            kind: "not-found",
            text: format!("A definition for the value Set '{url}' could not be found"),
            expression: Some(expression),
        },
    );
    validation.code = Some(code.to_owned());
    validation
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
