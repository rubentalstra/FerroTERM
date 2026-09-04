//! The implicit concept maps of SNOMED CT: `http://snomed.info/sct?fhir_cm=[sctid]`.
//!
//! The FHIR SNOMED CT page defines the form over four association reference
//! sets, gives each the relationship its members assert, and fixes the shape of
//! the `ConceptMap` it produces
//! (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps"). An
//! association member points at another SNOMED concept through
//! `targetComponentId`, so its group names SNOMED as both source and target.
//!
//! The same page says a simple map reference set also defines an implicit
//! concept map, and gives it no template. A map member points at a code of
//! another system through `mapTarget`, and no RF2 file records which system
//! that is, so the group names no target. No specification says how the
//! complex and extended map columns reach `$translate` either; carrying them as
//! `product` parts is our own design.

use concept_graph::ordinal::Ordinal;
use concept_graph::refsets::{Table, ValueRef};

use crate::conceptmap::model::{ConceptMapModel, DependsOn, Element, Group, Relationship, Target};
use crate::provider::{CodeSystemProvider, Concept, ProviderError, Successor};
use crate::snomed::{FHIR_CM, FHIR_VS, SYSTEM, SnomedProvider};

/// The reference set field an association member points through.
const TARGET_COMPONENT: &str = "targetComponentId";
/// The reference set field a map member points through.
const MAP_TARGET: &str = "mapTarget";
/// The complex and extended map fields that travel as `product` parts, in the
/// order the RF2 release format declares them.
///
/// No FHIR version says where these belong in a `$translate` result, so this
/// is our own design: `product` keeps them beside the target they qualify.
const PRODUCT_FIELDS: [&str; 6] = [
    "mapGroup",
    "mapPriority",
    "mapRule",
    "mapAdvice",
    "correlationId",
    "mapCategoryId",
];

/// One historical association reference set: its concept id and the
/// `ConceptMap` relationship its members carry.
struct Association {
    /// The reference set concept id.
    refset: u64,
    /// The relationship a member of it asserts.
    relationship: Relationship,
}

/// The association reference sets the FHIR SNOMED CT page lists as implicit
/// concept maps, each with the relationship that page gives it
/// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
const ASSOCIATIONS: [Association; 4] = [
    // `900000000000523009 |POSSIBLY EQUIVALENT TO association reference set|`.
    Association {
        refset: 900_000_000_000_523_009,
        relationship: Relationship::Inexact,
    },
    // `900000000000526001 |REPLACED BY association reference set|`.
    Association {
        refset: 900_000_000_000_526_001,
        relationship: Relationship::Equivalent,
    },
    // `900000000000527005 |SAME AS association reference set|`.
    Association {
        refset: 900_000_000_000_527_005,
        relationship: Relationship::Equal,
    },
    // `900000000000530003 |ALTERNATIVE association reference set|`.
    Association {
        refset: 900_000_000_000_530_003,
        relationship: Relationship::Inexact,
    },
];

/// The reference sets a translation of an inactive concept falls back to: the
/// concept a `SAME AS` names is the same concept, and the one `REPLACED BY`
/// names supersedes it.
const SUCCESSORS: [u64; 2] = [sct_ecl::eval::SAME_AS, sct_ecl::eval::REPLACED_BY];

/// The relationship the members of `refset` assert, when it is a historical
/// association reference set.
fn association(refset: u64) -> Option<Relationship> {
    ASSOCIATIONS
        .iter()
        .find(|held| held.refset == refset)
        .map(|held| held.relationship)
}

/// The concepts this edition's `SAME AS` and `REPLACED BY` reference sets name
/// in place of `concept`.
///
/// # Errors
///
/// Returns the storage error of a concept that does not read.
pub(crate) fn successors(
    edition: &SnomedProvider,
    concept: Concept,
) -> Result<Vec<Successor>, ProviderError> {
    let ordinal = Ordinal::new(concept.index());
    let mut out = Vec::new();
    for refset in SUCCESSORS {
        let Some(table) = edition.member_tables().table(refset) else {
            continue;
        };
        if !table.members().contains(ordinal.index()) {
            continue;
        }
        let Some(field) = table.field(TARGET_COMPONENT) else {
            continue;
        };
        let relationship = association(refset).unwrap_or(Relationship::RelatedTo);
        for row in 0..table.len() {
            if table.concept(row) != Some(ordinal) {
                continue;
            }
            let Some(target) = association_target(edition, table, row, field, Some(relationship))?
            else {
                continue;
            };
            let Some(code) = target.code else {
                continue;
            };
            out.push(Successor {
                code,
                display: target.display,
                relationship,
                map: format!("{SYSTEM}?{FHIR_CM}={refset}"),
            });
        }
    }
    Ok(out)
}

/// The `ConceptMap` the implicit URI `url` denotes, over the reference set
/// `refset` of `edition`.
///
/// # Errors
///
/// Returns [`ProviderError::UnknownImplicitConceptMap`] when the edition holds
/// no such reference set, and the storage error of a concept that does not read.
pub(crate) fn concept_map(
    edition: &SnomedProvider,
    url: &str,
    refset: u64,
) -> Result<ConceptMapModel, ProviderError> {
    let Some(table) = edition.member_tables().table(refset) else {
        return Err(ProviderError::UnknownImplicitConceptMap {
            url: url.to_owned(),
        });
    };
    let relationship = association(refset);
    let target_component = table.field(TARGET_COMPONENT);
    let map_target = table.field(MAP_TARGET);
    // NOTE: a reference set with neither column maps nothing, so the URI names no
    // concept map (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
    if target_component.is_none() && map_target.is_none() {
        return Err(ProviderError::UnknownImplicitConceptMap {
            url: url.to_owned(),
        });
    }
    let mut elements: Vec<Element> = Vec::new();
    for row in 0..table.len() {
        let Some(source) = table.concept(row) else {
            continue;
        };
        let Some(code) = code_of(edition, source)? else {
            continue;
        };
        let target = match (target_component, map_target) {
            (Some(field), _) => association_target(edition, table, row, field, relationship)?,
            (None, Some(field)) => map(edition, table, row, field),
            (None, None) => None,
        };
        let Some(target) = target else {
            continue;
        };
        let display = edition.display(Concept::new(source.index()), None)?;
        match elements
            .iter_mut()
            .find(|held| held.code.as_deref() == Some(&code))
        {
            Some(held) => held.targets.push(target),
            None => elements.push(Element {
                code: Some(code),
                display,
                no_map: false,
                comment: None,
                targets: vec![target],
            }),
        }
    }
    // NOTE: a map reference set names its target system nowhere in the RF2 rows, so
    // only an association map, whose targets are SNOMED concepts, declares one
    // (<https://hl7.org/fhir/R4B/snomedct.html>).
    // NOTE: a bare `http://snomed.info/sct` base means an unspecified edition, and the
    // server SHALL answer from the edition it serves, so the map states that version
    // either way (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
    let base = edition.identity().version.clone();
    let target_system = relationship.map(|_| SYSTEM.to_owned());
    let target_version = relationship.map(|_| base.clone());
    // NOTE: the page's template names the map after the reference set and scopes it
    // to the edition's own implicit value set
    // (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Concept Maps").
    let scope = Some(format!("{base}?{FHIR_VS}"));
    let refset_name = name_of(edition, refset);
    Ok(ConceptMapModel {
        url: url.to_owned(),
        version: Some(base.clone()),
        name: refset_name
            .as_ref()
            .map(|name| format!("SNOMED CT {name} Concept Map")),
        title: None,
        status: String::from("active"),
        source_scope: scope.clone(),
        target_scope: scope,
        groups: vec![Group {
            source: Some(SYSTEM.to_owned()),
            source_version: Some(base),
            target: target_system,
            target_version,
            elements,
            unmapped: None,
        }],
    })
}

/// The name of the reference set concept `refset`, for the map's `name`.
fn name_of(edition: &SnomedProvider, refset: u64) -> Option<String> {
    let located = edition.locate(&refset.to_string()).ok()??;
    edition.display(located.concept, None).ok()?
}

/// The target of an association member: the concept `targetComponentId` names.
fn association_target(
    edition: &SnomedProvider,
    table: &Table,
    row: usize,
    field: usize,
    relationship: Option<Relationship>,
) -> Result<Option<Target>, ProviderError> {
    let (code, display) = match table.value(row, field) {
        Some(ValueRef::Concept(ordinal)) => (
            code_of(edition, ordinal)?,
            edition.display(Concept::new(ordinal.index()), None)?,
        ),
        // NOTE: a target outside the edition keeps its component id as the code, the
        // only identity the row carries (RF2 §Association Reference Set).
        Some(ValueRef::Component(id)) => (Some(id.to_string()), None),
        _ => (None, None),
    };
    Ok(code.map(|code| Target {
        code: Some(code),
        display,
        relationship: relationship.unwrap_or(Relationship::RelatedTo),
        comment: None,
        depends_on: Vec::new(),
        product: Vec::new(),
    }))
}

/// The target of a map member: the code `mapTarget` carries, with the complex
/// and extended columns as `product` parts.
///
/// No specification says which `equivalence` an RF2 map row asserts, so the
/// target is `relatedto`, the code for concepts that overlap in meaning by an
/// unstated relationship
/// (<https://hl7.org/fhir/R4B/valueset-concept-map-equivalence.html>); the
/// row's own `correlationId` travels with it as a `product` part.
fn map(edition: &SnomedProvider, table: &Table, row: usize, field: usize) -> Option<Target> {
    let code = match table.value(row, field)? {
        ValueRef::String(text) if !text.is_empty() => text.to_owned(),
        _ => return None,
    };
    let product = PRODUCT_FIELDS
        .into_iter()
        .filter_map(|name| part(edition, table, row, name))
        .collect();
    Some(Target {
        code: Some(code),
        display: None,
        relationship: Relationship::RelatedTo,
        comment: None,
        depends_on: Vec::new(),
        product,
    })
}

/// One `product` part: the value of the column `name`, when the row has one.
fn part(edition: &SnomedProvider, table: &Table, row: usize, name: &str) -> Option<DependsOn> {
    let field = table.field(name)?;
    let (system, value) = match table.value(row, field)? {
        ValueRef::Concept(ordinal) => (
            Some(SYSTEM.to_owned()),
            code_of(edition, ordinal).ok().flatten()?,
        ),
        ValueRef::Component(id) => (Some(SYSTEM.to_owned()), id.to_string()),
        ValueRef::Integer(value) => (None, value.to_string()),
        ValueRef::String(text) if !text.is_empty() => (None, text.to_owned()),
        ValueRef::String(_) => return None,
    };
    Some(DependsOn {
        attribute: name.to_owned(),
        system,
        value,
        display: None,
    })
}

/// The code of the concept at `ordinal`.
fn code_of(edition: &SnomedProvider, ordinal: Ordinal) -> Result<Option<String>, ProviderError> {
    edition.code(Concept::new(ordinal.index()))
}
