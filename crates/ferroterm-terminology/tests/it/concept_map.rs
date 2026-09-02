//! `ConceptMap/$translate` on R4B
//! (<https://hl7.org/fhir/R4B/conceptmap-operation-translate.html>).

use ferroterm_fhir::r4b::codeable_concept::CodeableConcept;
use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::concept_map::{
    ConceptMap, ConceptMapGroup, ConceptMapGroupElement, ConceptMapGroupElementTarget,
};
use ferroterm_fhir::r4b::operations::concept_map_translate::{
    ConceptMapTranslateRequest, ConceptMapTranslateResponseMatch,
};
use ferroterm_terminology::conceptmap::model::Relationship;
use ferroterm_terminology::operations::OperationError;
use ferroterm_terminology::operations::translate::{Translation, translate};
use ferroterm_testkit::fhir::{
    ANIMALS, CM_ANIMALS_COLOURS, CM_FALLBACK, COLOURS, VS_ALL, VS_COLOURS,
};

use crate::value_set::World;

fn equivalence(m: &ConceptMapTranslateResponseMatch) -> &str {
    m.equivalence
        .as_ref()
        .and_then(|e| e.value.as_deref())
        .unwrap_or_default()
}

fn target_code(m: &ConceptMapTranslateResponseMatch) -> Option<&str> {
    m.concept
        .as_ref()
        .and_then(|c| c.code.as_ref())
        .and_then(|c| c.value.as_deref())
}

fn run(world: &World, request: &ConceptMapTranslateRequest) -> Translation {
    translate(&world.sources(), request).expect("translates")
}

#[test]
fn relationship_vocabularies_meet_in_one_enum() {
    assert_eq!(
        Relationship::from_relationship("source-is-broader-than-target"),
        Some(Relationship::Narrower)
    );
    assert_eq!(
        Relationship::from_equivalence("wider").map(Relationship::relationship),
        Some("source-is-narrower-than-target")
    );
    assert_eq!(Relationship::Narrower.inverse(), Relationship::Wider);
    assert!(!Relationship::Unmatched.translates());
    assert!(Relationship::RelatedTo.translates());
    assert_eq!(Relationship::from_equivalence("kinda"), None);
}

#[test]
fn the_store_loads_both_maps_with_the_r5_shapes_reduced() {
    let world = World::load();
    assert_eq!(world.concept_maps().len(), 2);
    let map = world
        .concept_maps()
        .resolve(CM_ANIMALS_COLOURS, None)
        .expect("map");
    assert_eq!(map.source_scope.as_deref(), Some(VS_ALL));
    assert_eq!(map.target_scope.as_deref(), Some(VS_COLOURS));
    let group = &map.groups[0];
    assert_eq!(group.source.as_deref(), Some(ANIMALS));
    let dog = group
        .elements
        .iter()
        .find(|e| e.code.as_deref() == Some("dog"))
        .expect("dog");
    assert_eq!(dog.targets[0].relationship, Relationship::Narrower);
    assert_eq!(dog.targets[0].comment.as_deref(), Some("roughly"));
    let fish = group
        .elements
        .iter()
        .find(|e| e.code.as_deref() == Some("fish"))
        .expect("fish");
    assert!(fish.no_map);
    assert_eq!(fish.comment.as_deref(), Some("fish have no colour"));
    assert_eq!(
        group.unmapped.as_ref().and_then(|u| u.relationship),
        Some(Relationship::RelatedTo)
    );
}

#[test]
fn translate_by_url_code_coding_and_codeable_concept() {
    let world = World::load();
    let by_code = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let translation = run(&world, &by_code);
    assert_eq!(translation.response.result.value, Some(true));
    let m = &translation.response.r#match[0];
    assert_eq!(equivalence(m), "equivalent");
    assert_eq!(target_code(m), Some("RED"));
    assert_eq!(
        m.concept
            .as_ref()
            .and_then(|c| c.system.as_ref())
            .and_then(|s| s.value.as_deref()),
        Some(COLOURS)
    );
    assert_eq!(
        m.source.as_ref().and_then(|s| s.value.as_deref()),
        Some(format!("{CM_ANIMALS_COLOURS}|1.0").as_str())
    );
    assert_eq!(
        translation.origins[0].origin_map,
        format!("{CM_ANIMALS_COLOURS}|1.0")
    );
    assert_eq!(
        translation.origins[0]
            .source_concept
            .as_ref()
            .and_then(|c| c.code.as_ref())
            .and_then(|c| c.value.as_deref()),
        Some("cat")
    );
    let by_coding = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        coding: Some(Coding {
            system: Some(ANIMALS.into()),
            code: Some("dog".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let translation = run(&world, &by_coding);
    assert_eq!(equivalence(&translation.response.r#match[0]), "narrower");
    assert_eq!(
        translation.origins[0].source_comment, None,
        "the comment on the target is not the element's"
    );
    let by_concept = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        codeable_concept: Some(CodeableConcept {
            coding: vec![
                Coding {
                    system: Some(ANIMALS.into()),
                    code: Some("cat".into()),
                    ..Default::default()
                },
                Coding {
                    system: Some(ANIMALS.into()),
                    code: Some("dog".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let translation = run(&world, &by_concept);
    assert_eq!(translation.response.r#match.len(), 2);
}

#[test]
fn no_map_and_unmapped_rules_still_answer() {
    let world = World::load();
    let fish = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("fish".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let translation = run(&world, &fish);
    assert_eq!(translation.response.result.value, Some(false));
    assert_eq!(equivalence(&translation.response.r#match[0]), "unmatched");
    assert!(translation.origins[0].no_map);
    assert_eq!(
        translation.origins[0].source_comment.as_deref(),
        Some("fish have no colour")
    );
    assert!(translation.response.message.is_some());
    let kitten = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("kitten".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let translation = run(&world, &kitten);
    assert_eq!(
        translation.response.result.value,
        Some(true),
        "fixed unmapped"
    );
    assert_eq!(equivalence(&translation.response.r#match[0]), "relatedto");
    assert_eq!(target_code(&translation.response.r#match[0]), Some("blue"));
    let via_other = ConceptMapTranslateRequest {
        url: Some(CM_FALLBACK.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let translation = run(&world, &via_other);
    assert_eq!(
        target_code(&translation.response.r#match[0]),
        Some("RED"),
        "other-map defers"
    );
    assert_eq!(
        translation.origins[0].origin_map,
        format!("{CM_ANIMALS_COLOURS}|1.0")
    );
}

#[test]
fn maps_are_chosen_by_scope_and_target_system_and_read_in_reverse() {
    let world = World::load();
    let chosen = ConceptMapTranslateRequest {
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        source: Some(VS_ALL.into()),
        targetsystem: Some(COLOURS.into()),
        ..Default::default()
    };
    let translation = run(&world, &chosen);
    assert_eq!(translation.response.result.value, Some(true));
    assert_eq!(
        translation.response.r#match.len(),
        1,
        "the fallback map has no matching scope"
    );
    let wrong_target = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        targetsystem: Some("http://example.org/fhir/CodeSystem/nowhere".into()),
        ..Default::default()
    };
    let translation = run(&world, &wrong_target);
    assert_eq!(translation.response.result.value, Some(false));
    assert!(translation.response.r#match.is_empty());
    let reverse = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("Green".into()),
        system: Some(COLOURS.into()),
        reverse: Some(true.into()),
        ..Default::default()
    };
    let translation = run(&world, &reverse);
    assert_eq!(target_code(&translation.response.r#match[0]), Some("dog"));
    assert_eq!(
        equivalence(&translation.response.r#match[0]),
        "wider",
        "the relationship is inverted"
    );
}

#[test]
fn an_inline_map_translates_and_malformed_requests_are_refused() {
    let world = World::load();
    let inline = ConceptMap {
        url: Some("http://example.org/inline-map".into()),
        status: "draft".into(),
        group: vec![ConceptMapGroup {
            source: Some(ANIMALS.into()),
            target: Some(COLOURS.into()),
            element: vec![ConceptMapGroupElement {
                code: Some("plant".into()),
                target: vec![ConceptMapGroupElementTarget {
                    code: Some("Green".into()),
                    equivalence: "wider".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let request = ConceptMapTranslateRequest {
        concept_map: Some(inline.clone()),
        code: Some("plant".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let translation = run(&world, &request);
    assert_eq!(target_code(&translation.response.r#match[0]), Some("Green"));
    let both = ConceptMapTranslateRequest {
        concept_map: Some(inline),
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("plant".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    assert!(matches!(
        translate(&world.sources(), &both),
        Err(OperationError::Invalid(_))
    ));
    let no_system = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("cat".into()),
        ..Default::default()
    };
    assert!(matches!(
        translate(&world.sources(), &no_system),
        Err(OperationError::Required(_))
    ));
    let two = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        coding: Some(Coding::default()),
        ..Default::default()
    };
    assert!(matches!(
        translate(&world.sources(), &two),
        Err(OperationError::Invalid(_))
    ));
    let unknown = ConceptMapTranslateRequest {
        url: Some("http://example.org/fhir/ConceptMap/nowhere".into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let error = translate(&world.sources(), &unknown).expect_err("unknown");
    assert!(matches!(error, OperationError::UnknownConceptMap(_)));
    assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
    let other_system = ConceptMapTranslateRequest {
        url: Some(CM_ANIMALS_COLOURS.into()),
        code: Some("x".into()),
        system: Some("http://example.org/fhir/CodeSystem/nowhere".into()),
        ..Default::default()
    };
    let translation = run(&world, &other_system);
    assert_eq!(translation.response.result.value, Some(false));
    assert!(
        translation.response.r#match.is_empty(),
        "no group covers the system"
    );
}
