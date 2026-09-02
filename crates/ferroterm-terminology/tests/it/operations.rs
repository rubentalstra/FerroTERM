//! The three R4B operations over the synthetic provider: every parameter
//! branch the R4B definitions admit, and every refusal.

use std::sync::Arc;

use ferroterm_fhir::r4b::code_system::CodeSystem;
use ferroterm_fhir::r4b::codeable_concept::CodeableConcept;
use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::code_system_lookup::CodeSystemLookupRequest;
use ferroterm_fhir::r4b::operations::code_system_subsumes::CodeSystemSubsumesRequest;
use ferroterm_fhir::r4b::operations::code_system_validate_code::CodeSystemValidateCodeRequest;
use ferroterm_fhir::r4b::parameters::ParametersParameterValue;
use ferroterm_terminology::operations::lookup::lookup;
use ferroterm_terminology::operations::subsumes::subsumes;
use ferroterm_terminology::operations::validate_code::validate_code;
use ferroterm_terminology::operations::{Invocation, OperationError};
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

fn instance(registry: &Registry, url: &str) -> Invocation {
    Invocation::Instance(registry.resolve(url, None).expect("resolves"))
}

// ---- $lookup ------------------------------------------------------------

#[test]
fn lookup_by_system_and_code_returns_name_version_display_designations_properties() {
    let registry = registry();
    let request = CodeSystemLookupRequest {
        system: Some(URL.into()),
        code: Some("cat".into()),
        ..Default::default()
    };
    let response = lookup(&registry, &Invocation::Type, &request).expect("looks up");
    assert_eq!(response.name.value.as_deref(), Some("Fixture"));
    assert_eq!(
        response.version.and_then(|v| v.value).as_deref(),
        Some("2025")
    );
    assert_eq!(response.display.value.as_deref(), Some("Cat"));
    let languages: Vec<&str> = response
        .designation
        .iter()
        .filter_map(|d| d.language.as_ref().and_then(|l| l.value.as_deref()))
        .collect();
    assert_eq!(languages, ["en", "nl"]);
    let codes: Vec<&str> = response
        .property
        .iter()
        .filter_map(|p| p.code.value.as_deref())
        .collect();
    assert_eq!(codes, ["legs", "kingdom"]);
    assert_eq!(
        response.property[0].value,
        Some(ParametersParameterValue::Integer(4.into()))
    );
}

#[test]
fn lookup_by_coding_with_version_and_display_language() {
    let registry = registry();
    let request = CodeSystemLookupRequest {
        coding: Some(Coding {
            system: Some(URL.into()),
            version: Some("2024".into()),
            code: Some("dog".into()),
            ..Default::default()
        }),
        display_language: Some("nl".into()),
        ..Default::default()
    };
    let response = lookup(&registry, &Invocation::Type, &request).expect("looks up");
    assert_eq!(
        response.version.and_then(|v| v.value).as_deref(),
        Some("2024")
    );
    assert_eq!(response.display.value.as_deref(), Some("Hond"));
}

#[test]
fn lookup_property_selects_properties_and_lang_x_selects_designations() {
    let registry = registry();
    let request = CodeSystemLookupRequest {
        system: Some(URL.into()),
        code: Some("cat".into()),
        property: vec!["kingdom".into(), "lang.nl".into(), "display".into()],
        ..Default::default()
    };
    let response = lookup(&registry, &Invocation::Type, &request).expect("looks up");
    let codes: Vec<&str> = response
        .property
        .iter()
        .filter_map(|p| p.code.value.as_deref())
        .collect();
    assert_eq!(
        codes,
        ["kingdom"],
        "display is a named parameter, legs was not asked"
    );
    assert_eq!(response.designation.len(), 1);
    assert_eq!(response.designation[0].value.value.as_deref(), Some("Kat"));
}

#[test]
fn lookup_refusals_carry_their_issue_code_and_status() {
    let registry = registry();
    let run =
        |request: CodeSystemLookupRequest| lookup(&registry, &Invocation::Type, &request).err();
    // Nothing named.
    let error = run(CodeSystemLookupRequest::default()).expect("refused");
    assert!(matches!(error, OperationError::Required(_)));
    assert_eq!(
        (error.issue_code(), error.status()),
        ("required", StatusCode::BAD_REQUEST)
    );
    // Code without system.
    let error = run(CodeSystemLookupRequest {
        code: Some("cat".into()),
        ..Default::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::Required(_)));
    // Both forms.
    let error = run(CodeSystemLookupRequest {
        code: Some("cat".into()),
        system: Some(URL.into()),
        coding: Some(coding(Some(URL), "cat")),
        ..Default::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::Invalid(_)));
    // Unknown code: the page's example status.
    let error = run(CodeSystemLookupRequest {
        code: Some("unicorn".into()),
        system: Some(URL.into()),
        ..Default::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::UnknownCode { ref code, .. } if code == "unicorn"));
    assert_eq!(
        (error.issue_code(), error.status()),
        ("not-found", StatusCode::BAD_REQUEST)
    );
    // Unknown system and version.
    let error = run(CodeSystemLookupRequest {
        code: Some("cat".into()),
        system: Some("http://example.org/nowhere".into()),
        ..Default::default()
    })
    .expect("refused");
    assert_eq!(
        (error.issue_code(), error.status()),
        ("not-found", StatusCode::NOT_FOUND)
    );
    let error = run(CodeSystemLookupRequest {
        code: Some("cat".into()),
        system: Some(URL.into()),
        version: Some("1999".into()),
        ..Default::default()
    })
    .expect("refused");
    assert!(matches!(error, OperationError::UnknownVersion { .. }));
    // Instance level is not declared for $lookup in R4B.
    let error = lookup(
        &registry,
        &instance(&registry, URL),
        &CodeSystemLookupRequest {
            code: Some("cat".into()),
            ..Default::default()
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
            &CodeSystemSubsumesRequest {
                code_a: Some(a.into()),
                code_b: Some(b.into()),
                system: Some(URL.into()),
                ..Default::default()
            },
        )
        .expect("subsumes")
        .outcome
        .value
    };
    assert_eq!(run("animal", "cat").as_deref(), Some("subsumes"));
    assert_eq!(run("cat", "animal").as_deref(), Some("subsumed-by"));
    assert_eq!(run("cat", "cat").as_deref(), Some("equivalent"));
    assert_eq!(run("cat", "dog").as_deref(), Some("not-subsumed"));
    let by_codings = subsumes(
        &registry,
        &Invocation::Type,
        &CodeSystemSubsumesRequest {
            coding_a: Some(coding(Some(URL), "root")),
            coding_b: Some(coding(Some(URL), "kitten")),
            ..Default::default()
        },
    )
    .expect("subsumes");
    assert_eq!(by_codings.outcome.value.as_deref(), Some("subsumes"));
    let on_instance = subsumes(
        &registry,
        &instance(&registry, URL),
        &CodeSystemSubsumesRequest {
            code_a: Some("kitten".into()),
            code_b: Some("cat".into()),
            ..Default::default()
        },
    )
    .expect("subsumes");
    assert_eq!(on_instance.outcome.value.as_deref(), Some("subsumed-by"));
}

#[test]
fn subsumes_refusals_are_errors_never_not_subsumed() {
    let registry = registry();
    let run = |request: CodeSystemSubsumesRequest| {
        subsumes(&registry, &Invocation::Type, &request).expect_err("refused")
    };
    assert!(matches!(
        run(CodeSystemSubsumesRequest::default()),
        OperationError::Required(_)
    ));
    assert!(matches!(
        run(CodeSystemSubsumesRequest {
            code_a: Some("cat".into()),
            coding_b: Some(coding(Some(URL), "dog")),
            system: Some(URL.into()),
            ..Default::default()
        }),
        OperationError::Invalid(_)
    ));
    assert!(matches!(
        run(CodeSystemSubsumesRequest {
            code_a: Some("cat".into()),
            code_b: Some("dog".into()),
            ..Default::default()
        }),
        OperationError::Required(_)
    ));
    let unknown = run(CodeSystemSubsumesRequest {
        code_a: Some("cat".into()),
        code_b: Some("unicorn".into()),
        system: Some(URL.into()),
        ..Default::default()
    });
    assert!(matches!(unknown, OperationError::UnknownCode { .. }));
    let foreign = run(CodeSystemSubsumesRequest {
        coding_a: Some(coding(Some(URL), "cat")),
        coding_b: Some(coding(Some("http://loinc.org"), "1")),
        ..Default::default()
    });
    assert!(matches!(foreign, OperationError::NotSupported(_)));
    let flat = run(CodeSystemSubsumesRequest {
        code_a: Some("cat".into()),
        code_b: Some("dog".into()),
        system: Some(FLAT_URL.into()),
        ..Default::default()
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
            &CodeSystemSubsumesRequest {
                code_a: Some("a".into()),
                code_b: Some("b".into()),
                system: Some(URL.into()),
                ..Default::default()
            }
        ),
        Err(OperationError::UnknownSystem(_))
    ));
}
