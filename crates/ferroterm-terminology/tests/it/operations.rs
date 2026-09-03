//! The three R4B operations over the synthetic provider: every parameter
//! branch the R4B definitions admit, and every refusal.

use std::sync::Arc;

use ferroterm_fhir::r4b::code_system::CodeSystem;
use ferroterm_fhir::r4b::codeable_concept::CodeableConcept;
use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::code_system_validate_code::CodeSystemValidateCodeRequest;
use ferroterm_fhir::r4b::parameters::ParametersParameterValue;
use ferroterm_terminology::operations::lookup::{LookupInput, lookup};
use ferroterm_terminology::operations::subsumes::{SubsumesInput, subsumes};
use ferroterm_terminology::operations::validate_code::validate_code;
use ferroterm_terminology::operations::{CodingRef, Invocation, OperationError};
use ferroterm_terminology::provider::PropertyValue;
use ferroterm_terminology::registry::Registry;
use http::StatusCode;

use crate::fixture::{FLAT_URL, URL, registry};

fn coding(system: Option<&str>, code: &str) -> Coding {
    Coding {
        system: system.map(std::convert::Into::into),
        code: Some(code.into()),
        ..Default::default()
    }
}

/// A coding as the engine names one.
fn coding_ref(system: Option<&str>, code: &str) -> CodingRef {
    CodingRef {
        system: system.map(str::to_owned),
        code: Some(code.to_owned()),
        ..CodingRef::default()
    }
}

fn instance(registry: &Registry, url: &str) -> Invocation {
    Invocation::Instance(registry.resolve(url, None).expect("resolves"))
}

// ---- $lookup ------------------------------------------------------------

#[test]
fn lookup_by_system_and_code_returns_name_version_display_designations_properties() {
    let registry = registry();
    let input = LookupInput {
        system: Some(URL.to_owned()),
        code: Some(String::from("cat")),
        ..LookupInput::default()
    };
    let outcome = lookup(&registry, &Invocation::Type, &input).expect("looks up");
    assert_eq!(outcome.name, "Fixture");
    assert_eq!(outcome.version.as_deref(), Some("2025"));
    assert_eq!(outcome.display, "Cat");
    let languages: Vec<&str> = outcome
        .designations
        .iter()
        .filter_map(|d| d.language.as_deref())
        .collect();
    assert_eq!(languages, ["en", "nl"]);
    let codes: Vec<&str> = outcome.properties.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(codes, ["legs", "kingdom"]);
    assert_eq!(outcome.properties[0].value, PropertyValue::Integer(4));
}

#[test]
fn lookup_by_coding_with_version_and_display_language() {
    let registry = registry();
    let input = LookupInput {
        coding: Some(CodingRef {
            system: Some(URL.to_owned()),
            version: Some(String::from("2024")),
            code: Some(String::from("dog")),
            display: None,
        }),
        display_language: Some(String::from("nl")),
        ..LookupInput::default()
    };
    let outcome = lookup(&registry, &Invocation::Type, &input).expect("looks up");
    assert_eq!(outcome.version.as_deref(), Some("2024"));
    assert_eq!(outcome.display, "Hond");
}

#[test]
fn lookup_property_selects_properties_and_lang_x_selects_designations() {
    let registry = registry();
    let input = LookupInput {
        system: Some(URL.to_owned()),
        code: Some(String::from("cat")),
        properties: vec![
            String::from("kingdom"),
            String::from("lang.nl"),
            String::from("display"),
        ],
        ..LookupInput::default()
    };
    let outcome = lookup(&registry, &Invocation::Type, &input).expect("looks up");
    let codes: Vec<&str> = outcome.properties.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(
        codes,
        ["kingdom"],
        "display is a named parameter, legs was not asked"
    );
    assert_eq!(outcome.designations.len(), 1);
    assert_eq!(outcome.designations[0].value, "Kat");
}

#[test]
fn lookup_refusals_carry_their_issue_code_and_status() {
    let registry = registry();
    let run = |input: LookupInput| lookup(&registry, &Invocation::Type, &input).err();
    // Nothing named.
    let error = run(LookupInput::default()).expect("refused");
    assert!(matches!(error, OperationError::Required(_)));
    assert_eq!(
        (error.issue_code(), error.status()),
        ("required", StatusCode::BAD_REQUEST)
    );
    // A code without a system.
    let error = run(LookupInput {
        code: Some(String::from("cat")),
        ..LookupInput::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::Required(_)));
    // Both forms at once.
    let error = run(LookupInput {
        code: Some(String::from("cat")),
        system: Some(URL.to_owned()),
        coding: Some(coding_ref(Some(URL), "cat")),
        ..LookupInput::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::Invalid(_)));
    // An unknown code.
    let error = run(LookupInput {
        code: Some(String::from("unicorn")),
        system: Some(URL.to_owned()),
        ..LookupInput::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::UnknownCode { ref code, .. } if code == "unicorn"));
    assert_eq!(
        (error.issue_code(), error.status()),
        ("not-found", StatusCode::BAD_REQUEST)
    );
    // An unknown system.
    let error = run(LookupInput {
        code: Some(String::from("cat")),
        system: Some(String::from("http://example.org/nowhere")),
        ..LookupInput::default()
    })
    .expect("refused");
    assert_eq!(
        (error.issue_code(), error.status()),
        ("not-found", StatusCode::NOT_FOUND)
    );
    // An unknown version.
    let error = run(LookupInput {
        code: Some(String::from("cat")),
        system: Some(URL.to_owned()),
        version: Some(String::from("1999")),
        ..LookupInput::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::UnknownVersion { .. }));
    // The instance level, which the definition does not declare.
    let error = lookup(
        &registry,
        &instance(&registry, URL),
        &LookupInput {
            code: Some(String::from("cat")),
            ..LookupInput::default()
        },
    )
    .expect_err("refused");
    assert!(matches!(error, OperationError::NotSupported(_)));
    assert_eq!(error.issue_code(), "not-supported");
}

// ---- $validate-code -------------------------------------------------------

#[test]
fn validate_code_by_code_coding_and_codeable_concept() {
    let registry = registry();
    let by_code = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("cat".into()),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(by_code.result.value, Some(true));
    assert!(by_code.message.is_none());
    assert_eq!(
        by_code.display.and_then(|d| d.value).as_deref(),
        Some("Cat")
    );

    let by_coding = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            coding: Some(coding(Some(URL), "dog")),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(by_coding.result.value, Some(true));

    // Any coding of the concept in the system validates; foreign codings are skipped.
    let concept = CodeableConcept {
        coding: vec![
            coding(Some("http://loinc.org"), "1234-5"),
            coding(Some(URL), "unicorn"),
            coding(Some(URL), "cat"),
        ],
        ..Default::default()
    };
    let by_concept = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            codeable_concept: Some(concept),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(by_concept.result.value, Some(true));
    let none = CodeableConcept {
        coding: vec![coding(Some("http://loinc.org"), "1234-5")],
        ..Default::default()
    };
    let by_none = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            codeable_concept: Some(none),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(by_none.result.value, Some(false));
}

#[test]
fn validate_code_wrong_display_unknown_code_and_inactive_code() {
    let registry = registry();
    let wrong = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("cat".into()),
            display: Some("Dog".into()),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(wrong.result.value, Some(false));
    assert!(
        wrong
            .message
            .as_ref()
            .and_then(|m| m.value.as_deref())
            .is_some_and(|m| m.contains("display"))
    );
    assert_eq!(wrong.display.and_then(|d| d.value).as_deref(), Some("Cat"));
    // A designation in another language is a valid display; the fixture is case-sensitive.
    let dutch = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("cat".into()),
            display: Some("Kat".into()),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(dutch.result.value, Some(true));
    let unknown = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("unicorn".into()),
            ..Default::default()
        },
    )
    .expect("an invalid code is a false result, not an error");
    assert_eq!(unknown.result.value, Some(false));
    assert!(unknown.display.is_none());
    let inactive = validate_code(
        &registry,
        &Invocation::Type,
        &CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("fish".into()),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(inactive.result.value, Some(true), "inactive is not invalid");
    assert!(
        inactive
            .message
            .as_ref()
            .and_then(|m| m.value.as_deref())
            .is_some_and(|m| m.contains("inactive"))
    );
}

#[test]
fn validate_code_refusals() {
    let registry = registry();
    let run = |request: CodeSystemValidateCodeRequest| {
        validate_code(&registry, &Invocation::Type, &request).expect_err("refused")
    };
    assert!(matches!(
        run(CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            ..Default::default()
        }),
        OperationError::Invalid(_)
    ));
    assert!(matches!(
        run(CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            code: Some("cat".into()),
            coding: Some(coding(Some(URL), "cat")),
            ..Default::default()
        }),
        OperationError::Invalid(_)
    ));
    assert!(matches!(
        run(CodeSystemValidateCodeRequest {
            code: Some("cat".into()),
            ..Default::default()
        }),
        OperationError::Required(_)
    ));
    assert!(matches!(
        run(CodeSystemValidateCodeRequest {
            url: Some(URL.into()),
            coding: Some(coding(Some("http://loinc.org"), "1")),
            ..Default::default()
        }),
        OperationError::Invalid(_)
    ));
    assert!(matches!(
        run(CodeSystemValidateCodeRequest {
            code_system: Some(CodeSystem::default()),
            code: Some("cat".into()),
            ..Default::default()
        }),
        OperationError::NotSupported(_)
    ));
    // Instance level: the instance is the system; a contradicting url is invalid.
    let ok = validate_code(
        &registry,
        &instance(&registry, URL),
        &CodeSystemValidateCodeRequest {
            code: Some("cat".into()),
            ..Default::default()
        },
    )
    .expect("validates");
    assert_eq!(ok.result.value, Some(true));
    let mismatch = validate_code(
        &registry,
        &instance(&registry, URL),
        &CodeSystemValidateCodeRequest {
            url: Some(FLAT_URL.into()),
            code: Some("cat".into()),
            ..Default::default()
        },
    )
    .expect_err("refused");
    assert!(matches!(mismatch, OperationError::Invalid(_)));
}

// ---- $subsumes ------------------------------------------------------------

#[test]
fn subsumes_outcomes_follow_the_closure() {
    let registry = registry();
    let run = |a: &str, b: &str| {
        subsumes(
            &registry,
            &Invocation::Type,
            &SubsumesInput {
                code_a: Some(a.to_owned()),
                code_b: Some(b.to_owned()),
                system: Some(URL.to_owned()),
                ..SubsumesInput::default()
            },
        )
        .expect("subsumes")
        .code()
    };
    assert_eq!(run("animal", "cat"), "subsumes");
    assert_eq!(run("cat", "animal"), "subsumed-by");
    assert_eq!(run("cat", "cat"), "equivalent");
    assert_eq!(run("cat", "dog"), "not-subsumed");
    let by_codings = subsumes(
        &registry,
        &Invocation::Type,
        &SubsumesInput {
            coding_a: Some(coding_ref(Some(URL), "root")),
            coding_b: Some(coding_ref(Some(URL), "kitten")),
            ..SubsumesInput::default()
        },
    )
    .expect("subsumes");
    assert_eq!(by_codings.code(), "subsumes");
    let on_instance = subsumes(
        &registry,
        &instance(&registry, URL),
        &SubsumesInput {
            code_a: Some(String::from("kitten")),
            code_b: Some(String::from("cat")),
            ..SubsumesInput::default()
        },
    )
    .expect("subsumes");
    assert_eq!(on_instance.code(), "subsumed-by");
}

#[test]
fn subsumes_refusals_are_errors_never_not_subsumed() {
    let registry = registry();
    let run =
        |input: SubsumesInput| subsumes(&registry, &Invocation::Type, &input).expect_err("refused");
    assert!(matches!(
        run(SubsumesInput::default()),
        OperationError::Required(_)
    ));
    // A code and a coding mixed.
    assert!(matches!(
        run(SubsumesInput {
            code_a: Some(String::from("cat")),
            coding_b: Some(coding_ref(Some(URL), "dog")),
            system: Some(URL.to_owned()),
            ..SubsumesInput::default()
        }),
        OperationError::Invalid(_)
    ));
    // Codes without a system.
    assert!(matches!(
        run(SubsumesInput {
            code_a: Some(String::from("cat")),
            code_b: Some(String::from("dog")),
            ..SubsumesInput::default()
        }),
        OperationError::Required(_)
    ));
    let unknown = run(SubsumesInput {
        code_a: Some(String::from("cat")),
        code_b: Some(String::from("unicorn")),
        system: Some(URL.to_owned()),
        ..SubsumesInput::default()
    });
    assert!(matches!(unknown, OperationError::UnknownCode { .. }));
    // Codings of two systems.
    let foreign = run(SubsumesInput {
        coding_a: Some(coding_ref(Some(URL), "cat")),
        coding_b: Some(coding_ref(Some("http://loinc.org"), "1")),
        ..SubsumesInput::default()
    });
    assert!(matches!(foreign, OperationError::NotSupported(_)));
    // A system without subsumption.
    let flat = run(SubsumesInput {
        code_a: Some(String::from("cat")),
        code_b: Some(String::from("dog")),
        system: Some(FLAT_URL.to_owned()),
        ..SubsumesInput::default()
    });
    assert!(matches!(flat, OperationError::NotSupported(_)));
    assert_eq!(flat.status(), StatusCode::BAD_REQUEST);
    let mut registry_without = Registry::new();
    registry_without
        .register(Arc::new(crate::fixture::Fixture::flat()))
        .expect("registers");
    assert!(matches!(
        subsumes(
            &registry_without,
            &Invocation::Type,
            &SubsumesInput {
                code_a: Some(String::from("a")),
                code_b: Some(String::from("b")),
                system: Some(URL.to_owned()),
                ..SubsumesInput::default()
            }
        ),
        Err(OperationError::UnknownSystem(_))
    ));
}
