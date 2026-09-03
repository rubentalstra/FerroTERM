//! `ValueSet/$expand` and `ValueSet/$validate-code` on R4B over the compose
//! layer (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>,
//! <https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).

use std::sync::Arc;

use ferroterm_testkit::fhir::{
    ANIMALS, COLOURS, VS_ALL, VS_ENUMERATED, VS_LOOP_A, VS_PETS, VS_PETS_REF, write_code_systems,
};
use fhir_terminology::conceptmap::store::ConceptMapStore;
use fhir_terminology::fhir_codesystem::load::{FhirVersion, load_dir};
use fhir_terminology::fhir_codesystem::provider::FhirCodeSystem;
use fhir_terminology::operations::CodingRef;
use fhir_terminology::operations::expand::{ExpandInput, ExpansionOutcome, ParameterValue};
use fhir_terminology::operations::value_set_validate_code::ValueSetValidateInput;
use fhir_terminology::operations::{OperationError, Sources, expand, value_set_validate_code};
use fhir_terminology::provider::PropertyValue;
use fhir_terminology::registry::Registry;
use fhir_terminology::valueset;
use fhir_terminology::valueset::store::ValueSetStore;
use fhir_types::r4b::value_set::{ValueSet, ValueSetCompose, ValueSetComposeInclude};

pub(crate) struct World {
    _dir: tempfile::TempDir,
    registry: Registry,
    value_sets: ValueSetStore,
    concept_maps: ConceptMapStore,
}

impl World {
    pub(crate) fn load() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        write_code_systems(dir.path()).expect("writes");
        let mut registry = Registry::new();
        for model in load_dir(dir.path(), FhirVersion::R5).expect("loads") {
            if model.supplements.is_none() {
                registry
                    .register(Arc::new(FhirCodeSystem::new(model).expect("builds")))
                    .expect("registers");
            }
        }
        let mut value_sets = ValueSetStore::new();
        for model in valueset::load::load_dir(dir.path(), FhirVersion::R5).expect("loads") {
            value_sets.insert(model).expect("stores");
        }
        let mut concept_maps = ConceptMapStore::new();
        for model in fhir_terminology::conceptmap::load::load_dir(dir.path(), FhirVersion::R5)
            .expect("loads")
        {
            concept_maps.insert(model).expect("stores");
        }
        Self {
            _dir: dir,
            registry,
            value_sets,
            concept_maps,
        }
    }

    pub(crate) fn concept_maps(&self) -> &ConceptMapStore {
        &self.concept_maps
    }

    pub(crate) fn sources(&self) -> Sources<'_> {
        Sources {
            registry: &self.registry,
            value_sets: &self.value_sets,
            concept_maps: &self.concept_maps,
        }
    }
}

fn codes(outcome: &ExpansionOutcome) -> Vec<String> {
    outcome.contains.iter().map(|c| c.code.clone()).collect()
}

fn parameter(outcome: &ExpansionOutcome, name: &str) -> Vec<ParameterValue> {
    outcome
        .parameters
        .iter()
        .filter(|p| p.name == name)
        .map(|p| p.value.clone())
        .collect()
}

#[test]
fn the_store_loads_every_value_set_and_defaults_to_the_greatest_version() {
    let world = World::load();
    assert_eq!(world.value_sets.len(), 8);
    let default = world.value_sets.resolve(VS_ALL, None).expect("default");
    assert_eq!(default.version.as_deref(), Some("2.0"));
    let pinned = world
        .value_sets
        .resolve(&format!("{VS_ALL}|1.0"), None)
        .expect("1.0");
    assert_eq!(pinned.version.as_deref(), Some("1.0"));
    assert!(world.value_sets.resolve(VS_ALL, Some("3.0")).is_none());
}

#[test]
fn expand_lists_the_system_flat_with_the_parameter_echo_and_used_codesystem() {
    let world = World::load();
    let request = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        value_set_version: Some(String::from("1.0")),
        exclude_nested: Some(true),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &request).expect("expands");
    assert_eq!(vs.model.version.as_deref(), Some("1.0"));
    assert!(
        !vs.include_definition,
        "includeDefinition defaults to false"
    );
    assert_eq!(vs.total, 9);
    assert!(vs.identifier.starts_with("urn:uuid:"));
    assert_eq!(
        codes(&vs),
        [
            "animal", "cat", "dodo", "dog", "fish", "kitten", "living", "pet", "plant"
        ]
    );
    let dodo = vs.contains.iter().find(|c| c.code == "dodo").expect("dodo");
    assert!(dodo.inactive);
    let pet = vs.contains.iter().find(|c| c.code == "pet").expect("pet");
    assert!(pet.abstract_concept);
    assert_eq!(parameter(&vs, "excludeNested").len(), 1);
    assert_eq!(
        parameter(&vs, "used-codesystem"),
        [ParameterValue::Uri(format!("{ANIMALS}|2.0"))]
    );
}

#[test]
fn expand_pages_filters_and_honours_active_only() {
    let world = World::load();
    let page = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        offset: Some(2),
        count: Some(3),
        active_only: Some(true),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &page).expect("expands");
    assert_eq!(vs.total, 7, "2.0 drops dodo; activeOnly drops fish");
    assert_eq!(vs.offset, Some(2));
    assert_eq!(codes(&vs), ["dog", "kitten", "living"]);
    let text = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        filter: Some(String::from("kat")),
        display_language: Some(String::from("de")),
        include_designations: Some(true),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &text).expect("expands");
    assert_eq!(codes(&vs), ["cat"]);
    let cat = &vs.contains[0];
    assert_eq!(cat.display.as_deref(), Some("Katze"));
    assert_eq!(
        cat.designations.len(),
        1,
        "only the requested language's designations"
    );
    let zero = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        count: Some(0),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &zero).expect("expands");
    assert!(codes(&vs).is_empty());
    assert_eq!(vs.total, 8);
}

#[test]
fn expand_follows_value_set_references_and_refuses_a_cycle() {
    let world = World::load();
    let referenced = ExpandInput {
        url: Some(VS_PETS_REF.to_owned()),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &referenced).expect("expands");
    assert_eq!(codes(&vs), ["kitten", "pet"]);
    let looped = ExpandInput {
        url: Some(VS_LOOP_A.to_owned()),
        ..ExpandInput::default()
    };
    let error = expand::expand(&world.sources(), &looped).expect_err("cycle");
    assert!(
        matches!(error, OperationError::ValueSetInvalid(_)),
        "{error}"
    );
    assert_eq!(error.issue_code(), "invalid");
}

#[test]
fn expand_takes_an_inline_value_set_and_pins_system_versions() {
    let world = World::load();
    let inline = ValueSet {
        url: Some("http://example.org/inline".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(COLOURS.into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let request = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        include_definition: Some(true),
        system_version: vec![format!("{COLOURS}|1")],
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &request).expect("expands");
    assert_eq!(codes(&vs), ["Green", "RED", "blue"]);
    assert!(
        vs.include_definition,
        "includeDefinition returns the compose"
    );
    let both = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        url: Some(VS_ALL.to_owned()),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &both),
        Err(OperationError::Invalid(_))
    ));
    let wrong_version = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        check_system_version: vec![format!("{COLOURS}|9")],
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &wrong_version),
        Err(OperationError::UnknownVersion { .. })
    ));
    let excluded = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        exclude_system: vec![COLOURS.to_owned()],
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &excluded).expect("expands");
    assert!(codes(&vs).is_empty());
}

#[test]
fn expand_refuses_what_it_cannot_answer() {
    let world = World::load();
    let unknown = ExpandInput {
        url: Some(String::from("http://example.org/fhir/ValueSet/nowhere")),
        ..ExpandInput::default()
    };
    let error = expand::expand(&world.sources(), &unknown).expect_err("unknown");
    assert!(matches!(error, OperationError::UnknownValueSet(_)));
    assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
    let none = ExpandInput::default();
    assert!(matches!(
        expand::expand(&world.sources(), &none),
        Err(OperationError::Required(_))
    ));
    let context = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        context: true,
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &context),
        Err(OperationError::NotSupported(_))
    ));
    let negative = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        offset: Some(-1),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &negative),
        Err(OperationError::Invalid(_))
    ));
    let unknown_system = ValueSet {
        url: Some("http://example.org/inline".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some("http://example.org/fhir/CodeSystem/nowhere".into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let request = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&unknown_system)),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &request),
        Err(OperationError::UnknownSystem(_))
    ));
    let no_system = ValueSet {
        url: Some("http://example.org/inline".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude::default()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let request = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&no_system)),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &request),
        Err(OperationError::ValueSetInvalid(_))
    ));
}

#[test]
fn an_unknown_filter_operator_is_refused_at_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("ValueSet-bad.json"),
        serde_json::json!({
            "resourceType": "ValueSet", "url": "http://example.org/bad", "status": "active",
            "compose": {"include": [{"system": ANIMALS, "filter": [{"property": "concept", "op": "equals", "value": "cat"}]}]}
        })
        .to_string(),
    )
    .expect("writes");
    let error = valueset::load::load_dir(dir.path(), FhirVersion::R4B).expect_err("refused");
    assert!(error.to_string().contains("ValueSet-bad.json"), "{error}");
}

#[test]
fn validate_code_answers_the_membership_the_echo_and_the_issues() {
    let world = World::load();
    let good = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &good).expect("validates");
    assert!(validation.result);
    assert_eq!(validation.display.as_deref(), Some("Kitten"));
    assert_eq!(validation.system.as_deref(), Some(ANIMALS));
    assert_eq!(validation.version.as_deref(), Some("2.0"));
    assert_eq!(validation.code.as_deref(), Some("kitten"));
    assert!(validation.issues.is_empty());
    let inferred = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &inferred).expect("validates");
    assert!(validation.result, "one system: inferred");
    let outside = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        coding: Some(CodingRef {
            system: Some(ANIMALS.to_owned()),
            code: Some(String::from("dog")),
            ..CodingRef::default()
        }),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &outside).expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues.len(), 1);
    assert_eq!(validation.issues[0].kind, "not-in-vs");
    assert_eq!(validation.issues[0].code, "code-invalid");
    assert_eq!(
        validation.code.as_deref(),
        Some("dog"),
        "a valid code outside the set is still echoed"
    );
    let unknown_code = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("unicorn")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &unknown_code).expect("validates");
    assert!(!validation.result);
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["not-in-vs", "invalid-code"]);
    let unknown_system = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(String::from("http://example.org/fhir/CodeSystem/nowhere")),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &unknown_system)
        .expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "not-found");
}

#[test]
fn validate_code_checks_display_case_and_inactive_codes() {
    let world = World::load();
    let wrong_display = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        display: Some(String::from("Hamster")),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &wrong_display)
        .expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "invalid-display");
    assert_eq!(validation.display.as_deref(), Some("Cat"));
    let synonym = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        display: Some(String::from("domestic  CAT")),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &synonym).expect("validates");
    assert!(
        validation.result,
        "a designation, whitespace and case folded"
    );
    let case = ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        code: Some(String::from("red")),
        system: Some(COLOURS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &case).expect("validates");
    assert!(validation.result, "colours is case-insensitive");
    assert_eq!(
        validation.code.as_deref(),
        Some("RED"),
        "the system's spelling is echoed"
    );
    let inactive = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        value_set_version: Some(String::from("1.0")),
        code: Some(String::from("dodo")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &inactive).expect("validates");
    assert!(validation.result, "inactive is not invalid");
    assert_eq!(validation.issues[0].kind, "status-check");
    assert_eq!(validation.issues[0].severity, "warning");
    let abstract_refused = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("pet")),
        system: Some(ANIMALS.to_owned()),
        abstract_ok: Some(false),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &abstract_refused)
        .expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "code-rule");
}

#[test]
fn validate_code_refuses_malformed_requests() {
    let world = World::load();
    let none = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        ..ValueSetValidateInput::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &none),
        Err(OperationError::Invalid(_))
    ));
    let two = ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("cat")),
        coding: Some(CodingRef::default()),
        ..ValueSetValidateInput::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &two),
        Err(OperationError::Invalid(_))
    ));
    let no_value_set = ValueSetValidateInput {
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &no_value_set),
        Err(OperationError::Required(_))
    ));
    let unknown = ValueSetValidateInput {
        url: Some(String::from("http://example.org/fhir/ValueSet/nowhere")),
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &unknown),
        Err(OperationError::UnknownValueSet(_))
    ));
    let ambiguous = ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        code: Some(String::from("cat")),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &ambiguous).expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "cannot-infer");
}

// NOTE: R5 `$expand` `property` asks for concept properties on each `contains`,
// by code or by the property's URI, `*` for all
// (<https://hl7.org/fhir/R5/valueset-operation-expand.html>).
#[test]
fn expand_returns_the_properties_asked_for() {
    let world = World::load();
    let by_code = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_PETS.to_owned()),
            property: vec![String::from("legs")],
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    let kitten = by_code
        .contains
        .iter()
        .find(|c| c.code == "kitten")
        .expect("kitten");
    assert_eq!(kitten.properties.len(), 1);
    assert_eq!(kitten.properties[0].code, "legs");
    assert_eq!(kitten.properties[0].value, PropertyValue::Integer(4));
    let pet = by_code
        .contains
        .iter()
        .find(|c| c.code == "pet")
        .expect("pet");
    assert!(pet.properties.is_empty(), "pet declares no leg count");
    assert_eq!(by_code.properties.len(), 1);
    assert_eq!(by_code.properties[0].code, "legs");
    assert_eq!(
        by_code.properties[0].uri.as_deref(),
        Some("http://example.org/legs")
    );
    assert_eq!(
        parameter(&by_code, "property"),
        [ParameterValue::Code(String::from("legs"))]
    );
    let by_uri = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_PETS.to_owned()),
            property: vec![String::from("http://example.org/legs")],
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    assert_eq!(
        by_uri.contains[0].properties,
        by_code.contains[0].properties
    );
    let all = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_PETS.to_owned()),
            property: vec![String::from("*")],
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    let kitten = all
        .contains
        .iter()
        .find(|c| c.code == "kitten")
        .expect("kitten");
    let codes: Vec<&str> = kitten.properties.iter().map(|p| p.code.as_str()).collect();
    assert!(
        codes.contains(&"legs") && codes.contains(&"parent"),
        "{codes:?}"
    );
    let none = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_PETS.to_owned()),
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    assert!(none.contains.iter().all(|c| c.properties.is_empty()));
    assert!(none.properties.is_empty());
}
