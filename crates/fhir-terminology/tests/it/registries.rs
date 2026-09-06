//! The registry and grammar systems: BCP 47 (RFC 5646), BCP 13 (RFC 6838),
//! and ISO 3166-1 over the vendored IANA and CLDR data.

use std::sync::Arc;

use concept_graph::subsumption::Outcome;
use fhir_terminology::conceptmap::store::ConceptMapStore;
use fhir_terminology::filter::{Filter, FilterOperator};
use fhir_terminology::operations::expand::ExpandInput;
use fhir_terminology::operations::value_set_validate_code::ValueSetValidateInput;
use fhir_terminology::operations::{
    Invocation, OperationError, Sources, expand, subsumes, value_set_validate_code,
};
use fhir_terminology::provider::{CodeSystemProvider, ContentMode, ProviderError};
use fhir_terminology::registries::bcp47::{Analysis, Bcp47Provider};
use fhir_terminology::registries::{bcp13, bcp47, iso3166};
use fhir_terminology::registry::Registry;
use fhir_terminology::valueset::store::ValueSetStore;
use fhir_types::r4b::value_set::{
    ValueSet, ValueSetCompose, ValueSetComposeInclude, ValueSetComposeIncludeConcept,
    ValueSetComposeIncludeFilter,
};

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(Arc::new(Bcp47Provider::new()))
        .expect("bcp47");
    registry
        .register(Arc::new(bcp13::Bcp13Provider::new()))
        .expect("bcp13");
    registry
        .register(Arc::new(iso3166::provider().expect("cldr data")))
        .expect("iso3166");
    registry
}

fn filter(property: &str, op: FilterOperator, value: &str) -> Filter {
    Filter {
        property: property.to_owned(),
        op,
        value: value.to_owned(),
    }
}

fn enumerating(system: &str, codes: &[&str]) -> ValueSet {
    ValueSet {
        url: Some("http://example.org/enumerated".into()),
        status: "active".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(system.into()),
                concept: codes
                    .iter()
                    .map(|code| ValueSetComposeIncludeConcept {
                        code: (*code).into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn inline(system: &str, filters: Vec<(&str, &str, &str)>) -> ValueSet {
    ValueSet {
        url: Some("http://example.org/inline".into()),
        status: "active".into(),
        compose: Some(ValueSetCompose {
            include: vec![ValueSetComposeInclude {
                system: Some(system.into()),
                filter: filters
                    .into_iter()
                    .map(|(p, op, v)| ValueSetComposeIncludeFilter {
                        property: p.into(),
                        op: op.into(),
                        value: v.into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn bcp47_distinguishes_malformed_well_formed_and_valid_tags() {
    let provider = Bcp47Provider::new();
    assert!(matches!(provider.analyze("en"), Analysis::Valid(_)));
    assert!(matches!(provider.analyze("EN-gb"), Analysis::Valid(_)));
    assert!(matches!(provider.analyze("zh-Hant-TW"), Analysis::Valid(_)));
    assert!(matches!(
        provider.analyze("sl-rozaj-biske"),
        Analysis::Valid(_)
    ));
    assert!(matches!(
        provider.analyze("de-DE-u-co-phonebk"),
        Analysis::Valid(_)
    ));
    assert!(matches!(
        provider.analyze("x-private-tag"),
        Analysis::Valid(_)
    ));
    assert!(matches!(provider.analyze("i-klingon"), Analysis::Valid(_)));
    match provider.analyze("en-QQ") {
        Analysis::WellFormed { unknown, .. } => assert_eq!(unknown, ["QQ"]),
        other => panic!("well-formed with an unknown region, got {other:?}"),
    }
    match provider.analyze("abcd") {
        Analysis::WellFormed { unknown, .. } => {
            assert_eq!(unknown, ["abcd"], "4 letters are reserved");
        }
        other => panic!("reserved primary subtag, got {other:?}"),
    }
    assert!(matches!(provider.analyze(""), Analysis::Malformed(_)));
    assert!(matches!(provider.analyze("en--GB"), Analysis::Malformed(_)));
    assert!(matches!(
        provider.analyze("en-GB-u"),
        Analysis::Malformed(_)
    ));
    assert!(matches!(
        provider.analyze("toolongsubtag"),
        Analysis::Malformed(_)
    ));
    assert!(matches!(provider.analyze("en_GB"), Analysis::Malformed(_)));
}

#[test]
fn bcp47_locates_in_canonical_case_with_a_composed_display_and_parts() {
    let provider = Bcp47Provider::new();
    let located = provider
        .locate("EN-latn-gb")
        .expect("reads")
        .expect("valid");
    assert_eq!(located.code, "en-Latn-GB");
    assert_eq!(
        provider
            .display(located.concept, None)
            .expect("reads")
            .as_deref(),
        Some("English (Latin, United Kingdom)")
    );
    let again = provider
        .locate("en-Latn-GB")
        .expect("reads")
        .expect("valid");
    assert_eq!(
        again.concept, located.concept,
        "one ordinal per canonical tag"
    );
    let codes: Vec<String> = provider
        .properties(located.concept)
        .expect("reads")
        .into_iter()
        .map(|p| format!("{}={}", p.code, p.value.as_text()))
        .collect();
    assert_eq!(codes, ["language=en", "script=Latn", "region=GB"]);
    assert!(
        provider.locate("en-QQ").expect("reads").is_none(),
        "well-formed is not valid"
    );
    assert!(provider.locate("not a tag").expect("reads").is_none());
    assert!(
        provider
            .filter_matches(
                located.concept,
                &filter("language", FilterOperator::Equal, "en")
            )
            .expect("answers")
    );
    assert!(
        provider
            .filter_matches(
                located.concept,
                &filter("region", FilterOperator::Equal, "gb")
            )
            .expect("answers")
    );
    assert!(
        !provider
            .filter_matches(
                located.concept,
                &filter("region", FilterOperator::Equal, "US")
            )
            .expect("answers")
    );
    assert!(matches!(provider.all(), Err(ProviderError::NotEnumerable)));
    assert!(matches!(
        provider.filter(&filter("language", FilterOperator::Equal, "en")),
        Err(ProviderError::NotEnumerable)
    ));
    assert_eq!(provider.declaration().content, ContentMode::NotPresent);
    let deprecated = provider
        .locate("art-lojban")
        .expect("reads")
        .expect("grandfathered");
    assert!(!provider.status(deprecated.concept).expect("reads").active);
}

#[test]
fn bcp13_parses_the_grammar_knows_the_registry_and_subsumes_by_parameters() {
    let provider = bcp13::Bcp13Provider::new();
    let plain = provider
        .locate("Text/Plain; CharSet=UTF-8")
        .expect("reads")
        .expect("valid");
    assert_eq!(plain.code, "text/plain; charset=utf-8");
    assert!(provider.locate("not a mime type").expect("reads").is_none());
    assert!(provider.locate("text/").expect("reads").is_none());
    assert!(
        provider
            .locate("text/plain; charset")
            .expect("reads")
            .is_none()
    );
    let unregistered = provider
        .locate("application/x-tx-ecosystem-test")
        .expect("reads")
        .expect("valid");
    let props = |c| -> Vec<String> {
        provider
            .properties(c)
            .expect("reads")
            .into_iter()
            .map(|p| format!("{}={}", p.code, p.value.as_text()))
            .collect()
    };
    assert_eq!(
        props(plain.concept),
        ["base=text", "base=text/plain", "registered=true"]
    );
    assert!(props(unregistered.concept).contains(&String::from("registered=false")));
    assert!(
        provider
            .filter_matches(
                plain.concept,
                &filter("base", FilterOperator::Equal, "text")
            )
            .expect("answers")
    );
    assert!(
        provider
            .filter_matches(
                plain.concept,
                &filter("base", FilterOperator::Equal, "text/plain")
            )
            .expect("answers")
    );
    assert!(
        !provider
            .filter_matches(
                plain.concept,
                &filter("base", FilterOperator::Equal, "text/html")
            )
            .expect("answers")
    );
    let registered = provider
        .filter_all(&[filter("registered", FilterOperator::Equal, "true")])
        .expect("enumerates");
    assert!(
        registered.len() > 2000,
        "{} registered types",
        registered.len()
    );
    let narrow = provider
        .filter_all(&[
            filter("registered", FilterOperator::Equal, "true"),
            filter("base", FilterOperator::Equal, "text/plain"),
        ])
        .expect("enumerates");
    assert_eq!(narrow.len(), 1);
    assert!(matches!(
        provider.filter_all(&[filter("base", FilterOperator::Equal, "text")]),
        Err(ProviderError::NotEnumerable)
    ));
    assert!(matches!(
        provider.filter_all(&[filter("registered", FilterOperator::Equal, "false")]),
        Err(ProviderError::NotEnumerable)
    ));
}

#[test]
fn bcp13_subsumes_by_parameters_and_cannot_decide_unknown_ones() {
    let provider = bcp13::Bcp13Provider::new();
    let sub = |a: &str, b: &str| {
        let a = provider.locate(a).expect("reads").expect("valid").concept;
        let b = provider.locate(b).expect("reads").expect("valid").concept;
        provider.subsumes(a, b)
    };
    assert_eq!(
        sub("text/plain", "text/plain").expect("decides"),
        Some(Outcome::Equivalent)
    );
    assert_eq!(
        sub("Text/Plain; CharSet=UTF-8", "text/plain; charset=utf-8").expect("decides"),
        Some(Outcome::Equivalent)
    );
    assert_eq!(
        sub("text/plain", "application/json").expect("decides"),
        Some(Outcome::NotSubsumed)
    );
    assert_eq!(
        sub("application/xml", "application/fhir+xml").expect("decides"),
        Some(Outcome::NotSubsumed),
        "a suffix is not a hierarchy"
    );
    assert_eq!(
        sub("text/plain", "text/plain; charset=utf-8").expect("decides"),
        Some(Outcome::Subsumes)
    );
    assert_eq!(
        sub("text/plain; charset=utf-8", "text/plain").expect("decides"),
        Some(Outcome::SubsumedBy)
    );
    assert_eq!(
        sub("text/plain; charset=utf-8", "text/plain; charset=utf-16").expect("decides"),
        Some(Outcome::NotSubsumed)
    );
    assert_eq!(
        sub(
            "text/plain; charset=utf-8",
            "text/plain; charset=utf-8; format=flowed"
        )
        .expect("decides"),
        Some(Outcome::Subsumes)
    );
    assert_eq!(
        sub("text/plain; charset=utf-8", "text/plain; format=flowed").expect("decides"),
        Some(Outcome::NotSubsumed)
    );
    assert_eq!(
        sub("text/plain; foo=bar", "text/plain; charset=utf-8; foo=bar").expect("decides"),
        Some(Outcome::Subsumes)
    );
    assert!(matches!(
        sub("text/plain", "text/plain; foo=bar"),
        Err(ProviderError::CannotDetermine(_))
    ));
    assert!(matches!(
        sub("text/plain; foo=bar", "text/plain; foo=baz"),
        Err(ProviderError::CannotDetermine(_))
    ));
}

#[test]
fn iso3166_is_a_case_insensitive_table_with_user_assigned_codes() {
    let provider = iso3166::provider().expect("cldr data");
    assert_eq!(provider.identity().url, iso3166::URL);
    assert!(!provider.identity().version.is_empty());
    let nl = provider.locate("nl").expect("reads").expect("NL");
    assert_eq!(nl.code, "NL");
    assert_eq!(
        provider
            .display(nl.concept, None)
            .expect("reads")
            .as_deref(),
        Some("Netherlands")
    );
    let props: Vec<String> = provider
        .properties(nl.concept)
        .expect("reads")
        .into_iter()
        .map(|p| format!("{}={}", p.code, p.value.as_text()))
        .collect();
    assert!(props.contains(&String::from("alpha3=NLD")));
    assert!(props.contains(&String::from("numeric=528")));
    let xy = provider
        .locate("XY")
        .expect("reads")
        .expect("user-assigned");
    assert_eq!(
        provider
            .display(xy.concept, None)
            .expect("reads")
            .as_deref(),
        Some("User-assigned")
    );
    assert!(
        provider.locate("EU").expect("reads").is_none(),
        "a CLDR reservation, not ISO 3166-1"
    );
    assert!(provider.locate("ZZZ").expect("reads").is_none());
    let regex = provider
        .filter(&filter("code", FilterOperator::Regex, "^N[A-Z]$"))
        .expect("regex");
    assert!(regex.contains(nl.concept.index()));
    assert!(regex.len() >= 10 && regex.len() < 30, "{}", regex.len());
    assert!(
        iso3166::is_user_assigned("QM")
            && iso3166::is_user_assigned("ZZ")
            && !iso3166::is_user_assigned("QL")
    );
}

#[test]
fn the_grammar_systems_refuse_expansion_and_validate_by_membership() {
    let registry = registry();
    let value_sets = ValueSetStore::new();
    let concept_maps = ConceptMapStore::new();
    let sources = Sources {
        registry: &registry,
        value_sets: &value_sets,
        concept_maps: &concept_maps,
    };
    let all = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![],
        ))),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&sources, &all),
        Err(OperationError::NotSupported(_))
    ));
    let base = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("base", "=", "text")],
        ))),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&sources, &base),
        Err(OperationError::NotSupported(_))
    ));
    let registered = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("registered", "=", "true")],
        ))),
        ..ExpandInput::default()
    };
    let error = expand::expand(&sources, &registered).expect_err("too costly without count");
    assert!(matches!(error, OperationError::TooCostly(_)), "{error}");
    assert_eq!(error.issue_code(), "too-costly");
    let paged = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("registered", "=", "true")],
        ))),
        count: Some(50),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&sources, &paged).expect("pages");
    assert_eq!(vs.contains.len(), 50);
    let narrow = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("registered", "=", "true"), ("base", "=", "text/plain")],
        ))),
        ..ExpandInput::default()
    };
    let vs = expand::expand(&sources, &narrow).expect("expands");
    assert_eq!(vs.total, 1);
    let languages = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp47::URL,
            vec![("language", "=", "en")],
        ))),
        ..ExpandInput::default()
    };
    assert!(matches!(
        expand::expand(&sources, &languages),
        Err(OperationError::NotSupported(_))
    ));
}

#[test]
fn the_grammar_systems_validate_by_membership_and_decline_undetermined_subsumption() {
    let registry = registry();
    let value_sets = ValueSetStore::new();
    let concept_maps = ConceptMapStore::new();
    let sources = Sources {
        registry: &registry,
        value_sets: &value_sets,
        concept_maps: &concept_maps,
    };
    let member = ValueSetValidateInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("base", "=", "text")],
        ))),
        code: Some(String::from("text/plain; charset=utf-8")),
        system: Some(bcp13::URL.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&sources, &member).expect("validates");
    assert!(validation.result);
    assert_eq!(
        validation.code.as_deref(),
        Some("text/plain; charset=utf-8")
    );
    let outsider = ValueSetValidateInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("base", "=", "text")],
        ))),
        code: Some(String::from("application/json")),
        system: Some(bcp13::URL.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation =
        value_set_validate_code::validate_code(&sources, &outsider).expect("validates");
    assert!(!validation.result);
    assert_eq!(validation.issues[0].kind, "not-in-vs");
    let english = ValueSetValidateInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp47::URL,
            vec![("language", "=", "en"), ("region", "=", "US")],
        ))),
        code: Some(String::from("en-us")),
        system: Some(bcp47::URL.to_owned()),
        ..ValueSetValidateInput::default()
    };
    let validation = value_set_validate_code::validate_code(&sources, &english).expect("validates");
    assert!(validation.result);
    assert_eq!(
        validation.code.as_deref(),
        Some("en-us"),
        "the request's spelling"
    );
    assert_eq!(validation.normalized_code.as_deref(), Some("en-US"));
    assert_eq!(
        validation.display.as_deref(),
        Some("English (United States)")
    );
    let subsumes_request = subsumes::SubsumesInput {
        system: Some(bcp13::URL.to_owned()),
        code_a: Some(String::from("text/plain")),
        code_b: Some(String::from("text/plain; foo=bar")),
        ..subsumes::SubsumesInput::default()
    };
    let error = subsumes::subsumes(&registry, &Invocation::Type, &subsumes_request)
        .expect_err("undetermined");
    assert!(matches!(error, OperationError::CannotDetermine(_)));
    assert_eq!(error.tx_issue_type(), "cannot-determine");
}

/// The IANA registry is finite, so `registered = true` walks it, but every
/// registered type also carries the parameters of RFC 2045 §5.1, which no
/// enumeration holds: "if the value set itself is unbounded due to the
/// inclusion of post-coordinated value sets (e.g. SNOMED CT, UCUM), then the
/// extension valueset-unclosed can be used to indicate that the expansion is
/// incomplete" (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>,
/// Notes).
#[test]
fn a_filtered_media_type_expansion_is_unclosed_and_an_enumerated_one_is_not() {
    let registry = registry();
    let value_sets = ValueSetStore::new();
    let concept_maps = ConceptMapStore::new();
    let sources = Sources {
        registry: &registry,
        value_sets: &value_sets,
        concept_maps: &concept_maps,
    };
    let narrow = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(&inline(
            bcp13::URL,
            vec![("registered", "=", "true"), ("base", "=", "text/plain")],
        ))),
        ..ExpandInput::default()
    };
    let outcome = expand::expand(&sources, &narrow).expect("expands");
    assert_eq!(outcome.total, 1);
    assert!(outcome.unclosed, "the parameterised forms are not listed");
    let rendered = fhir_terminology::valueset::render::r4b::expansion(&outcome);
    let expansion = rendered.expansion.expect("an expansion");
    assert_eq!(
        expansion.extension,
        vec![fhir_types::r4b::extension::Extension {
            url: String::from("http://hl7.org/fhir/StructureDefinition/valueset-unclosed"),
            value: Some(fhir_types::r4b::extension::ExtensionValue::Boolean(
                true.into()
            )),
            ..Default::default()
        }]
    );
    let enumerated = ExpandInput {
        inline_value_set: Some(fhir_terminology::valueset::convert::r4b::convert(
            &enumerating(bcp13::URL, &["text/plain", "text/plain; charset=utf-8"]),
        )),
        ..ExpandInput::default()
    };
    let outcome = expand::expand(&sources, &enumerated).expect("expands");
    assert_eq!(outcome.total, 2);
    assert!(
        !outcome.unclosed,
        "the codes an include lists are its members"
    );
    let rendered = fhir_terminology::valueset::render::r4b::expansion(&outcome);
    assert!(
        rendered
            .expansion
            .expect("an expansion")
            .extension
            .is_empty()
    );
}

/// A tag is valid when every subtag is registered, and the variants,
/// extensions, and private-use subtags of RFC 5646 §2.2.6 to §2.2.7 build
/// unboundedly many valid tags over the finite registry.
#[test]
fn a_language_tag_selection_is_unclosed() {
    let provider = Bcp47Provider::new();
    assert!(provider.unclosed(&[filter("language", FilterOperator::Equal, "en")]));
}
