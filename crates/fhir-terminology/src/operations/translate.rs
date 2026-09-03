//! `ConceptMap/$translate` in the terms every served FHIR version shares.
//!
//! The operation pages: <https://hl7.org/fhir/R4B/conceptmap-operation-translate.html>
//! and <https://hl7.org/fhir/R5/conceptmap-operation-translate.html>.
//!
//! The map is inline, named by `url`, or chosen: every stored map whose
//! scopes fit `source` and `target` and whose groups map the code's system
//! (to `targetsystem` when given). One of `code` with `system`, `coding`, or
//! `codeableConcept` is the input; `reverse` reads the groups the other way.
//! An element without a target, or an `unmapped` rule, still yields a match,
//! so a client sees why nothing translated. `result` is true when a match has
//! a relationship other than `unmatched` or `disjoint`.

use std::sync::Arc;

use super::{CodingRef, OperationError, Sources};
use crate::conceptmap::model::{
    ConceptMapModel, DependsOn, Element, Group, ModelError, Relationship, Target, UnmappedMode,
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
    pub source_concept: Option<CodingRef>,
    /// The element's comment.
    pub source_comment: Option<String>,
    /// `noMap`: the element is explicitly not mapped.
    pub no_map: bool,
}

/// The outcome of a translation: the R4B response and, per match, its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// Whether some match translates the code (a relationship other than `unmatched` or `disjoint`).
    pub result: bool,
    /// Why nothing translated, when nothing did.
    pub message: Option<String>,
    /// The matches, in the order the maps and groups were read.
    pub matches: Vec<Match>,
}

/// One match of the translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The relationship between the source and the target.
    pub relationship: Relationship,
    /// The target concept, absent for an element without a target.
    pub concept: Option<CodingRef>,
    /// The other elements the match depends on (`product`).
    pub products: Vec<Product>,
    /// The canonical of the map the match came from (`source`).
    pub source: Option<String>,
    /// Where the match came from, beyond what every version declares.
    pub origin: MatchOrigin,
}

/// One `product` of a match: an attribute and the concept it depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// The attribute (`element`).
    pub element: String,
    /// The concept.
    pub concept: CodingRef,
}

/// The input of `$translate`: the union of the parameters the served versions
/// declare.
#[derive(Debug, Default)]
pub struct TranslateInput {
    /// The concept map URL (`url`).
    pub url: Option<String>,
    /// The concept map version (`conceptMapVersion`).
    pub concept_map_version: Option<String>,
    /// The inline `conceptMap`, converted by the wire layer of its version.
    pub inline_concept_map: Option<Result<ConceptMapModel, ModelError>>,
    /// The code.
    pub code: Option<String>,
    /// The code system URI (`system`).
    pub system: Option<String>,
    /// The code system version.
    pub version: Option<String>,
    /// The coding, instead of `code`.
    pub coding: Option<CodingRef>,
    /// The codings of a `codeableConcept`, instead of `code`.
    pub codeable_concept: Option<Vec<CodingRef>>,
    /// The source value set (`source`, R4B) or scope (`sourceScope`, R5).
    pub source: Option<String>,
    /// The target value set (`target`, R4B) or scope (`targetScope`, R5).
    pub target: Option<String>,
    /// The target code system (`targetsystem`, R4B; `targetSystem`, R5).
    pub target_system: Option<String>,
    /// Whether the groups are read the other way.
    pub reverse: Option<bool>,
    /// Whether `dependency` was given; not supported.
    pub dependency: bool,
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
    input: &TranslateInput,
) -> Result<Translation, OperationError> {
    if input.dependency {
        return Err(OperationError::NotSupported(String::from(
            "`dependency` is not supported",
        )));
    }
    let maps = candidate_maps(sources, input)?;
    let reverse = input.reverse.unwrap_or(false);
    let target_system = input.target_system.as_deref();
    let subjects = subjects(input)?;
    let mut matches = Vec::new();
    for subject in &subjects {
        for map in &maps {
            matches.extend(matches_in(
                sources,
                map,
                subject,
                target_system,
                reverse,
                0,
            )?);
        }
    }
    let result = matches.iter().any(|m| m.relationship.translates());
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
        result,
        message,
        matches,
    })
}

/// The codes to translate: the one `code`/`system`, the `coding`, or every
/// `codeableConcept.coding` with a system.
fn subjects(input: &TranslateInput) -> Result<Vec<Subject>, OperationError> {
    let inputs = usize::from(input.code.is_some())
        + usize::from(input.coding.is_some())
        + usize::from(input.codeable_concept.is_some());
    if inputs != 1 {
        return Err(OperationError::Invalid(String::from(
            "provide one and only one of `code`, `coding`, or `codeableConcept`",
        )));
    }
    if let Some(code) = input.code.as_deref() {
        let system = input.system.as_deref().ok_or_else(|| {
            OperationError::Required(String::from("`system` is required with `code`"))
        })?;
        return Ok(vec![Subject {
            system: system.to_owned(),
            version: input.version.clone(),
            code: code.to_owned(),
        }]);
    }
    let codings: Vec<&CodingRef> = match (&input.coding, &input.codeable_concept) {
        (Some(coding), _) => vec![coding],
        (None, Some(concept)) => concept.iter().collect(),
        (None, None) => Vec::new(),
    };
    let mut subjects = Vec::new();
    for coding in codings {
        let (Some(system), Some(code)) = (coding.system.as_deref(), coding.code.as_deref()) else {
            return Err(OperationError::Required(String::from(
                "a coding to translate needs `system` and `code`",
            )));
        };
        subjects.push(Subject {
            system: system.to_owned(),
            version: coding.version.clone(),
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
    input: &TranslateInput,
) -> Result<Vec<Arc<ConceptMapModel>>, OperationError> {
    let url = input.url.as_deref();
    match (&input.inline_concept_map, url) {
        (Some(_), Some(_)) => Err(OperationError::Invalid(String::from(
            "provide either `url` or an inline `conceptMap`, not both",
        ))),
        (Some(inline), None) => Ok(vec![Arc::new(
            inline
                .clone()
                .map_err(|e| OperationError::Invalid(e.to_string()))?,
        )]),
        (None, Some(url)) => {
            let version = input.concept_map_version.as_deref();
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
            let source = input.source.as_deref();
            let target = input.target.as_deref();
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
) -> Result<Vec<Match>, OperationError> {
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
            source_concept: Some(CodingRef {
                system: Some(subject.system.clone()),
                code: Some(subject.code.clone()),
                ..CodingRef::default()
            }),
            source_comment: element.and_then(|e| e.comment.clone()),
            no_map: element.is_some_and(|e| e.no_map),
        };
        let mut found = false;
        for (element, targets) in element_targets(group, subject, reverse) {
            found = true;
            if targets.is_empty() {
                out.push(Match {
                    relationship: Relationship::Unmatched,
                    concept: None,
                    products: Vec::new(),
                    source: Some(map.canonical()),
                    origin: origin(Some(element)),
                });
            }
            for (code, display, relationship, product) in targets {
                out.push(Match {
                    relationship,
                    concept: Some(CodingRef {
                        system: to.clone(),
                        version: to_version.clone(),
                        code: code.clone(),
                        display: display.clone(),
                    }),
                    products: product.iter().map(product_of).collect(),
                    source: Some(map.canonical()),
                    origin: origin(Some(element)),
                });
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
) -> Result<Vec<Match>, OperationError> {
    let origin = MatchOrigin {
        origin_map: map.canonical(),
        source_concept: Some(CodingRef {
            system: Some(subject.system.clone()),
            code: Some(subject.code.clone()),
            ..CodingRef::default()
        }),
        source_comment: None,
        no_map: false,
    };
    let fixed = |code: Option<&str>, display: Option<&str>, relationship: Relationship| Match {
        relationship,
        concept: Some(CodingRef {
            system: group.target.clone(),
            version: group.target_version.clone(),
            code: code.map(str::to_owned),
            display: display.map(str::to_owned),
        }),
        products: Vec::new(),
        source: Some(map.canonical()),
        origin: origin.clone(),
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

fn product_of(d: &DependsOn) -> Product {
    Product {
        element: d.attribute.clone(),
        concept: CodingRef {
            system: d.system.clone(),
            version: None,
            code: Some(d.value.clone()),
            display: d.display.clone(),
        },
    }
}
