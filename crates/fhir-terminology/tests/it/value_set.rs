//! `ValueSet/$expand` and `ValueSet/$validate-code` on R4B over the compose
//! layer (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>,
//! <https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).

use std::sync::Arc;

use ferroterm_testkit::fhir::{
    ANIMALS, ANIMALS_NL, COLOURS, VS_ALL, VS_COLOURS, VS_ENUMERATED, VS_LOOP_A, VS_PETS,
    VS_PETS_REF, write_code_systems,
};
use fhir_terminology::conceptmap::store::ConceptMapStore;
use fhir_terminology::fhir_codesystem::load::{FhirVersion, load_dir};
use fhir_terminology::fhir_codesystem::provider::FhirCodeSystem;
use fhir_terminology::operations::expand::{ExpandInput, ExpansionOutcome, ParameterValue};
use fhir_terminology::operations::value_set_validate_code::ValueSetValidateInput;
use fhir_terminology::operations::{CodingRef, Issue};
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
            } else {
                let target = model
                    .supplements
                    .clone()
                    .expect("a supplement names its system");
                registry.register_supplement(
                    target,
                    fhir_terminology::supplement::Supplement::from_code_system(&model),
                );
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
    // NOTE: `includeDesignations` lists every designation; `displayLanguage` chooses
    // the display alone (the ecosystem's language cases, #190).
    assert_eq!(
        cat.designations.len(),
        2,
        "every designation, whatever the display language"
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
}

#[test]
fn validate_code_names_an_unknown_system_and_an_unknown_version() {
    let world = World::load();
    let unknown_system = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(String::from("http://example.org/fhir/CodeSystem/nowhere")),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &unknown_system)
        .expect("validates");
    assert!(!validation.result);
    // NOTE: the membership issue leads, then the missing system, which the input
    // alone named (`x-unknown-system`), the ecosystem's shape (#189).
    assert_eq!(validation.issues[0].kind, "not-in-vs");
    assert_eq!(validation.issues[1].kind, "not-found");
    assert_eq!(validation.issues[1].code, "not-found");
    assert_eq!(
        validation.x_unknown_systems,
        ["http://example.org/fhir/CodeSystem/nowhere"],
        "x-unknown-system names the system"
    );
    assert_eq!(
        validation.code.as_deref(),
        Some("kitten"),
        "the code is echoed"
    );
    assert_eq!(
        validation.message.as_deref(),
        Some(
            "A definition for CodeSystem 'http://example.org/fhir/CodeSystem/nowhere' could not be found, so the code cannot be validated"
        )
    );
    let unknown_version = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        system_version: Some(String::from("9.9")),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &unknown_version)
        .expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.unknown_systems, [format!("{ANIMALS}|9.9")]);
    // NOTE: the code is still resolved against the version the value set uses, and
    // the versionless include's choice is a warning beside the not-found error
    // (the ecosystem's `version` cases, #177).
    assert_eq!(validation.version.as_deref(), Some("2.0"));
    assert_eq!(validation.display.as_deref(), Some("Kitten"));
    let shape: Vec<(&str, &str)> = validation
        .issues
        .iter()
        .map(|i| (i.severity, i.kind))
        .collect();
    assert_eq!(shape, [("error", "not-found"), ("warning", "vs-invalid")]);
    assert_eq!(validation.issues[0].expression.as_deref(), Some("system"));
    assert_eq!(validation.issues[1].expression.as_deref(), Some("version"));
    assert!(
        validation.issues[0].text.ends_with(". Valid versions: 2.0"),
        "{}",
        validation.issues[0].text
    );
    assert!(
        validation.issues[1]
            .text
            .contains("for the versionless include"),
        "{}",
        validation.issues[1].text
    );
}

// NOTE: a coding version that differs from the include's is validated against the
// include's version and itemised as vs-invalid; a check-system-version that
// forbids the include's version is a version-error, never a refusal (#177).
#[test]
fn validate_code_itemises_version_disagreements_against_the_value_set_version() {
    let world = World::load();
    let inline = |version: Option<&str>| ValueSet {
        url: Some("http://example.org/inline-pinned".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                version: version.map(Into::into),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let coding = |version: &str| CodingRef {
        system: Some(ANIMALS.to_owned()),
        version: Some(version.to_owned()),
        code: Some(String::from("cat")),
        display: None,
    };
    // An unserved coding version against a pinned include: vs-invalid, then not-found.
    let unserved = ValueSetValidateInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline(Some("2.0")))),
        coding: Some(coding("9.9")),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &unserved).expect("validates");
    let shape: Vec<(&str, &str)> = validation
        .issues
        .iter()
        .map(|i| (i.severity, i.kind))
        .collect();
    assert_eq!(shape, [("error", "vs-invalid"), ("error", "not-found")]);
    assert_eq!(validation.unknown_systems, [format!("{ANIMALS}|9.9")]);
    assert_eq!(validation.display.as_deref(), Some("Cat"));
    assert_eq!(
        validation.version.as_deref(),
        Some("2.0"),
        "the include's version answers"
    );
    assert_eq!(
        validation.issues[0].text,
        format!(
            "The code system '{ANIMALS}' version '2.0' in the ValueSet include is different to the one in the value ('9.9')"
        )
    );
    assert_eq!(
        validation.issues[0].expression.as_deref(),
        Some("Coding.version")
    );
    assert!(
        validation
            .message
            .as_deref()
            .unwrap()
            .starts_with("A definition for CodeSystem"),
        "the not-found text leads the message: {:?}",
        validation.message
    );
    // A check that forbids the include's version: a version-error, result false.
    let checked = ValueSetValidateInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline(Some("2.0")))),
        coding: Some(coding("2.0")),
        check_system_version: vec![format!("{ANIMALS}|1.x")],
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &checked).expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "version-error");
    assert_eq!(validation.issues[0].code, "exception");
    assert_eq!(
        validation.issues[0].text,
        format!(
            "The version '2.0' is not allowed for system '{ANIMALS}': required to be '1.x' by a version-check parameter"
        )
    );
    assert_eq!(
        validation.message.as_deref(),
        Some(validation.issues[0].text.as_str())
    );
    // On $expand the same check is refused as an exception.
    let expand = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline(Some("2.0")))),
        check_system_version: vec![format!("{ANIMALS}|1.x")],
        ..ExpandInput::default()
    };
    let error = expand::expand(&world.sources(), &expand).expect_err("refused");
    assert!(
        matches!(error, OperationError::VersionCheck(_)),
        "{error:?}"
    );
    assert_eq!(error.issue_code(), "exception");
    assert_eq!(error.tx_issue_type(), "version-error");
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
    assert_eq!(validation.issues[0].kind, "code-comment");
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
    // The ecosystem's shape: not in the value set, then the system undetermined.
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["not-in-vs", "cannot-infer"]);
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
    // NOTE: the ecosystem does not echo `property` among the expansion parameters (#190).
    assert!(parameter(&by_code, "property").is_empty());
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

// NOTE: the value set version trio and the system trio negotiate the versions an
// operation touches, top-level and imported alike
// (<https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-expand.html>,
// <https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-validate-code.html>).
#[test]
fn expand_negotiates_value_set_versions_and_names_the_value_sets_it_used() {
    let world = World::load();
    let pinned = ExpandInput {
        url: Some(VS_PETS_REF.to_owned()),
        default_valueset_version: vec![format!("{VS_PETS}|1.0")],
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &pinned).expect("expands");
    let echoed: Vec<(&str, String)> = vs
        .parameters
        .iter()
        .filter_map(|p| match &p.value {
            ParameterValue::Uri(u) => Some((p.name.as_str(), u.clone())),
            _ => None,
        })
        .collect();
    assert!(echoed.contains(&("default-valueset-version", format!("{VS_PETS}|1.0"))));
    assert!(
        echoed.contains(&("used-valueset", format!("{VS_PETS}|1.0"))),
        "{echoed:?}"
    );
    let missing = ExpandInput {
        url: Some(VS_PETS_REF.to_owned()),
        default_valueset_version: vec![format!("{VS_PETS}|9.9")],
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &missing),
        Err(OperationError::UnknownImport(url)) if url == format!("{VS_PETS}|9.9")
    ));
    let forced = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        force_valueset_version: vec![format!("{VS_ALL}|1.0")],
        ..ExpandInput::default()
    };
    let vs = expand::expand(&world.sources(), &forced).expect("expands");
    assert!(
        codes(&vs).iter().any(|c| c == "dodo"),
        "1.0 has no exclude, so the forced version keeps the dodo: {:?}",
        codes(&vs)
    );
    let checked = ExpandInput {
        url: Some(VS_ALL.to_owned()),
        value_set_version: Some(String::from("2.0")),
        check_valueset_version: vec![format!("{VS_ALL}|1.0")],
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &checked),
        Err(OperationError::Invalid(_))
    ));
}

#[test]
fn validate_code_negotiates_system_and_value_set_versions() {
    let world = World::load();
    let defaulted = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        default_system_version: vec![format!("{ANIMALS}|2.0")],
        check_system_version: vec![format!("{ANIMALS}|2.0")],
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &defaulted).expect("validates");
    assert!(validation.result);
    assert_eq!(validation.version.as_deref(), Some("2.0"));
    let forced_away = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        force_system_version: vec![format!("{ANIMALS}|9.9")],
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &forced_away)
        .expect("a false result");
    assert!(!validation.result);
    assert_eq!(validation.unknown_systems, [format!("{ANIMALS}|9.9")]);
    let mismatch = ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        coding: Some(CodingRef {
            system: Some(ANIMALS.to_owned()),
            version: Some(String::from("2.0")),
            code: Some(String::from("kitten")),
            display: None,
        }),
        check_system_version: vec![format!("{ANIMALS}|1.0")],
        ..ValueSetValidateInput::default()
    };
    // NOTE: on a versionless include the check acts as the default, so a version
    // the server lacks is the unresolvable-include shape, never a refusal (#177).
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &mismatch).expect("validates");
    assert!(!validation.result);
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["vs-invalid", "not-found"]);
    assert_eq!(validation.unknown_systems, [format!("{ANIMALS}|1.0")]);
    let imported = ValueSetValidateInput {
        url: Some(VS_PETS_REF.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        default_valueset_version: vec![format!("{VS_PETS}|1.0")],
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &imported).expect("validates");
    assert!(validation.result);
    let unknown_import = ValueSetValidateInput {
        url: Some(VS_PETS_REF.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        default_valueset_version: vec![format!("{VS_PETS}|9.9")],
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &unknown_import)
        .expect("a false result, not an error");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "not-found");
    assert_eq!(
        validation.message.as_deref(),
        Some(format!("A definition for the value Set '{VS_PETS}|9.9' could not be found").as_str())
    );
    assert_eq!(validation.code.as_deref(), Some("kitten"));
}

// NOTE: `inferSystem` finds a bare code's system by its membership in the value
// set, and `lenient-display-validation` keeps a wrong display from failing the
// result (<https://hl7.org/fhir/6.0.0-ballot5/valueset-operation-validate-code.html>).
#[test]
fn validate_code_infers_the_system_by_membership_and_can_be_lenient_on_displays() {
    let world = World::load();
    let bare = |code: &str, infer: Option<bool>| ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        code: Some(code.to_owned()),
        infer_system: infer,
        ..ValueSetValidateInput::default()
    };
    let red = value_set_validate_code::validate_code(&world.sources(), &bare("RED", Some(true)))
        .expect("validates");
    assert!(red.result);
    assert_eq!(red.system.as_deref(), Some(COLOURS));
    let cat = value_set_validate_code::validate_code(&world.sources(), &bare("cat", Some(true)))
        .expect("validates");
    assert_eq!(cat.system.as_deref(), Some(ANIMALS));
    let dodo = value_set_validate_code::validate_code(&world.sources(), &bare("dodo", Some(true)))
        .expect("a false result");
    assert!(!dodo.result);
    let kinds: Vec<&str> = dodo.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["not-in-vs", "cannot-infer"]);
    assert_eq!(dodo.issues[1].code, "not-found");
    assert_eq!(dodo.code.as_deref(), Some("dodo"));
    assert_eq!(
        dodo.message.as_deref(),
        Some(format!("The System URI could not be determined for the code 'dodo' in the ValueSet '{VS_ENUMERATED}|1.0'").as_str())
    );
    // Without inferSystem a bare code over two systems cannot be placed.
    let ambiguous = value_set_validate_code::validate_code(&world.sources(), &bare("cat", None))
        .expect("a false result");
    assert!(!ambiguous.result);
    assert_eq!(ambiguous.issues[1].kind, "cannot-infer");
    let wrong_display = |lenient: Option<bool>| ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        display: Some(String::from("Puppy")),
        lenient_display_validation: lenient,
        ..ValueSetValidateInput::default()
    };
    let strict = value_set_validate_code::validate_code(&world.sources(), &wrong_display(None))
        .expect("validates");
    assert!(!strict.result);
    assert_eq!(strict.issues[0].severity, "error");
    let lenient =
        value_set_validate_code::validate_code(&world.sources(), &wrong_display(Some(true)))
            .expect("validates");
    assert!(lenient.result, "a warning does not fail the result");
    assert_eq!(lenient.issues[0].severity, "warning");
    assert_eq!(lenient.issues[0].kind, "invalid-display");
    assert_eq!(lenient.display.as_deref(), Some("Kitten"));
}

// NOTE: an include naming a version the server does not serve fails the
// validation as an invalid value set naming the system (the ecosystem's test
// cases, `version/coding-v10-vs1wb`); `$expand` keeps refusing it.
#[test]
fn validate_code_fails_over_an_include_the_server_cannot_resolve_and_names_the_system() {
    let world = World::load();
    let inline = ValueSet {
        url: Some("http://example.org/inline-bad-version".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                version: Some("9".into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let input = ValueSetValidateInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &input).expect("a false result");
    assert!(!validation.result);
    // NOTE: without a subject version only the include's unknown version is itemised
    // (the ecosystem's `coding-vnn-vs1wb`); with one, the disagreement comes first.
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["not-found"]);
    assert_eq!(validation.unknown_systems, [format!("{ANIMALS}|9")]);
    assert_eq!(validation.code.as_deref(), Some("kitten"));
    assert_eq!(validation.display.as_deref(), Some("Kitten"));
    assert_eq!(
        validation.version.as_deref(),
        Some("2.0"),
        "the served default answers"
    );
    let with_version = ValueSetValidateInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        system_version: Some(String::from("2.0")),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &with_version)
        .expect("a false result");
    let kinds: Vec<&str> = validation.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["vs-invalid", "not-found"]);
    assert_eq!(
        validation.version.as_deref(),
        Some("2.0"),
        "the subject's served version"
    );
    let expand = ExpandInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&world.sources(), &expand),
        Err(OperationError::UnknownVersion { .. })
    ));
    // A wildcard include version names the greatest match.
    let wild = ValueSet {
        url: Some("http://example.org/inline-wild".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                version: Some("2.x".into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let input = ValueSetValidateInput {
        inline_value_set: Some(valueset::convert::r4b::convert(&wild)),
        code: Some(String::from("kitten")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &input).expect("validates");
    assert!(validation.result);
    assert_eq!(validation.version.as_deref(), Some("2.0"));
}

// NOTE: a CodeableConcept is judged coding by coding, the ecosystem's shape (its
// test cases): the first coding in the value set answers the code outputs, the
// input is echoed, and issues name the coding at fault by index.
#[test]
fn validate_code_judges_a_codeable_concept_coding_by_coding() {
    let world = World::load();
    let coding = |system: &str, code: &str, display: Option<&str>| CodingRef {
        system: Some(system.to_owned()),
        version: None,
        code: Some(code.to_owned()),
        display: display.map(str::to_owned),
    };
    let mixed = ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        codeable_concept: Some(vec![
            coding(COLOURS, "blue", None),
            coding(ANIMALS, "cat", None),
        ]),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &mixed).expect("validates");
    assert!(validation.result, "{validation:?}");
    assert_eq!(validation.code.as_deref(), Some("cat"));
    assert_eq!(validation.system.as_deref(), Some(ANIMALS));
    assert_eq!(
        validation.codeable_concept.as_ref().map(Vec::len),
        Some(2),
        "echoed"
    );
    let kinds: Vec<(&str, &str, Option<&str>)> = validation
        .issues
        .iter()
        .map(|i| (i.severity, i.kind, i.expression.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [(
            "information",
            "this-code-not-in-vs",
            Some("CodeableConcept.coding[0].code")
        )]
    );
    let wrong_display = ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        codeable_concept: Some(vec![coding(ANIMALS, "cat", Some("Hamster"))]),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&world.sources(), &wrong_display)
        .expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.code.as_deref(), Some("cat"));
    assert_eq!(
        validation.issues[0].expression.as_deref(),
        Some("CodeableConcept.coding[0].display")
    );
    let none = ValueSetValidateInput {
        url: Some(VS_ENUMERATED.to_owned()),
        codeable_concept: Some(vec![
            coding(ANIMALS, "dodo", None),
            coding("http://example.org/none", "x", None),
        ]),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&world.sources(), &none).expect("a false result");
    assert!(!validation.result);
    assert_eq!(
        validation.code, None,
        "no coding in the value set answers the code outputs"
    );
    assert_eq!(validation.system, None);
    let kinds: Vec<(&str, &str, Option<&str>)> = validation
        .issues
        .iter()
        .map(|i| (i.severity, i.kind, i.expression.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("error", "not-in-vs", None),
            (
                "error",
                "not-found",
                Some("CodeableConcept.coding[1].system")
            ),
            (
                "information",
                "this-code-not-in-vs",
                Some("CodeableConcept.coding[0].code")
            ),
            (
                "information",
                "this-code-not-in-vs",
                Some("CodeableConcept.coding[1].code")
            ),
        ]
    );
    assert_eq!(validation.x_unknown_systems, ["http://example.org/none"]);
    assert!(validation.unknown_systems.is_empty());
    assert_eq!(
        validation.message.as_deref(),
        Some(format!("No valid coding was found for the value set '{VS_ENUMERATED}|1.0'").as_str())
    );
}

// NOTE: the issue texts follow the ecosystem's test cases (spec-silent, #183), so
// a validator shows its user the wording the reference server would.
#[test]
fn validate_code_speaks_the_ecosystems_words_for_displays_codes_and_membership() {
    let world = World::load();
    let cat = |display: &str, language: Option<&str>| ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        display: Some(display.to_owned()),
        display_language: language.map(str::to_owned),
        ..ValueSetValidateInput::default()
    };
    let run = |input: &ValueSetValidateInput| {
        value_set_validate_code::validate_code(&world.sources(), input).expect("validates")
    };
    // No displayLanguage: every display of the concept is valid, each with its language.
    let wrong = run(&cat("Hamster", None));
    assert!(!wrong.result);
    assert_eq!(
        wrong.issues[0].text,
        format!(
            "Wrong Display Name 'Hamster' for {ANIMALS}#cat. Valid display is one of 3 choices: 'Cat' (en), 'Domestic cat' (en) or 'Katze' (de) (for the language(s) '--')"
        )
    );
    assert_eq!(
        wrong.message.as_deref(),
        Some(wrong.issues[0].text.as_str())
    );
    // A displayLanguage with displays: only those count.
    assert!(run(&cat("Katze", Some("de"))).result);
    let english_in_german = run(&cat("Cat", Some("de")));
    assert!(!english_in_german.result);
    assert_eq!(
        english_in_german.issues[0].text,
        format!(
            "Wrong Display Name 'Cat' for {ANIMALS}#cat. Valid display is 'Katze' (de) (for the language(s) 'de')"
        )
    );
    // A displayLanguage without displays: the default language's are a note, others an error.
    let default_only = run(&cat("Cat", Some("nl")));
    assert!(default_only.result);
    assert_eq!(default_only.issues[0].severity, "information");
    assert_eq!(
        default_only.issues[0].text,
        format!(
            "There are no valid display names found for the code {ANIMALS}#cat for language(s) 'nl'. The display is 'Cat' which is a valid display for the default language"
        )
    );
    let nothing = run(&cat("Hamster", Some("nl")));
    assert!(!nothing.result);
    assert_eq!(
        nothing.issues[0].text,
        format!(
            "Wrong Display Name 'Hamster' for {ANIMALS}#cat. There are no valid display names found for language(s) 'nl'. Default display is 'Cat'"
        )
    );
}

#[test]
fn validate_code_speaks_the_ecosystems_words_for_codes_membership_abstract_and_case() {
    let world = World::load();
    let run = |input: &ValueSetValidateInput| {
        value_set_validate_code::validate_code(&world.sources(), input).expect("validates")
    };
    // An unknown code, and a code outside the value set with its asserted display.
    let unknown = run(&ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("unicorn")),
        system: Some(ANIMALS.to_owned()),
        ..ValueSetValidateInput::default()
    });
    let kinds: Vec<(&str, String)> = unknown
        .issues
        .iter()
        .map(|i| (i.kind, i.text.clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            (
                "not-in-vs",
                format!(
                    "The provided code '{ANIMALS}#unicorn' was not found in the value set '{VS_ALL}|2.0'"
                )
            ),
            (
                "invalid-code",
                format!("Unknown code 'unicorn' in the CodeSystem '{ANIMALS}' version '2.0'")
            ),
        ]
    );
    let outside = run(&ValueSetValidateInput {
        url: Some(VS_PETS.to_owned()),
        code: Some(String::from("dog")),
        system: Some(ANIMALS.to_owned()),
        display: Some(String::from("Doggo")),
        ..ValueSetValidateInput::default()
    });
    assert_eq!(
        outside.issues[0].text,
        format!(
            "The provided code '{ANIMALS}#dog ('Doggo')' was not found in the value set '{VS_PETS}|1.0'"
        )
    );
    // An abstract code refused: the rule, then the membership, the rule as message.
    let abstract_code = run(&ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("living")),
        system: Some(ANIMALS.to_owned()),
        abstract_ok: Some(false),
        ..ValueSetValidateInput::default()
    });
    let kinds: Vec<&str> = abstract_code.issues.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, ["code-rule", "not-in-vs"]);
    assert_eq!(
        abstract_code.message.as_deref(),
        Some(
            format!("Code '{ANIMALS}#living' is abstract, and not allowed in this context")
                .as_str()
        )
    );
    // A case-insensitive system located the code under another spelling: a warning.
    let cased = run(&ValueSetValidateInput {
        url: Some(VS_COLOURS.to_owned()),
        code: Some(String::from("red")),
        system: Some(COLOURS.to_owned()),
        ..ValueSetValidateInput::default()
    });
    assert!(cased.result, "{cased:?}");
    assert_eq!(cased.code.as_deref(), Some("RED"));
    assert_eq!(cased.issues[0].kind, "code-rule");
    assert_eq!(cased.issues[0].severity, "warning");
    assert_eq!(
        cased.issues[0].text,
        format!(
            "The code 'red' differs from the correct code 'RED' by case. Although the code system '{COLOURS}' is case insensitive, implementers are strongly encouraged to use the correct case anyway"
        )
    );
}

// NOTE: a loaded supplement applies only when the request names it in
// `useSupplement` or the value set asks for it (the ecosystem's requirements,
// #184); a supplement is never a code system of its own.
#[test]
fn supplements_apply_only_when_named() {
    let world = World::load();
    let kat = |supplements: Vec<String>| ValueSetValidateInput {
        url: Some(VS_ALL.to_owned()),
        code: Some(String::from("cat")),
        system: Some(ANIMALS.to_owned()),
        display: Some(String::from("Kat")),
        use_supplement: supplements,
        ..ValueSetValidateInput::default()
    };
    let dormant = value_set_validate_code::validate_code(&world.sources(), &kat(Vec::new()))
        .expect("validates");
    assert!(
        !dormant.result,
        "the Dutch designation comes from the supplement: {dormant:?}"
    );
    assert_eq!(dormant.issues[0].kind, "invalid-display");
    let applied =
        value_set_validate_code::validate_code(&world.sources(), &kat(vec![ANIMALS_NL.to_owned()]))
            .expect("validates");
    assert!(applied.result, "{applied:?}");
    let versioned = value_set_validate_code::validate_code(
        &world.sources(),
        &kat(vec![format!("{ANIMALS_NL}|1")]),
    )
    .expect("validates");
    assert!(versioned.result);
    let unknown = value_set_validate_code::validate_code(
        &world.sources(),
        &kat(vec![String::from(
            "http://example.org/fhir/CodeSystem/no-such-supplement",
        )]),
    )
    .expect_err("refused");
    assert!(
        matches!(unknown, OperationError::UnknownSupplement(_)),
        "{unknown:?}"
    );
    assert_eq!(unknown.issue_code(), "not-found");
    assert_eq!(
        unknown.to_string(),
        "Required supplement not found: http://example.org/fhir/CodeSystem/no-such-supplement"
    );
}

#[test]
fn a_value_set_asks_for_its_supplement_and_a_supplement_is_no_code_system() {
    let world = World::load();
    // A value set that asks for the supplement applies it by itself.
    let asking = ValueSet {
        url: Some("http://example.org/inline-asking".into()),
        status: "draft".into(),
        extension: vec![fhir_types::r4b::extension::Extension {
            url: "http://hl7.org/fhir/StructureDefinition/valueset-supplement".into(),
            value: Some(fhir_types::r4b::extension::ExtensionValue::Canonical(
                ANIMALS_NL.into(),
            )),
            ..Default::default()
        }],
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let by_value_set = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            inline_value_set: Some(valueset::convert::r4b::convert(&asking)),
            code: Some(String::from("cat")),
            system: Some(ANIMALS.to_owned()),
            display: Some(String::from("Kat")),
            ..ValueSetValidateInput::default()
        },
    )
    .expect("validates");
    assert!(by_value_set.result, "{by_value_set:?}");
    // The supplement's own URL is no code system.
    let as_system = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            url: Some(VS_ALL.to_owned()),
            code: Some(String::from("cat")),
            system: Some(ANIMALS_NL.to_owned()),
            ..ValueSetValidateInput::default()
        },
    )
    .expect("a false result");
    assert!(!as_system.result);
    assert_eq!(as_system.issues[0].kind, "invalid-data");
    assert_eq!(
        as_system.issues[0].text,
        format!(
            "CodeSystem {ANIMALS_NL}|1 is a supplement, so can't be used as a value in Coding.system"
        )
    );
    assert_eq!(as_system.unknown_systems, [ANIMALS_NL.to_owned()]);
    // $expand applies a named supplement to the designations it returns.
    let kat_in = |vs: &fhir_terminology::operations::expand::ExpansionOutcome| {
        vs.contains
            .iter()
            .any(|c| c.designations.iter().any(|d| d.value == "Kat"))
    };
    let expanded = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_ALL.to_owned()),
            include_designations: Some(true),
            use_supplement: vec![ANIMALS_NL.to_owned()],
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    assert!(
        kat_in(&expanded),
        "the named supplement's designation is in the expansion"
    );
    let plain = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_ALL.to_owned()),
            include_designations: Some(true),
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    assert!(!kat_in(&plain), "a dormant supplement adds nothing");
}

// NOTE: an inactive concept validates with `inactive`, `status`, and a code-comment
// warning; a draft, experimental, deprecated, or withdrawn resource earns a
// status-check note (the ecosystem's requirements, #185).
#[test]
fn validate_code_reports_inactive_concepts_and_resource_standing() {
    let world = World::load();
    let run = |code: &str| {
        value_set_validate_code::validate_code(
            &world.sources(),
            &ValueSetValidateInput {
                url: Some(VS_ALL.to_owned()),
                value_set_version: Some(String::from("1.0")),
                coding: Some(CodingRef {
                    system: Some(ANIMALS.to_owned()),
                    version: None,
                    code: Some(code.to_owned()),
                    display: None,
                }),
                ..ValueSetValidateInput::default()
            },
        )
        .expect("validates")
    };
    let active = run("cat");
    assert_eq!((active.inactive, active.status.as_deref()), (None, None));
    assert!(active.issues.is_empty(), "{active:?}");
    let retired = run("fish");
    assert!(retired.result, "inactive is a warning");
    assert_eq!(retired.inactive, Some(true));
    assert_eq!(retired.status.as_deref(), Some("retired"));
    assert_eq!(retired.issues.len(), 1);
    assert_eq!(
        (retired.issues[0].severity, retired.issues[0].kind),
        ("warning", "code-comment")
    );
    assert_eq!(
        retired.issues[0].text,
        "The concept 'fish' has a status of retired and inactive and its use should be reviewed"
    );
    assert_eq!(retired.issues[0].expression.as_deref(), Some("Coding"));
    assert_eq!(
        retired.message.as_deref(),
        Some(retired.issues[0].text.as_str())
    );
    let flagged = run("dodo");
    assert_eq!(flagged.inactive, Some(true));
    assert_eq!(
        flagged.status, None,
        "no status property, only the inactive flag"
    );
    assert_eq!(
        flagged.issues[0].text,
        "The concept 'dodo' has a status of inactive and its use should be reviewed"
    );
}

#[test]
fn a_draft_experimental_deprecated_or_withdrawn_resource_earns_a_status_note() {
    use fhir_terminology::operations::standing_note;
    use fhir_terminology::provider::Standing;
    let world = World::load();
    let draft = Standing {
        status: String::from("draft"),
        ..Standing::default()
    };
    let note = standing_note("CodeSystem", "http://example.org/cs|1", &draft).expect("noted");
    assert_eq!((note.severity, note.kind), ("information", "status-check"));
    assert_eq!(
        note.text,
        "Reference to draft CodeSystem http://example.org/cs|1"
    );
    let experimental = Standing {
        experimental: true,
        ..Standing::default()
    };
    assert_eq!(
        standing_note("CodeSystem", "http://example.org/cs|1", &experimental)
            .expect("noted")
            .text,
        "Reference to experimental CodeSystem http://example.org/cs|1"
    );
    let deprecated = Standing {
        standards_status: Some(String::from("deprecated")),
        ..Standing::default()
    };
    assert_eq!(
        standing_note("CodeSystem", "http://example.org/cs|1", &deprecated)
            .expect("noted")
            .text,
        "Reference to deprecated CodeSystem http://example.org/cs|1"
    );
    assert!(standing_note("CodeSystem", "x", &Standing::default()).is_none());
    // A withdrawn value set is noted on every validation against it.
    let withdrawn = ValueSet {
        url: Some("http://example.org/inline-withdrawn".into()),
        version: Some("1".into()),
        status: "active".into(),
        extension: vec![fhir_types::r4b::extension::Extension {
            url: "http://hl7.org/fhir/StructureDefinition/structuredefinition-standards-status"
                .into(),
            value: Some(fhir_types::r4b::extension::ExtensionValue::Code(
                "withdrawn".into(),
            )),
            ..Default::default()
        }],
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let noted = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            inline_value_set: Some(valueset::convert::r4b::convert(&withdrawn)),
            code: Some(String::from("cat")),
            system: Some(ANIMALS.to_owned()),
            ..ValueSetValidateInput::default()
        },
    )
    .expect("validates");
    assert!(noted.result);
    let shape: Vec<(&str, &str, &str)> = noted
        .issues
        .iter()
        .map(|i| (i.severity, i.kind, i.text.as_str()))
        .collect();
    assert_eq!(
        shape,
        [(
            "information",
            "status-check",
            "Reference to withdrawn ValueSet http://example.org/inline-withdrawn|1"
        )]
    );
    assert_eq!(noted.message, None, "notes carry no message");
}

// NOTE: every issue carries the ecosystem's message id, the message joins the
// errors in the ecosystem's order, and an unknown system fails with the
// membership issue first (the ecosystem's test cases, #189).
#[test]
fn validate_code_names_its_messages_and_shapes_the_unknown_system_answer() {
    let world = World::load();
    let unknown = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            url: Some(VS_ALL.to_owned()),
            code: Some(String::from("x")),
            system: Some(String::from("http://example.org/fhir/CodeSystem/nowhere")),
            ..ValueSetValidateInput::default()
        },
    )
    .expect("a false result");
    let shape: Vec<(&str, &str, Option<&str>)> = unknown
        .issues
        .iter()
        .map(|i| (i.kind, i.message_id(), i.expression.as_deref()))
        .collect();
    assert_eq!(
        shape,
        [
            (
                "not-in-vs",
                "None_of_the_provided_codes_are_in_the_value_set_one",
                Some("code")
            ),
            ("not-found", "UNKNOWN_CODESYSTEM", Some("system")),
        ]
    );
    assert_eq!(
        unknown.x_unknown_systems,
        ["http://example.org/fhir/CodeSystem/nowhere"]
    );
    assert!(
        unknown.unknown_systems.is_empty(),
        "the value set never named the system"
    );
    assert!(
        unknown
            .message
            .as_deref()
            .unwrap()
            .starts_with("A definition for CodeSystem"),
        "{:?}",
        unknown.message
    );
    // A system the value set does name is caused by the value set.
    let inline = ValueSet {
        url: Some("http://example.org/inline-unknown".into()),
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
    let caused = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            inline_value_set: Some(valueset::convert::r4b::convert(&inline)),
            code: Some(String::from("x")),
            system: Some(String::from("http://example.org/fhir/CodeSystem/nowhere")),
            ..ValueSetValidateInput::default()
        },
    )
    .expect("a false result");
    assert_eq!(
        caused.unknown_systems,
        ["http://example.org/fhir/CodeSystem/nowhere"]
    );
    assert!(caused.x_unknown_systems.is_empty());
}

#[test]
fn the_message_joins_the_errors_in_the_ecosystems_order() {
    let world = World::load();
    let pinned = ValueSet {
        url: Some("http://example.org/inline-pinned".into()),
        status: "draft".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(ANIMALS.into()),
                version: Some("2.0".into()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let checked = value_set_validate_code::validate_code(
        &world.sources(),
        &ValueSetValidateInput {
            inline_value_set: Some(valueset::convert::r4b::convert(&pinned)),
            coding: Some(CodingRef {
                system: Some(ANIMALS.to_owned()),
                version: Some(String::from("9.9")),
                code: Some(String::from("cat")),
                display: None,
            }),
            check_system_version: vec![format!("{ANIMALS}|1.x")],
            ..ValueSetValidateInput::default()
        },
    )
    .expect("a false result");
    let ids: Vec<&str> = checked.issues.iter().map(Issue::message_id).collect();
    assert_eq!(
        ids,
        [
            "VALUESET_VERSION_CHECK",
            "VALUESET_VALUE_MISMATCH",
            "UNKNOWN_CODESYSTEM_VERSION"
        ]
    );
    let message = checked.message.as_deref().unwrap();
    let order = [
        message.find("A definition for CodeSystem").unwrap(),
        message.find("The code system").unwrap(),
        message.find("The version").unwrap(),
    ];
    assert!(order[0] < order[1] && order[1] < order[2], "{message}");
}

// NOTE: an expansion lists every designation unless asked for some, flags an
// inactive concept with its status, and echoes neither `property` nor a
// language it was not asked for (the ecosystem's `$expand` cases, #190).
#[test]
fn expand_lists_every_designation_and_flags_inactive_concepts() {
    let world = World::load();
    let vs = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_ALL.to_owned()),
            value_set_version: Some(String::from("1.0")),
            include_designations: Some(true),
            property: vec![String::from("legs")],
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    let names: Vec<&str> = vs.parameters.iter().map(|p| p.name.as_str()).collect();
    assert!(!names.contains(&"property"), "{names:?}");
    assert!(
        !names.contains(&"displayLanguage"),
        "no language is echoed when none was asked for: {names:?}"
    );
    let fish = vs.contains.iter().find(|c| c.code == "fish").expect("fish");
    assert!(fish.inactive);
    let codes: Vec<&str> = fish.properties.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(
        codes,
        ["legs", "status"],
        "the status property rides along unasked"
    );
    assert_eq!(
        fish.properties[1].value,
        PropertyValue::Code(String::from("retired"))
    );
    let cat = vs.contains.iter().find(|c| c.code == "cat").expect("cat");
    let mut languages: Vec<Option<&str>> = cat
        .designations
        .iter()
        .map(|d| d.language.as_deref())
        .collect();
    languages.sort_unstable();
    assert_eq!(
        languages,
        [Some("de"), Some("en")],
        "every designation, whatever the display language"
    );
    let german = expand::expand(
        &world.sources(),
        &ExpandInput {
            url: Some(VS_ALL.to_owned()),
            value_set_version: Some(String::from("1.0")),
            include_designations: Some(true),
            display_language: Some(String::from("de")),
            ..ExpandInput::default()
        },
    )
    .expect("expands");
    let cat = german
        .contains
        .iter()
        .find(|c| c.code == "cat")
        .expect("cat");
    assert_eq!(
        cat.designations.len(),
        2,
        "displayLanguage narrows nothing: {:?}",
        cat.designations
    );
    let names: Vec<&str> = german.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "displayLanguage").count(),
        1,
        "the requested language once"
    );
}
