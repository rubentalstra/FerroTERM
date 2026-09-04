//! The judgement of a display a client asserts, in the terminology
//! ecosystem's words.
//!
//! No FHIR version fixes the wording of a display mismatch; the ecosystem's
//! test cases do, and a validator shows them to its user, so the texts follow
//! the reference server's (spec-silent, recorded on #183).

use std::sync::Arc;

use super::Issue;
use crate::language;
use crate::provider::{CodeSystemProvider, Concept, ContentMode, ProviderError};
use crate::text_match;

/// One display a concept accepts, with the language it is in.
struct Candidate {
    text: String,
    language: Option<String>,
}

impl Candidate {
    /// `'text' (lang)`, the tag only when the display has a language.
    fn quoted(&self) -> String {
        match &self.language {
            Some(lang) => format!("'{}' ({lang})", self.text),
            None => format!("'{}'", self.text),
        }
    }
}

/// Compares displays without case and with whitespace collapsed.
fn fold(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The displays `concept` accepts: its display in the system's language and
/// every designation in its own.
fn candidates(
    provider: &Arc<dyn CodeSystemProvider>,
    concept: Concept,
) -> Result<Vec<Candidate>, ProviderError> {
    let mut out: Vec<Candidate> = Vec::new();
    if let Some(display) = provider.display(concept, None)? {
        out.push(Candidate {
            text: display,
            language: provider.language().map(str::to_owned),
        });
    }
    for designation in provider.designations(concept, None)? {
        if designation.standards_status.is_some() {
            continue;
        }
        let duplicate = out
            .iter()
            .any(|c| c.text == designation.value && c.language == designation.language);
        if !duplicate {
            out.push(Candidate {
                text: designation.value,
                language: designation.language,
            });
        }
    }
    Ok(out)
}

/// `Valid display is 'a' (en)`, or `one of N choices: 'a' (en) or 'b' (de)`.
fn valid_display(list: &[&Candidate]) -> String {
    match list {
        [one] => format!("Valid display is {}", one.quoted()),
        many => {
            let quoted: Vec<String> = many.iter().map(|c| c.quoted()).collect();
            let (leading, final_one) = quoted.split_at(quoted.len() - 1);
            format!(
                "Valid display is one of {} choices: {} or {}",
                many.len(),
                leading.join(", "),
                final_one.join("")
            )
        }
    }
}

/// A display the client asserted, and how to judge it.
#[derive(Debug, Clone, Copy)]
pub struct Asserted<'a> {
    /// The code system URI, for the text.
    pub system: &'a str,
    /// The code as the system spells it, for the text.
    pub code: &'a str,
    /// The display the client sent.
    pub given: &'a str,
    /// The `displayLanguage` as the client sent it.
    pub requested: Option<&'a str>,
    /// Whether a wrong display is a warning (`lenient-display-validation`).
    pub lenient: bool,
}

/// Judges an asserted display.
///
/// `None` when the display is one the concept accepts in the requested
/// language, else the issue to raise at `expression`: an `information` when
/// it is valid in the default language alone.
///
/// # Errors
///
/// Returns the provider's error when the displays cannot be read.
pub fn judge(
    provider: &Arc<dyn CodeSystemProvider>,
    concept: Concept,
    asserted: Asserted<'_>,
    expression: Option<String>,
) -> Result<Option<Issue>, ProviderError> {
    let Some((severity, kind, text)) = judgement(provider, concept, asserted)? else {
        return Ok(None);
    };
    Ok(Some(Issue {
        severity,
        code: "invalid",
        kind,
        text,
        expression,
    }))
}

/// The `display-comment` for a display that matches a designation the system
/// has withdrawn or deprecated (the ecosystem's `INACTIVE_DISPLAY_FOUND`):
/// a warning under lenient validation, an error otherwise.
fn retired_designation(
    provider: &Arc<dyn CodeSystemProvider>,
    concept: Concept,
    asserted: Asserted<'_>,
) -> Result<Option<(&'static str, &'static str, String)>, ProviderError> {
    let wanted = fold(asserted.given);
    let retired = provider
        .designations(concept, None)?
        .into_iter()
        .find(|d| d.standards_status.is_some() && fold(&d.value) == wanted);
    let Some(retired) = retired else {
        return Ok(None);
    };
    let preferred = provider
        .display(
            concept,
            language::for_provider(provider.as_ref(), asserted.requested).as_deref(),
        )?
        .unwrap_or_default();
    let severity = if asserted.lenient { "warning" } else { "error" };
    Ok(Some((
        severity,
        "display-comment",
        format!(
            "'{}' is no longer considered a correct display for code '{}' (status = {}). The correct display is '{preferred}'",
            asserted.given,
            asserted.code,
            retired.standards_status.unwrap_or_default()
        ),
    )))
}

/// The severity and text of a wrong display, `None` for a right one.
fn judgement(
    provider: &Arc<dyn CodeSystemProvider>,
    concept: Concept,
    asserted: Asserted<'_>,
) -> Result<Option<(&'static str, &'static str, String)>, ProviderError> {
    if let Some(retired) = retired_designation(provider, concept, asserted)? {
        return Ok(Some(retired));
    }
    let Asserted {
        system,
        code,
        given,
        requested,
        lenient,
    } = asserted;
    let all = candidates(provider, concept)?;
    let wanted = fold(given);
    let matches = |c: &&Candidate| fold(&c.text) == wanted;
    // NOTE: a display that differs only in its whitespace is still wrong, and
    // said so (the ecosystem's `bad-display-ws` case).
    let exact = |c: &&Candidate| c.text.to_lowercase() == given.to_lowercase();
    let whitespace = |valid: &[&Candidate]| {
        format!(
            "Wrong Display Name '{given}' for {system}#{code}: the whitespace differs. {}",
            valid_display(valid)
        )
    };
    let severity = if lenient { "warning" } else { "error" };
    let Some(requested) = requested else {
        if all.iter().any(|c| exact(&c)) {
            return Ok(None);
        }
        let valid: Vec<&Candidate> = all.iter().collect();
        if all.iter().any(|c| matches(&c)) {
            return Ok(Some((severity, "invalid-display", whitespace(&valid))));
        }
        return Ok(Some((
            severity,
            "invalid-display",
            format!(
                "Wrong Display Name '{given}' for {system}#{code}. {} (for the language(s) '--')",
                valid_display(&valid)
            ),
        )));
    };
    // NOTE: `displayLanguage` is a BCP 47 range list (`de,it,zh`); a display in any
    // of its languages counts.
    let wanted_languages: Vec<String> = language::ranges(requested)
        .into_iter()
        .map(|range| range.tag)
        .collect();
    let in_language: Vec<&Candidate> = all
        .iter()
        .filter(|c| {
            c.language.as_deref().is_none_or(|l| {
                wanted_languages
                    .iter()
                    .any(|w| w == "*" || text_match::same_language(l, w))
            })
        })
        .collect();
    if !in_language.is_empty() {
        if in_language.iter().any(exact) {
            return Ok(None);
        }
        if in_language.iter().any(matches) {
            return Ok(Some((
                severity,
                "invalid-display",
                whitespace(&in_language),
            )));
        }
        return Ok(Some((
            severity,
            "invalid-display",
            format!(
                "Wrong Display Name '{given}' for {system}#{code}. {} (for the language(s) '{requested}')",
                valid_display(&in_language)
            ),
        )));
    }
    if all.iter().any(|c| matches(&c)) {
        return Ok(Some((
            "information",
            "invalid-display",
            format!(
                "There are no valid display names found for the code {system}#{code} for language(s) '{requested}'. The display is '{given}' which is a valid display for the default language"
            ),
        )));
    }
    let default = provider
        .display(
            concept,
            language::for_provider(provider.as_ref(), None).as_deref(),
        )?
        .unwrap_or_default();
    Ok(Some((
        severity,
        "invalid-display",
        format!(
            "Wrong Display Name '{given}' for {system}#{code}. There are no valid display names found for language(s) '{requested}'. Default display is '{default}'"
        ),
    )))
}

/// The `code-rule` warning for a code a case-insensitive system located under
/// another spelling; `None` when the spelling matches or the system is
/// case-sensitive.
#[must_use]
pub fn case_note(
    provider: &Arc<dyn CodeSystemProvider>,
    given: &str,
    located: &str,
    expression: Option<String>,
) -> Option<Issue> {
    if given == located {
        return None;
    }
    if given.to_lowercase() != located.to_lowercase() {
        // NOTE: an alternate form the system admits (the ecosystem's icd-11
        // `cs-validate-uri`: a URI for a code) is normalized with an information note.
        return Some(Issue {
            severity: "information",
            code: "business-rule",
            kind: "code-rule",
            text: format!(
                "The code '{given}' is an alternate form of '{located}', the code as the code system '{}' spells it",
                provider.identity().url
            ),
            expression,
        });
    }
    if provider.declaration().case_sensitive {
        return None;
    }
    Some(Issue {
        severity: "warning",
        code: "business-rule",
        kind: "code-rule",
        text: format!(
            "The code '{given}' differs from the correct code '{located}' by case. Although the code system '{}' is case insensitive, implementers are strongly encouraged to use the correct case anyway",
            provider.identity().url
        ),
        expression,
    })
}

/// The `invalid-code` text for a code the system does not have, naming the
/// version when the system has one and the fragment caveat when it is one.
#[must_use]
pub fn unknown_code_text(provider: &dyn CodeSystemProvider, code: &str) -> String {
    let identity = provider.identity();
    let version = if identity.version.is_empty() {
        String::new()
    } else {
        format!(" version '{}'", identity.version)
    };
    if matches!(provider.declaration().content, ContentMode::Fragment) {
        format!(
            "Unknown Code '{code}' in the CodeSystem '{}'{version} - note that the code system is labeled as a fragment, so the code may be valid in some other fragment",
            identity.url
        )
    } else {
        format!(
            "Unknown code '{code}' in the CodeSystem '{}'{version}",
            identity.url
        )
    }
}
