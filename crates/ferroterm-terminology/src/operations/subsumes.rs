//! `CodeSystem/$subsumes` on R4B
//! (<https://hl7.org/fhir/R4B/codesystem-operation-subsumes.html>).
//!
//! `codeA` and `codeB` with a `system` (or the instance), or `codingA` and
//! `codingB`. The outcome is `equivalent`, `subsumes`, `subsumed-by`, or
//! `not-subsumed`, A relative to B. When the relationship cannot be
//! determined (an unknown code, a system without a hierarchy, codings from a
//! system other than the one tested) the answer is an error, never
//! `not-subsumed`.

use ferroterm_fhir::r4b::operations::code_system_subsumes::{
    CodeSystemSubsumesRequest, CodeSystemSubsumesResponse,
};

use super::{
    Invocation, OperationError, code_text, coding_parts, locate, resolve, string_text, uri_text,
};
use crate::registry::Registry;

/// Runs `$subsumes`.
///
/// # Errors
///
/// Returns [`OperationError`] for missing or mixed inputs, a missing system,
/// codings from another system, an unknown system, version, or code, a
/// system without subsumption, or a provider failure.
pub fn subsumes(
    registry: &Registry,
    invocation: &Invocation,
    request: &CodeSystemSubsumesRequest,
) -> Result<CodeSystemSubsumesResponse, OperationError> {
    let codes = (
        code_text(request.code_a.as_ref()),
        code_text(request.code_b.as_ref()),
    );
    let codings = (&request.coding_a, &request.coding_b);
    let mut system = uri_text(request.system.as_ref());
    let mut version = string_text(request.version.as_ref());
    let (a, b) = match (codes, codings) {
        ((Some(a), Some(b)), (None, None)) => (a, b),
        ((None, None), (Some(coding_a), Some(coding_b))) => {
            let (system_a, version_a, a, _) = coding_parts(coding_a);
            let (system_b, version_b, b, _) = coding_parts(coding_b);
            // NOTE: codings from a system other than the one tested need
            // "well established" relationships between the systems, which the
            // server does not have; the definition says to return an error.
            for other in [system_a, system_b].into_iter().flatten() {
                match system {
                    Some(tested) if tested != other => {
                        return Err(OperationError::NotSupported(format!(
                            "coding system `{other}` differs from the tested system `{tested}`"
                        )));
                    }
                    Some(_) => {}
                    None => system = Some(other),
                }
            }
            if let (Some(va), Some(vb)) = (version_a, version_b)
                && va != vb
            {
                return Err(OperationError::Invalid(String::from(
                    "`codingA` and `codingB` name different versions",
                )));
            }
            version = version.or(version_a).or(version_b);
            let a = a.ok_or_else(|| {
                OperationError::Required(String::from("`codingA.code` is required"))
            })?;
            let b = b.ok_or_else(|| {
                OperationError::Required(String::from("`codingB.code` is required"))
            })?;
            (a, b)
        }
        ((None, None), (None, None)) => {
            return Err(OperationError::Required(String::from(
                "provide `codeA` and `codeB`, or `codingA` and `codingB`",
            )));
        }
        _ => {
            return Err(OperationError::Invalid(String::from(
                "provide both codes as `codeA` and `codeB`, or both as `codingA` and `codingB`",
            )));
        }
    };
    let resolved = resolve(registry, invocation, system, version)?;
    let provider = &resolved.provider;
    let a = locate(provider, a)?.concept;
    let b = locate(provider, b)?.concept;
    if let Some(outcome) = provider.subsumes(a, b)? {
        return Ok(CodeSystemSubsumesResponse {
            outcome: outcome.code().into(),
        });
    }
    let hierarchy = provider.hierarchy().ok_or_else(|| {
        OperationError::NotSupported(format!(
            "code system `{}` declares no subsumption",
            provider.identity().url
        ))
    })?;
    Ok(CodeSystemSubsumesResponse {
        outcome: hierarchy.subsumes(a, b).code().into(),
    })
}
