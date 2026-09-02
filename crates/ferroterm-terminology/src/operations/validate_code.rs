//! `CodeSystem/$validate-code` on R4B
//! (<https://hl7.org/fhir/R4B/codesystem-operation-validate-code.html>).
//!
//! One and only one of `code`, `coding`, `codeableConcept`. The system is the
//! instance, or `url`; R4B declares no `system` parameter (the generated
//! request refuses one). An invalid code is `result = false` with a message,
//! never an error; only an undeterminable validation is an error. A wrong
//! `display` is `result = false` with the correct `display`, the R4B example's
//! shape. An inactive code validates with a message (spec-silent in R4B;
//! `.claude/rules/fhir-terminology.md` F-VAL-4).

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::code_system_validate_code::{
    CodeSystemValidateCodeRequest, CodeSystemValidateCodeResponse,
};

use super::{Invocation, OperationError, code_text, coding_parts, resolve, string_text, uri_text};
use crate::provider::CodeSystemProvider;
use crate::registry::Registry;

/// Runs `$validate-code`.
///
/// # Errors
///
/// Returns [`OperationError`] when the validation cannot be performed: no or
/// several code inputs, no system, an inline `codeSystem`, an unknown system
/// or version, or a provider failure.
pub fn validate_code(
    registry: &Registry,
    invocation: &Invocation,
    request: &CodeSystemValidateCodeRequest,
) -> Result<CodeSystemValidateCodeResponse, OperationError> {
    if request.code_system.is_some() {
        return Err(OperationError::NotSupported(String::from(
            "validating against an inline `codeSystem` resource is not supported; name a served system with `url`",
        )));
    }
    let inputs = usize::from(request.code.is_some())
        + usize::from(request.coding.is_some())
        + usize::from(request.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    let url = uri_text(request.url.as_ref());
    let version = string_text(request.version.as_ref());
    let language = code_text(request.display_language.as_ref());
    if let Some(code) = code_text(request.code.as_ref()) {
        let resolved = resolve(registry, invocation, url, version)?;
        return check(
            &resolved.provider,
            code,
            string_text(request.display.as_ref()),
            language,
        );
    }
    if let Some(coding) = &request.coding {
        let (system, coding_version, code, display) = coding_parts(coding);
        if let (Some(url), Some(system)) = (url, system)
            && url != system
        {
            return Err(OperationError::Invalid(format!(
                "`coding.system` `{system}` does not match `url` `{url}`"
            )));
        }
        let code = code
            .ok_or_else(|| OperationError::Required(String::from("`coding.code` is required")))?;
        let resolved = resolve(
            registry,
            invocation,
            url.or(system),
            coding_version.or(version),
        )?;
        return check(
            &resolved.provider,
            code,
            display.or(string_text(request.display.as_ref())),
            language,
        );
    }
    let Some(concept) = &request.codeable_concept else {
        return Err(OperationError::Required(String::from(
            "provide one of `code`, `coding`, or `codeableConcept`",
        )));
    };
    // The concept validates when one of its codings is in the code system
    // (the definition's rule); codings of other systems are skipped.
    let resolved = resolve(registry, invocation, url, version)?;
    let target = resolved.provider.identity().url.clone();
    let mut messages = Vec::new();
    let mut any = false;
    for coding in &concept.coding {
        let (system, _, code, display) = coding_parts(coding);
        if system.is_some_and(|s| s != target) {
            continue;
        }
        let Some(code) = code else {
            continue;
        };
        any = true;
        let response = check(&resolved.provider, code, display, language)?;
        if response.result.value == Some(true) {
            return Ok(response);
        }
        if let Some(message) = string_text(response.message.as_ref()) {
            messages.push(message.to_owned());
        }
    }
    let message = if any {
        messages.join("; ")
    } else {
        format!("no coding of the CodeableConcept is in code system `{target}`")
    };
    Ok(CodeSystemValidateCodeResponse {
        result: false.into(),
        message: Some(message.into()),
        display: None,
    })
}

fn check(
    provider: &std::sync::Arc<dyn CodeSystemProvider>,
    code: &str,
    display: Option<&str>,
    language: Option<&str>,
) -> Result<CodeSystemValidateCodeResponse, OperationError> {
    let identity = provider.identity();
    let Some(located) = provider.locate(code)? else {
        return Ok(CodeSystemValidateCodeResponse {
            result: false.into(),
            message: Some(
                format!(
                    "code `{code}` is not in code system `{}` version `{}`",
                    identity.url, identity.version
                )
                .into(),
            ),
            display: None,
        });
    };
    let concept = located.concept;
    let preferred = provider.display(concept, language)?;
    let mut messages = Vec::new();
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
            messages.push(format!(
                "the display `{display}` is not a designation of `{code}`"
            ));
        }
    }
    let status = provider.status(concept)?;
    if !status.active {
        messages.push(format!("code `{code}` is inactive"));
    }
    Ok(CodeSystemValidateCodeResponse {
        result: result.into(),
        message: (!messages.is_empty()).then(|| messages.join("; ").into()),
        display: preferred.map(std::convert::Into::into),
    })
}

/// A `Coding` for the code system and code, for callers building outcomes.
#[must_use]
pub fn coding(system: &str, code: &str, display: Option<&str>) -> Coding {
    Coding {
        system: Some(system.into()),
        code: Some(code.into()),
        display: display.map(std::convert::Into::into),
        ..Default::default()
    }
}
