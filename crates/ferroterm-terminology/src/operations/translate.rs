//! `ConceptMap/$translate` on R4B
//! (<https://hl7.org/fhir/R4B/conceptmap-operation-translate.html>).
//!
//! The map is inline, named by `url`, or chosen: every stored map whose
//! scopes fit `source` and `target` and whose groups map the code's system
//! (to `targetsystem` when given). One of `code` with `system`, `coding`, or
//! `codeableConcept` is the input; `reverse` reads the groups the other way.
//! An element without a target, or an `unmapped` rule, still yields a match,
//! so a client sees why nothing translated. `result` is true when a match has
//! a relationship other than `unmatched` or `disjoint`.

use std::sync::Arc;

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::concept_map_translate::{
    ConceptMapTranslateRequest, ConceptMapTranslateResponse, ConceptMapTranslateResponseMatch,
    ConceptMapTranslateResponseMatchProduct,
};

use super::{OperationError, Sources, code_text, coding_parts, string_text, uri_text};
use crate::conceptmap::convert;
use crate::conceptmap::model::{
    ConceptMapModel, DependsOn, Element, Group, Relationship, Target, UnmappedMode,
};
use crate::versioned::Versioned;

/// How deep `unmapped.mode = other-map` may chain (our own guard).
const OTHER_MAP_DEPTH: usize = 8;

/// What the ecosystem reports beside each R4B `match`: the map the match
/// came from, the source concept, its comment, and whether the element is
/// explicitly unmapped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchOrigin {
    /// The canonical of the map (`url|version`).
    pub origin_map: String,
    /// The source concept the match is for.
    pub source_concept: Option<Coding>,
    /// The element's comment.
    pub source_comment: Option<String>,
    /// `noMap`: the element is explicitly not mapped.
    pub no_map: bool,
}

/// The outcome of a translation: the R4B response and, per match, its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// `result`, `message`, and `match`.
    pub response: ConceptMapTranslateResponse,
    /// One origin per entry of `response.match`, in order.
    pub origins: Vec<MatchOrigin>,
}

/// The code under translation.
#[derive(Debug, Clone)]
struct Subject {
    system: String,
    version: Option<String>,
    code: String,
}

/// Runs `$translate`.
///
/// # Errors
///
/// Returns [`OperationError`] when the translation cannot be performed: no
/// or several code inputs, a code without a system, both `url` and
/// `conceptMap`, an unknown or invalid map, or `dependency`.
pub fn translate(
    sources: &Sources<'_>,
    request: &ConceptMapTranslateRequest,
) -> Result<Translation, OperationError> {
    if !request.dependency.is_empty() {
        return Err(OperationError::NotSupported(String::from(
            "`dependency` is not supported",
        )));
    }
    let maps = candidate_maps(sources, request)?;
    let reverse = request
        .reverse
        .as_ref()
        .and_then(|b| b.value)
        .unwrap_or(false);
    let target_system = uri_text(request.targetsystem.as_ref());
    let subjects = subjects(request)?;
    let mut matches = Vec::new();
    let mut origins = Vec::new();
    for subject in &subjects {
        for map in &maps {
            let found = matches_in(sources, map, subject, target_system, reverse, 0)?;
            for (found, origin) in found {
                matches.push(found);
                origins.push(origin);
            }
        }
    }
    let result = matches.iter().any(|m| {
        m.equivalence
            .as_ref()
            .and_then(|e| e.value.as_deref())
            .and_then(Relationship::from_equivalence)
            .is_some_and(Relationship::translates)
    });
    let message = if result {
        None
    } else {
        let subject = subjects
            .first()
            .map_or(String::new(), |s| format!("{}#{}", s.system, s.code));
        Some(match target_system {
            Some(target) => format!("no translation of `{subject}` to `{target}`"),
            None => format!("no translation of `{subject}`"),
        })
    };
    Ok(Translation {
        response: ConceptMapTranslateResponse {
            result: result.into(),
            message: message.as_deref().map(Into::into),
            r#match: matches,
        },
        origins,
    })
}

/// The codes to translate: the one `code`/`system`, the `coding`, or every
/// `codeableConcept.coding` with a system.
fn subjects(request: &ConceptMapTranslateRequest) -> Result<Vec<Subject>, OperationError> {
    let inputs = usize::from(request.code.is_some())
        + usize::from(request.coding.is_some())
        + usize::from(request.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    if let Some(code) = code_text(request.code.as_ref()) {
        let system = uri_text(request.system.as_ref()).ok_or_else(|| {
            OperationError::Required(String::from("`system` is required with `code`"))
        })?;
        return Ok(vec![Subject {
            system: system.to_owned(),
            version: string_text(request.version.as_ref()).map(str::to_owned),
            code: code.to_owned(),
        }]);
    }
    let codings: Vec<&Coding> = match (&request.coding, &request.codeable_concept) {
        (Some(coding), _) => vec![coding],
        (None, Some(concept)) => concept.coding.iter().collect(),
        (None, None) => Vec::new(),
    };
    let mut subjects = Vec::new();
    for coding in codings {
        let (system, version, code, _) = coding_parts(coding);
        let (Some(system), Some(code)) = (system, code) else {
            return Err(OperationError::Required(String::from(
                "a coding to translate needs `system` and `code`",
            )));
        };
        subjects.push(Subject {
            system: system.to_owned(),
            version: version.map(str::to_owned),
            code: code.to_owned(),
        });
    }
    if subjects.is_empty() {
        return Err(OperationError::Required(String::from(
            "`codeableConcept` carries no coding with `system` and `code`",
        )));
    }
    Ok(subjects)
}

/// The maps to consult: the inline one, the one `url` names, or every stored
/// map whose scopes fit `source` and `target`.
fn candidate_maps(
    sources: &Sources<'_>,
    request: &ConceptMapTranslateRequest,
) -> Result<Vec<Arc<ConceptMapModel>>, OperationError> {
    let url = uri_text(request.url.as_ref());
    match (&request.concept_map, url) {
        (Some(_), Some(_)) => Err(OperationError::Invalid(String::from(
            "provide either `url` or an inline `conceptMap`, not both",
        ))),
        (Some(inline), None) => Ok(vec![Arc::new(
            convert::r4b::convert(inline).map_err(|e| OperationError::Invalid(e.to_string()))?,
        )]),
        (None, Some(url)) => {
            let version = string_text(request.concept_map_version.as_ref());
            sources
                .concept_maps
                .resolve(url, version)
                .map(|map| vec![map])
                .ok_or_else(|| {
                    OperationError::UnknownConceptMap(match version {
                        Some(version) => format!("{url}|{version}"),
                        None => url.to_owned(),
                    })
                })
        }
        (None, None) => {
            let source = uri_text(request.source.as_ref());
            let target = uri_text(request.target.as_ref());
            Ok(sources
                .concept_maps
                .iter()
                .filter(|map| {
                    source.is_none_or(|s| map.source_scope.as_deref() == Some(s))
                        && target.is_none_or(|t| map.target_scope.as_deref() == Some(t))
                })
                .cloned()
                .collect())
        }
    }
}

/// The matches for `subject` in `map`.
fn matches_in(
    sources: &Sources<'_>,
    map: &ConceptMapModel,
    subject: &Subject,
    target_system: Option<&str>,
    reverse: bool,
    depth: usize,
) -> Result<Vec<(ConceptMapTranslateResponseMatch, MatchOrigin)>, OperationError> {
    let mut out = Vec::new();
    for group in &map.groups {
        let (from, from_version, to, to_version) = if reverse {
            (
                &group.target,
                &group.target_version,
                &group.source,
                &group.source_version,
            )
        } else {
            (
                &group.source,
                &group.source_version,
                &group.target,
                &group.target_version,
            )
        };
        if from.as_deref() != Some(subject.system.as_str()) {
            continue;
        }
        if let (Some(wanted), Some(named)) = (&subject.version, from_version)
            && wanted != named
        {
            continue;
        }
        if target_system.is_some_and(|t| to.as_deref() != Some(t)) {
            continue;
        }
        let origin = |element: Option<&Element>| MatchOrigin {
            origin_map: map.canonical(),
            source_concept: Some(Coding {
                system: Some(subject.system.as_str().into()),
                code: Some(subject.code.as_str().into()),
                ..Default::default()
            }),
            source_comment: element.and_then(|e| e.comment.clone()),
            no_map: element.is_some_and(|e| e.no_map),
        };
        let mut found = false;
        for (element, targets) in element_targets(group, subject, reverse) {
            found = true;
            if targets.is_empty() {
                out.push((
                    ConceptMapTranslateResponseMatch {
                        equivalence: Some(Relationship::Unmatched.equivalence().into()),
                        concept: None,
                        product: Vec::new(),
                        source: Some(map.canonical().as_str().into()),
                    },
                    origin(Some(element)),
                ));
            }
            for (code, display, relationship, product) in targets {
                out.push((
                    ConceptMapTranslateResponseMatch {
                        equivalence: Some(relationship.equivalence().into()),
                        concept: Some(Coding {
                            system: to.as_deref().map(Into::into),
                            version: to_version.as_deref().map(Into::into),
                            code: code.as_deref().map(Into::into),
                            display: display.as_deref().map(Into::into),
                            ..Default::default()
                        }),
                        product: product.iter().map(product_of).collect(),
                        source: Some(map.canonical().as_str().into()),
                    },
                    origin(Some(element)),
                ));
            }
        }
        if !found
            && !reverse
            && let Some(unmapped) = &group.unmapped
        {
            out.extend(unmapped_matches(
                sources,
                map,
                group,
                unmapped,
                subject,
                target_system,
                depth,
            )?);
        }
    }
    Ok(out)
}

/// The elements of `group` for `subject`, each with its targets read in the
/// requested direction: `(code, display, relationship, product)`.
#[expect(
    clippy::type_complexity,
    reason = "one tuple per matched target, consumed in place"
)]
fn element_targets<'a>(
    group: &'a Group,
    subject: &Subject,
    reverse: bool,
) -> Vec<(
    &'a Element,
    Vec<(
        &'a Option<String>,
        &'a Option<String>,
        Relationship,
        &'a [DependsOn],
    )>,
)> {
    if reverse {
        let mut out = Vec::new();
        for element in &group.elements {
            let hits: Vec<_> = element
                .targets
                .iter()
                .filter(|t| t.code.as_deref() == Some(subject.code.as_str()))
                .map(|t: &Target| {
                    (
                        &element.code,
                        &element.display,
                        t.relationship.inverse(),
                        t.product.as_slice(),
                    )
                })
                .collect();
            if !hits.is_empty() {
                out.push((element, hits));
            }
        }
        return out;
    }
    group
        .elements
        .iter()
        .filter(|e| e.code.as_deref() == Some(subject.code.as_str()))
        .map(|element| {
            (
                element,
                element
                    .targets
                    .iter()
                    .map(|t| (&t.code, &t.display, t.relationship, t.product.as_slice()))
                    .collect(),
            )
        })
        .collect()
}

/// The matches an `unmapped` rule yields for a code no element names.
fn unmapped_matches(
    sources: &Sources<'_>,
    map: &ConceptMapModel,
    group: &Group,
    unmapped: &crate::conceptmap::model::Unmapped,
    subject: &Subject,
    target_system: Option<&str>,
    depth: usize,
) -> Result<Vec<(ConceptMapTranslateResponseMatch, MatchOrigin)>, OperationError> {
    let origin = MatchOrigin {
        origin_map: map.canonical(),
        source_concept: Some(Coding {
            system: Some(subject.system.as_str().into()),
            code: Some(subject.code.as_str().into()),
            ..Default::default()
        }),
        source_comment: None,
        no_map: false,
    };
    let fixed = |code: Option<&str>, display: Option<&str>, relationship: Relationship| {
        (
            ConceptMapTranslateResponseMatch {
                equivalence: Some(relationship.equivalence().into()),
                concept: Some(Coding {
                    system: group.target.as_deref().map(Into::into),
                    version: group.target_version.as_deref().map(Into::into),
                    code: code.map(Into::into),
                    display: display.map(Into::into),
                    ..Default::default()
                }),
                product: Vec::new(),
                source: Some(map.canonical().as_str().into()),
            },
            origin.clone(),
        )
    };
    Ok(match unmapped.mode {
        // NOTE: R4B's `provided` says "use the code as provided", so the target is
        // the same code, `equal` unless the map says otherwise
        // (<https://hl7.org/fhir/R4B/valueset-conceptmap-unmapped-mode.html>).
        UnmappedMode::Provided => vec![fixed(
            Some(&subject.code),
            None,
            unmapped.relationship.unwrap_or(Relationship::Equal),
        )],
        UnmappedMode::Fixed => vec![fixed(
            unmapped.code.as_deref(),
            unmapped.display.as_deref(),
            unmapped.relationship.unwrap_or(Relationship::RelatedTo),
        )],
        UnmappedMode::OtherMap => {
            let Some(other) = &unmapped.other_map else {
                return Ok(Vec::new());
            };
            if depth >= OTHER_MAP_DEPTH {
                return Err(OperationError::Invalid(format!(
                    "`unmapped.mode = other-map` chains deeper than {OTHER_MAP_DEPTH} maps at `{other}`"
                )));
            }
            let other_map = sources
                .concept_maps
                .resolve(other, None)
                .ok_or_else(|| OperationError::UnknownConceptMap(other.clone()))?;
            matches_in(
                sources,
                &other_map,
                subject,
                target_system,
                false,
                depth + 1,
            )?
        }
    })
}

fn product_of(d: &DependsOn) -> ConceptMapTranslateResponseMatchProduct {
    ConceptMapTranslateResponseMatchProduct {
        element: Some(d.attribute.as_str().into()),
        concept: Some(Coding {
            system: d.system.as_deref().map(Into::into),
            code: Some(d.value.as_str().into()),
            display: d.display.as_deref().map(Into::into),
            ..Default::default()
        }),
    }
}
