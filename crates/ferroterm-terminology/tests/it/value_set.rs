//! `ValueSet/$expand` and `ValueSet/$validate-code` on R4B over the compose
//! layer (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>,
//! <https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).

use std::sync::Arc;

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::value_set_expand::ValueSetExpandRequest;
use ferroterm_fhir::r4b::operations::value_set_validate_code::ValueSetValidateCodeRequest;
use ferroterm_fhir::r4b::value_set::{ValueSet, ValueSetCompose, ValueSetComposeInclude};
use ferroterm_terminology::fhir_codesystem::load::{FhirVersion, load_dir};
use ferroterm_terminology::fhir_codesystem::provider::FhirCodeSystem;
use ferroterm_terminology::operations::{OperationError, Sources, expand, value_set_validate_code};
use ferroterm_terminology::registry::Registry;
use ferroterm_terminology::valueset;
use ferroterm_terminology::valueset::store::ValueSetStore;
use ferroterm_testkit::fhir::{
    ANIMALS, COLOURS, VS_ALL, VS_ENUMERATED, VS_LOOP_A, VS_PETS, VS_PETS_REF, write_code_systems,
};

struct World {
    _dir: tempfile::TempDir,
    registry: Registry,
    value_sets: ValueSetStore,
}

impl World {
    fn load() -> Self {
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
        Self {
            _dir: dir,
            registry,
            value_sets,
        }
    }

    fn sources(&self) -> Sources<'_> {
        Sources {
            registry: &self.registry,
            value_sets: &self.value_sets,
        }
    }
}

fn codes(value_set: &ValueSet) -> Vec<String> {
    value_set
        .expansion
        .as_ref()
        .expect("expansion")
        .contains
        .iter()
        .filter_map(|c| c.code.as_ref().and_then(|c| c.value.clone()))
        .collect()
}

fn parameter(value_set: &ValueSet, name: &str) -> Vec<String> {
    value_set
        .expansion
        .as_ref()
        .expect("expansion")
        .parameter
        .iter()
        .filter(|p| p.name.value.as_deref() == Some(name))
        .map(|p| format!("{:?}", p.value))
        .collect()
}

#[test]
fn the_store_loads_every_value_set_and_defaults_to_the_greatest_version() {
    let world = World::load();
    assert_eq!(world.value_sets.len(), 7);
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
    let request = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        value_set_version: Some("1.0".into()),
        exclude_nested: Some(true.into()),
        ..Default::default()
    };
    let response = expand::expand(&world.sources(), &request).expect("expands");
    let vs = &response.r#return;
    assert_eq!(
        vs.version.as_ref().and_then(|v| v.value.as_deref()),
        Some("1.0")
    );
    assert!(vs.compose.is_none(), "includeDefinition defaults to false");
    let expansion = vs.expansion.as_ref().expect("expansion");
    assert_eq!(expansion.total.as_ref().and_then(|t| t.value), Some(9));
    assert!(
        expansion
            .identifier
            .as_ref()
            .and_then(|i| i.value.as_deref())
            .is_some_and(|i| i.starts_with("urn:uuid:"))
    );
    assert_eq!(
        codes(vs),
        [
            "animal", "cat", "dodo", "dog", "fish", "kitten", "living", "pet", "plant"
        ]
    );
    let dodo = expansion
        .contains
        .iter()
        .find(|c| c.code.as_ref().and_then(|c| c.value.as_deref()) == Some("dodo"))
        .expect("dodo");
    assert_eq!(dodo.inactive.as_ref().and_then(|b| b.value), Some(true));
    let pet = expansion
        .contains
        .iter()
        .find(|c| c.code.as_ref().and_then(|c| c.value.as_deref()) == Some("pet"))
        .expect("pet");
    assert_eq!(pet.r#abstract.as_ref().and_then(|b| b.value), Some(true));
    assert_eq!(parameter(vs, "excludeNested").len(), 1);
    assert_eq!(
        parameter(vs, "used-codesystem"),
        [format!(
            "Some(Uri(Uri {{ id: None, extension: [], value: Some(\"{ANIMALS}|2.0\") }}))"
        )]
    );
}

#[test]
fn expand_pages_filters_and_honours_active_only() {
    let world = World::load();
    let page = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        offset: Some(2.into()),
        count: Some(3.into()),
        active_only: Some(true.into()),
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &page)
        .expect("expands")
        .r#return;
    let expansion = vs.expansion.as_ref().expect("expansion");
    assert_eq!(
        expansion.total.as_ref().and_then(|t| t.value),
        Some(7),
        "2.0 drops dodo; activeOnly drops fish"
    );
    assert_eq!(expansion.offset.as_ref().and_then(|o| o.value), Some(2));
    assert_eq!(codes(&vs), ["dog", "kitten", "living"]);
    let text = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        filter: Some("kat".into()),
        display_language: Some("de".into()),
        include_designations: Some(true.into()),
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &text)
        .expect("expands")
        .r#return;
    assert_eq!(codes(&vs), ["cat"]);
    let cat = &vs.expansion.as_ref().expect("expansion").contains[0];
    assert_eq!(
        cat.display.as_ref().and_then(|d| d.value.as_deref()),
        Some("Katze")
    );
    assert_eq!(
        cat.designation.len(),
        1,
        "only the requested language's designations"
    );
    let zero = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        count: Some(0.into()),
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &zero)
        .expect("expands")
        .r#return;
    assert!(codes(&vs).is_empty());
    assert_eq!(
        vs.expansion
            .as_ref()
            .and_then(|e| e.total.as_ref())
            .and_then(|t| t.value),
        Some(8)
    );
}

#[test]
fn expand_follows_value_set_references_and_refuses_a_cycle() {
    let world = World::load();
    let referenced = ValueSetExpandRequest {
        url: Some(VS_PETS_REF.into()),
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &referenced)
        .expect("expands")
        .r#return;
    assert_eq!(codes(&vs), ["kitten", "pet"]);
    let looped = ValueSetExpandRequest {
        url: Some(VS_LOOP_A.into()),
        ..Default::default()
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
    let request = ValueSetExpandRequest {
        value_set: Some(inline.clone()),
        include_definition: Some(true.into()),
        system_version: vec![format!("{COLOURS}|1").as_str().into()],
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &request)
        .expect("expands")
        .r#return;
    assert_eq!(codes(&vs), ["Green", "RED", "blue"]);
    assert!(
        vs.compose.is_some(),
        "includeDefinition returns the compose"
    );
    let both = ValueSetExpandRequest {
        value_set: Some(inline.clone()),
        url: Some(VS_ALL.into()),
        ..Default::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &both),
        Err(OperationError::Invalid(_))
    ));
    let wrong_version = ValueSetExpandRequest {
        value_set: Some(inline.clone()),
        check_system_version: vec![format!("{COLOURS}|9").as_str().into()],
        ..Default::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &wrong_version),
        Err(OperationError::UnknownVersion { .. })
    ));
    let excluded = ValueSetExpandRequest {
        value_set: Some(inline),
        exclude_system: vec![COLOURS.into()],
        ..Default::default()
    };
    let vs = expand::expand(&world.sources(), &excluded)
        .expect("expands")
        .r#return;
    assert!(codes(&vs).is_empty());
}

#[test]
fn expand_refuses_what_it_cannot_answer() {
    let world = World::load();
    let unknown = ValueSetExpandRequest {
        url: Some("http://example.org/fhir/ValueSet/nowhere".into()),
        ..Default::default()
    };
    let error = expand::expand(&world.sources(), &unknown).expect_err("unknown");
    assert!(matches!(error, OperationError::UnknownValueSet(_)));
    assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
    let none = ValueSetExpandRequest::default();
    assert!(matches!(
        expand::expand(&world.sources(), &none),
        Err(OperationError::Required(_))
    ));
    let context = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        context: Some("Patient.gender".into()),
        ..Default::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &context),
        Err(OperationError::NotSupported(_))
    ));
    let negative = ValueSetExpandRequest {
        url: Some(VS_ALL.into()),
        offset: Some((-1).into()),
        ..Default::default()
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
    let request = ValueSetExpandRequest {
        value_set: Some(unknown_system),
        ..Default::default()
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
    let request = ValueSetExpandRequest {
        value_set: Some(no_system),
        ..Default::default()
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
    let good = ValueSetValidateCodeRequest {
        url: Some(VS_PETS.into()),
        code: Some("kitten".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &good).expect("validates");
    assert_eq!(validation.response.result.value, Some(true));
    assert_eq!(
        validation
            .response
            .display
            .as_ref()
            .and_then(|d| d.value.as_deref()),
        Some("Kitten")
    );
    assert_eq!(validation.system.as_deref(), Some(ANIMALS));
    assert_eq!(validation.version.as_deref(), Some("2.0"));
    assert_eq!(validation.code.as_deref(), Some("kitten"));
    assert!(validation.issues.is_empty());
    let inferred = ValueSetValidateCodeRequest {
        url: Some(VS_PETS.into()),
        code: Some("kitten".into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &inferred).expect("validates");
    assert_eq!(
        validation.response.result.value,
        Some(true),
        "one system: inferred"
    );
    let outside = ValueSetValidateCodeRequest {
        url: Some(VS_PETS.into()),
        coding: Some(Coding {
            system: Some(ANIMALS.into()),
            code: Some("dog".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &outside).expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    assert_eq!(validation.issues.len(), 1);
    assert_eq!(validation.issues[0].kind, "not-in-vs");
    assert_eq!(validation.issues[0].code, "code-invalid");
    assert_eq!(
        validation.code.as_deref(),
        None,
        "a code outside the set is not echoed"
    );
    let unknown_code = ValueSetValidateCodeRequest {
        url: Some(VS_PETS.into()),
        code: Some("unicorn".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &unknown_code).expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["not-in-vs", "invalid-code"]);
    let unknown_system = ValueSetValidateCodeRequest {
        url: Some(VS_PETS.into()),
        code: Some("kitten".into()),
        system: Some("http://example.org/fhir/CodeSystem/nowhere".into()),
        ..Default::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &unknown_system)
        .expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    assert_eq!(validation.issues[0].kind, "not-found");
}

#[test]
fn validate_code_checks_display_case_and_inactive_codes() {
    let world = World::load();
    let wrong_display = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        display: Some("Hamster".into()),
        ..Default::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &wrong_display)
        .expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    assert_eq!(validation.issues[0].kind, "invalid-display");
    assert_eq!(
        validation
            .response
            .display
            .as_ref()
            .and_then(|d| d.value.as_deref()),
        Some("Cat")
    );
    let synonym = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        display: Some("domestic  CAT".into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &synonym).expect("validates");
    assert_eq!(
        validation.response.result.value,
        Some(true),
        "a designation, whitespace and case folded"
    );
    let case = ValueSetValidateCodeRequest {
        url: Some(VS_ENUMERATED.into()),
        code: Some("red".into()),
        system: Some(COLOURS.into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &case).expect("validates");
    assert_eq!(
        validation.response.result.value,
        Some(true),
        "colours is case-insensitive"
    );
    assert_eq!(
        validation.code.as_deref(),
        Some("RED"),
        "the system's spelling is echoed"
    );
    let inactive = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        value_set_version: Some("1.0".into()),
        code: Some("dodo".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &inactive).expect("validates");
    assert_eq!(
        validation.response.result.value,
        Some(true),
        "inactive is not invalid"
    );
    assert_eq!(validation.issues[0].kind, "status-check");
    assert_eq!(validation.issues[0].severity, "warning");
    let abstract_refused = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        code: Some("pet".into()),
        system: Some(ANIMALS.into()),
        r#abstract: Some(false.into()),
        ..Default::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &abstract_refused)
        .expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    assert_eq!(validation.issues[0].kind, "code-rule");
}

#[test]
fn validate_code_refuses_malformed_requests() {
    let world = World::load();
    let none = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        ..Default::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &none),
        Err(OperationError::Invalid(_))
    ));
    let two = ValueSetValidateCodeRequest {
        url: Some(VS_ALL.into()),
        code: Some("cat".into()),
        coding: Some(Coding::default()),
        ..Default::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &two),
        Err(OperationError::Invalid(_))
    ));
    let no_value_set = ValueSetValidateCodeRequest {
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &no_value_set),
        Err(OperationError::Required(_))
    ));
    let unknown = ValueSetValidateCodeRequest {
        url: Some("http://example.org/fhir/ValueSet/nowhere".into()),
        code: Some("cat".into()),
        system: Some(ANIMALS.into()),
        ..Default::default()
    };
    assert!(matches!(
        value_set_validate_code::validate_code(&world.sources(), &unknown),
        Err(OperationError::UnknownValueSet(_))
    ));
    let ambiguous = ValueSetValidateCodeRequest {
        url: Some(VS_ENUMERATED.into()),
        code: Some("cat".into()),
        ..Default::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &ambiguous).expect("validates");
    assert_eq!(validation.response.result.value, Some(false));
    assert_eq!(validation.issues[0].kind, "cannot-infer");
}
