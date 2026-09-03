use fhir_codegen::fhir::StructureKind;
use fhir_codegen::snapshot::{ElementShape, Max, ResolveError, ResolvedElement, ResolvedStructure};

use crate::R4B;

fn resolve(name: &str) -> ResolvedStructure {
    let definition = R4B
        .structure_definition_named(name)
        .expect("structure is defined");
    ResolvedStructure::resolve(definition).expect("snapshot resolves")
}

#[test]
fn every_r4b_snapshot_resolves() {
    for definition in R4B.structure_definitions().values() {
        if definition.snapshot.is_none() {
            continue;
        }
        if let Err(error) = ResolvedStructure::resolve(definition) {
            panic!("{}: {error}", definition.url);
        }
    }
}

#[test]
fn value_set_snapshot_is_resolved_in_order() {
    let value_set = resolve("ValueSet");
    assert_eq!(value_set.kind, StructureKind::Resource);
    assert_eq!(value_set.elements.len(), 85);
    let root = value_set.elements.first().expect("has a root");
    assert_eq!(root.path, "ValueSet");
    assert_eq!(root.shape, ElementShape::Root);
    assert_eq!(root.min, 0);
    assert_eq!(root.max, Max::Unbounded);
}

#[test]
fn cardinality_type_and_binding_are_carried() {
    let value_set = resolve("ValueSet");
    let op = value_set
        .element("ValueSet.compose.include.filter.op")
        .expect("filter.op exists");
    assert_eq!(op.name(), "op");
    assert_eq!((op.min, op.max), (1, Max::Bounded(1)));
    match &op.shape {
        ElementShape::Typed(types) => {
            assert_eq!(
                types.iter().map(|t| t.code.as_str()).collect::<Vec<_>>(),
                vec!["code"]
            );
        }
        other => panic!("expected Typed, got {other:?}"),
    }
    let binding = op.binding.as_ref().expect("filter.op is bound");
    assert_eq!(binding.strength, "required");
    assert_eq!(
        binding.value_set.as_deref(),
        Some("http://hl7.org/fhir/ValueSet/filter-operator|4.3.0")
    );

    let include = value_set
        .element("ValueSet.compose.include")
        .expect("include exists");
    assert_eq!((include.min, include.max), (1, Max::Unbounded));
    assert!(include.max.is_repeating());
    match &include.shape {
        ElementShape::Typed(types) => assert_eq!(types[0].code, "BackboneElement"),
        other => panic!("expected Typed, got {other:?}"),
    }
}

#[test]
fn choice_types_are_expanded() {
    let parameters = resolve("Parameters");
    let value = parameters
        .element("Parameters.parameter.value[x]")
        .expect("value[x] exists");
    assert_eq!(value.choice_stem(), Some("value"));
    let ElementShape::Choice(types) = &value.shape else {
        panic!("expected Choice, got {:?}", value.shape);
    };
    // Every R4B open datatype (https://hl7.org/fhir/R4B/datatypes.html#open).
    assert_eq!(types.len(), 50);
    let codes = types.iter().map(|t| t.code.as_str()).collect::<Vec<_>>();
    assert!(codes.contains(&"Coding"));
    assert!(codes.contains(&"CodeableConcept"));
    assert!(codes.contains(&"boolean"));
    assert!(codes.contains(&"Meta"));
    let plain = parameters
        .element("Parameters.parameter.name")
        .expect("name exists");
    assert_eq!(plain.choice_stem(), None);
}

#[test]
fn content_references_point_at_snapshot_elements() {
    let value_set = resolve("ValueSet");
    let exclude = value_set
        .element("ValueSet.compose.exclude")
        .expect("exclude exists");
    assert_eq!(
        exclude.shape,
        ElementShape::ContentReference {
            structure: None,
            path: "ValueSet.compose.include".to_owned(),
        }
    );
    let designation = value_set
        .element("ValueSet.expansion.contains.designation")
        .expect("designation exists");
    assert_eq!(
        designation.shape,
        ElementShape::ContentReference {
            structure: None,
            path: "ValueSet.compose.include.concept.designation".to_owned(),
        }
    );
}

#[test]
fn primitive_value_types_resolve_to_the_fhir_type() {
    let string = resolve("string");
    assert_eq!(string.kind, StructureKind::PrimitiveType);
    let value = string.element("string.value").expect("string.value exists");
    let ElementShape::Typed(types) = &value.shape else {
        panic!("expected Typed, got {:?}", value.shape);
    };
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].code, "string");
    assert_eq!(types[0].fhirpath_type.as_deref(), Some("String"));

    let decimal = resolve("decimal");
    let value = decimal
        .element("decimal.value")
        .expect("decimal.value exists");
    let ElementShape::Typed(types) = &value.shape else {
        panic!("expected Typed, got {:?}", value.shape);
    };
    assert_eq!(types[0].code, "decimal");
    assert_eq!(types[0].fhirpath_type.as_deref(), Some("Decimal"));
}

#[test]
fn target_profiles_are_carried_for_references() {
    let code_system = resolve("CodeSystem");
    let supplements = code_system
        .element("CodeSystem.supplements")
        .expect("supplements exists");
    let ElementShape::Typed(types) = &supplements.shape else {
        panic!("expected Typed, got {:?}", supplements.shape);
    };
    assert_eq!(types[0].code, "canonical");
    assert_eq!(
        types[0].target_profiles,
        vec!["http://hl7.org/fhir/StructureDefinition/CodeSystem".to_owned()]
    );
}

#[test]
fn children_of_walks_one_level() {
    let value_set = resolve("ValueSet");
    let children = value_set
        .children_of("ValueSet.compose")
        .map(ResolvedElement::name)
        .collect::<Vec<_>>();
    assert_eq!(
        children,
        vec![
            "id",
            "extension",
            "modifierExtension",
            "lockedDate",
            "inactive",
            "include",
            "exclude"
        ]
    );
    assert!(
        value_set
            .element("ValueSet.compose.include")
            .expect("exists")
            .is_child_of("ValueSet.compose")
    );
    assert!(
        !value_set
            .element("ValueSet.compose.include.system")
            .expect("exists")
            .is_child_of("ValueSet.compose")
    );
}

#[test]
fn a_structure_without_a_snapshot_is_refused() {
    let mut definition = R4B
        .structure_definition_named("Coding")
        .expect("Coding is defined")
        .clone();
    definition.snapshot = None;
    match ResolvedStructure::resolve(&definition) {
        Err(ResolveError::NoSnapshot { url }) => {
            assert_eq!(url, "http://hl7.org/fhir/StructureDefinition/Coding");
        }
        other => panic!("expected NoSnapshot, got {other:?}"),
    }
}

#[test]
fn an_invalid_max_is_refused() {
    let mut definition = R4B
        .structure_definition_named("Coding")
        .expect("Coding is defined")
        .clone();
    let snapshot = definition.snapshot.as_mut().expect("has snapshot");
    let element = snapshot
        .element
        .iter_mut()
        .find(|e| e.path == "Coding.code")
        .expect("Coding.code exists");
    element.max = Some("many".to_owned());
    match ResolvedStructure::resolve(&definition) {
        Err(ResolveError::InvalidMax { path, max, .. }) => {
            assert_eq!(path, "Coding.code");
            assert_eq!(max, "many");
        }
        other => panic!("expected InvalidMax, got {other:?}"),
    }
}

#[test]
fn a_dangling_content_reference_is_refused() {
    let mut definition = R4B
        .structure_definition_named("ValueSet")
        .expect("ValueSet is defined")
        .clone();
    let snapshot = definition.snapshot.as_mut().expect("has snapshot");
    let element = snapshot
        .element
        .iter_mut()
        .find(|e| e.path == "ValueSet.compose.exclude")
        .expect("exclude exists");
    element.content_reference = Some("#ValueSet.nowhere".to_owned());
    match ResolvedStructure::resolve(&definition) {
        Err(ResolveError::DanglingReference { path, target, .. }) => {
            assert_eq!(path, "ValueSet.compose.exclude");
            assert_eq!(target, "ValueSet.nowhere");
        }
        other => panic!("expected DanglingReference, got {other:?}"),
    }
}

#[test]
fn absolute_content_references_name_their_structure() {
    // The bmi profile points a component's referenceRange at the base Observation
    // (https://hl7.org/fhir/R4B/bmi.html), an absolute contentReference.
    let bmi = R4B
        .structure_definitions()
        .get("http://hl7.org/fhir/StructureDefinition/bmi")
        .expect("bmi profile is defined");
    let resolved = ResolvedStructure::resolve(bmi).expect("bmi resolves");
    let range = resolved
        .element("Observation.component.referenceRange")
        .expect("component.referenceRange exists");
    assert_eq!(
        range.shape,
        ElementShape::ContentReference {
            structure: Some("http://hl7.org/fhir/StructureDefinition/Observation".to_owned()),
            path: "Observation.referenceRange".to_owned(),
        }
    );
}
