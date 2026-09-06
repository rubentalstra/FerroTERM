//! The SNOMED CT provider through the seam, over the synthetic artifact.

use std::sync::Arc;

use concept_graph::subsumption::Outcome;
use fhir_terminology::capabilities::Summary;
use fhir_terminology::filter::{Filter, FilterOperator};
use fhir_terminology::operations::{Invocation, OperationError, lookup, validate_code};
use fhir_terminology::provider::{
    Capability, CodeSystemProvider, Compositional, Concept, PropertyValue, ProviderError,
};
use fhir_terminology::registry::Registry;
use fhir_terminology::snomed::{OpenError, SYSTEM, SnomedProvider};

use ferroterm_testkit::snomed;
use ferroterm_testkit::snomed::{
    ALTERNATIVE, ANIMAL, CAT, CODES_MAP, COVERING, DOG, EDITION, FISH, FUR, ICD10_MAP, LEGS,
    MODULE_CONCEPT, MODULE_DEPENDENCY, PETS, POSSIBLY_EQUIVALENT_TO, REPLACED_BY, SAME_AS, SCHEME,
    TOP, VERSION, item, sctid,
};

fn provider() -> (tempfile::TempDir, SnomedProvider) {
    let dir = tempfile::tempdir().expect("tempdir");
    snomed::write(dir.path()).expect("writes the fixture");
    let provider = SnomedProvider::open(dir.path(), "en").expect("opens");
    (dir, provider)
}

#[test]
fn identity_and_declaration_follow_the_manifest() {
    let (_dir, p) = provider();
    assert_eq!(p.identity().url, SYSTEM);
    assert_eq!(p.identity().version, VERSION);
    assert_eq!(p.edition_uri(), EDITION);
    let declaration = p.declaration();
    assert_eq!(declaration.languages, ["en", "nl"]);
    assert!(declaration.capabilities.contains(&Capability::Subsumption));
    assert!(declaration.capabilities.contains(&Capability::Enumeration));
    let codes: Vec<&str> = declaration
        .properties
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(
        &codes[..6],
        [
            "inactive",
            "sufficientlyDefined",
            "moduleId",
            "effectiveTime",
            "parent",
            "child"
        ]
    );
    assert!(codes.contains(&sctid(item(COVERING)).as_str()));
    assert!(codes.contains(&sctid(item(LEGS)).as_str()));
    assert_eq!(p.language_refsets(), [snomed::GB_REFSET, snomed::NL_REFSET]);
    assert_eq!(p.all().expect("all").len(), 19);
}

#[test]
fn locate_accepts_valid_sctids_only() {
    let (_dir, p) = provider();
    let cat = p.locate(&sctid(item(CAT))).expect("reads").expect("cat");
    assert_eq!(cat.concept, Concept::new(CAT));
    assert_eq!(
        p.code(cat.concept).expect("reads").as_deref(),
        Some(sctid(item(CAT)).as_str())
    );
    // A well-formed SCTID the edition lacks, a wrong check digit, and text: absent, never an error.
    assert!(p.locate(&sctid(4242)).expect("reads").is_none());
    let mut wrong = sctid(item(CAT));
    wrong.pop();
    wrong.push('0');
    assert!(p.locate(&wrong).expect("reads").is_none());
    assert!(p.locate("cat").expect("reads").is_none());
}

#[test]
fn display_is_the_preferred_term_of_the_language_with_a_stated_fallback() {
    let (_dir, p) = provider();
    let cat = Concept::new(CAT);
    assert_eq!(p.display(cat, None).expect("reads").as_deref(), Some("Cat"));
    assert_eq!(
        p.display(cat, Some("en-GB")).expect("reads").as_deref(),
        Some("Cat")
    );
    assert_eq!(
        p.display(cat, Some("nl")).expect("reads").as_deref(),
        Some("Kat")
    );
    // No Dutch term: fall back to the default language's preferred term.
    assert_eq!(
        p.display(Concept::new(TOP), Some("nl"))
            .expect("reads")
            .as_deref(),
        Some("Living thing")
    );
    // Fish has an FSN in a refset and an unreferenced synonym: the active synonym in the language wins over the FSN.
    assert_eq!(
        p.display(Concept::new(FISH), None)
            .expect("reads")
            .as_deref(),
        Some("Fish")
    );
    // An unknown language: the default.
    assert_eq!(
        p.display(cat, Some("fr")).expect("reads").as_deref(),
        Some("Cat")
    );
}

#[test]
fn designations_carry_the_snomed_use_codings_and_filter_by_language() {
    let (_dir, p) = provider();
    let all = p.designations(Concept::new(CAT), None).expect("reads");
    assert_eq!(all.len(), 5);
    let fsn = all
        .iter()
        .find(|d| d.value == "Cat (synthetic)")
        .expect("fsn");
    assert_eq!(
        fsn.use_.as_ref().map(|u| u.code.as_str()),
        Some("900000000000003001")
    );
    assert_eq!(fsn.use_.as_ref().map(|u| u.system.as_str()), Some(SYSTEM));
    let dutch = p
        .designations(Concept::new(CAT), Some("nl-NL"))
        .expect("reads");
    let terms: Vec<&str> = dutch.iter().map(|d| d.value.as_str()).collect();
    assert_eq!(terms, ["Kat", "Poes"]);
    assert!(dutch.iter().all(|d| d.language.as_deref() == Some("nl")));
    assert!(p.definition(Concept::new(CAT)).expect("reads").is_none());
}

#[test]
fn properties_follow_the_snomed_on_fhir_list() {
    let (_dir, p) = provider();
    let props = p.properties(Concept::new(CAT)).expect("reads");
    let find = |code: &str| -> Vec<&PropertyValue> {
        props
            .iter()
            .filter(|p| p.code == code)
            .map(|p| &p.value)
            .collect()
    };
    assert_eq!(find("inactive"), [&PropertyValue::Boolean(false)]);
    assert_eq!(find("sufficientlyDefined"), [&PropertyValue::Boolean(true)]);
    assert_eq!(find("moduleId"), [&PropertyValue::Code(sctid(99))]);
    assert_eq!(
        find("effectiveTime"),
        [&PropertyValue::String(String::from("20260101"))]
    );
    assert_eq!(find("parent"), [&PropertyValue::Code(sctid(item(ANIMAL)))]);
    assert!(find("child").is_empty());
    assert_eq!(
        find(&sctid(item(COVERING))),
        [&PropertyValue::Code(sctid(item(FUR)))]
    );
    assert_eq!(
        find(&sctid(item(LEGS))),
        [&PropertyValue::Decimal(String::from("4"))]
    );
    let animal = p.properties(Concept::new(ANIMAL)).expect("reads");
    let children: Vec<&PropertyValue> = animal
        .iter()
        .filter(|p| p.code == "child")
        .map(|p| &p.value)
        .collect();
    assert_eq!(
        children,
        [
            &PropertyValue::Code(sctid(item(CAT))),
            &PropertyValue::Code(sctid(item(DOG)))
        ]
    );
    let fish = p.properties(Concept::new(FISH)).expect("reads");
    assert!(
        fish.iter()
            .any(|p| p.code == "inactive" && p.value == PropertyValue::Boolean(true))
    );
    assert!(
        fish.iter()
            .any(|p| p.code == "sufficientlyDefined" && p.value == PropertyValue::Boolean(false))
    );
    assert!(!p.status(Concept::new(FISH)).expect("reads").active);
    assert!(p.status(Concept::new(CAT)).expect("reads").active);
}

#[test]
fn the_hierarchy_answers_subsumption_and_the_filters_from_the_closure() {
    let (_dir, p) = provider();
    let hierarchy = p.hierarchy().expect("snomed has a hierarchy");
    assert_eq!(
        hierarchy.subsumes(Concept::new(ANIMAL), Concept::new(CAT)),
        Outcome::Subsumes
    );
    assert_eq!(
        hierarchy.subsumes(Concept::new(CAT), Concept::new(TOP)),
        Outcome::SubsumedBy
    );
    assert_eq!(
        hierarchy.subsumes(Concept::new(CAT), Concept::new(DOG)),
        Outcome::NotSubsumed
    );
    assert_eq!(
        hierarchy.subsumes(Concept::new(CAT), Concept::new(CAT)),
        Outcome::Equivalent
    );
    let descendants = p
        .filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::DescendentOf,
            value: sctid(item(ANIMAL)),
        })
        .expect("filters");
    assert_eq!(descendants.iter().collect::<Vec<_>>(), [CAT, DOG]);
    let leaves = p
        .filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::DescendentLeaf,
            value: sctid(item(TOP)),
        })
        .expect("filters");
    assert_eq!(
        leaves.iter().collect::<Vec<_>>(),
        [
            CAT,
            DOG,
            FUR,
            COVERING,
            LEGS,
            PETS,
            CODES_MAP,
            SAME_AS,
            SCHEME,
            REPLACED_BY,
            POSSIBLY_EQUIVALENT_TO,
            ALTERNATIVE,
            MODULE_DEPENDENCY,
            MODULE_CONCEPT,
            ICD10_MAP
        ]
    );
}

#[test]
fn search_reads_the_designation_index() {
    let (_dir, p) = provider();
    let ka = p.search("ka", Some("nl")).expect("searches");
    assert_eq!(ka.iter().collect::<Vec<_>>(), [CAT]);
    let synth = p.search("synth", None).expect("searches");
    assert_eq!(
        synth.len(),
        7,
        "every FSN except the two attribute FSNs carries the tag"
    );
    let none = p.search("zebra", None).expect("searches");
    assert!(none.is_empty());
}

#[test]
fn a_registry_with_the_provider_renders_terminology_capabilities() {
    let (_dir, p) = provider();
    let mut registry = Registry::new();
    registry.register(Arc::new(p)).expect("registers");
    let summary = Summary::of(&registry);
    let system = summary
        .systems
        .iter()
        .find(|s| s.url == SYSTEM)
        .expect("snomed");
    assert!(system.subsumption);
    assert_eq!(system.versions[0].code, VERSION);
    assert!(system.versions[0].is_default);
    // NOTE: the element is "If the compositional grammar defined by the code
    // system is supported"
    // (<https://hl7.org/fhir/R4B/terminologycapabilities-definitions.html#TerminologyCapabilities.codeSystem.version.compositional>).
    assert!(!system.versions[0].compositional);
    assert_eq!(system.versions[0].languages, ["en", "nl"]);
    assert!(
        system.versions[0]
            .properties
            .contains(&String::from("inactive"))
    );
    let r4b = summary.to_r4b("2026-09-02T00:00:00Z");
    assert_eq!(r4b.code_system.len(), 1);
}

#[test]
fn a_foreign_or_broken_artifact_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        SnomedProvider::open(dir.path(), "en"),
        Err(OpenError::Io { .. })
    ));
    snomed::write(dir.path()).expect("writes the fixture");
    std::fs::write(
        dir.path().join("manifest.json"),
        r#"{"manifest":2,"system":"http://loinc.org","edition":"x","version":"2.80","store":"store.redb","hierarchy":"hierarchy.bin","text":"text.bin"}"#,
    )
    .expect("writes");
    assert!(
        matches!(SnomedProvider::open(dir.path(), "en"), Err(OpenError::NotSnomed(s)) if s == "http://loinc.org")
    );
    std::fs::write(
        dir.path().join("manifest.json"),
        r#"{"manifest":1,"system":"http://snomed.info/sct","edition":"x","version":"2.80","store":"store.redb","hierarchy":"hierarchy.bin","text":"text.bin"}"#,
    )
    .expect("writes");
    assert!(matches!(
        SnomedProvider::open(dir.path(), "en"),
        Err(OpenError::ManifestVersion(1))
    ));
}

#[test]
fn the_implicit_value_sets_follow_the_snomed_ct_page() {
    let (_dir, p) = provider();
    let base = "http://snomed.info/sct";
    let all = p
        .implicit_value_set(&format!("{base}?fhir_vs"))
        .expect("implicit")
        .expect("compose");
    assert_eq!(all.include.len(), 1);
    assert!(all.include[0].filters.is_empty() && all.include[0].concepts.is_empty());
    assert_eq!(
        all.include[0]
            .system
            .as_ref()
            .and_then(|s| s.version.as_deref()),
        None,
        "the bare URI leaves the version to the registry"
    );
    let isa = p
        .implicit_value_set(&format!("{EDITION}?fhir_vs=isa/{}", sctid(item(ANIMAL))))
        .expect("implicit")
        .expect("compose");
    assert_eq!(isa.include[0].filters[0].op, FilterOperator::IsA);
    assert_eq!(isa.include[0].filters[0].value, sctid(item(ANIMAL)));
    assert_eq!(
        isa.include[0]
            .system
            .as_ref()
            .and_then(|s| s.version.as_deref()),
        Some(VERSION),
        "an edition base pins the served version"
    );
    assert!(
        p.implicit_value_set(&format!("{VERSION}?fhir_vs=isa/{}", sctid(item(ANIMAL))))
            .expect("implicit")
            .is_ok(),
        "the version URI is a base too"
    );
    let refsets = p
        .implicit_value_set(&format!("{base}?fhir_vs=refset"))
        .expect("implicit")
        .expect("compose");
    assert_eq!(
        refsets.include[0].concepts.len(),
        8,
        "every reference set with concept members"
    );
    assert!(
        refsets.include[0]
            .concepts
            .iter()
            .any(|c| c.code == sctid(item(PETS)))
    );
    // The FHIR SNOMED CT page defines the set as "all concept ids that
    // correspond to reference sets that are explicitly defined in the specified
    // SNOMED CT edition" (<https://hl7.org/fhir/R4B/snomedct.html>), with no
    // category excluded, so a metadata reference set is listed like any other.
    assert!(
        refsets.include[0]
            .concepts
            .iter()
            .any(|c| c.code == snomed::MODULE_DEPENDENCY_SCTID),
        "the Module Dependency reference set is a reference set of the edition"
    );
    let members = p
        .implicit_value_set(&format!("{base}?fhir_vs=refset/{}", sctid(item(PETS))))
        .expect("implicit")
        .expect("compose");
    assert_eq!(members.include[0].filters[0].op, FilterOperator::In);
    let selected = p.filter(&members.include[0].filters[0]).expect("filters");
    assert_eq!(selected.iter().collect::<Vec<_>>(), [CAT, DOG]);
}

#[test]
fn malformed_and_unknown_implicit_value_sets_are_refused() {
    let (_dir, p) = provider();
    let base = "http://snomed.info/sct";
    assert!(
        matches!(
            p.implicit_value_set(&format!("{base}?fhir_vs=refset/{}", sctid(item(FUR))))
                .expect("implicit"),
            Err(ProviderError::UnknownCode(_))
        ),
        "a concept that is not a reference set"
    );
    assert!(matches!(
        p.implicit_value_set(&format!("{base}?fhir_vs=isa/{}", sctid(77)))
            .expect("implicit"),
        Err(ProviderError::UnknownCode(_))
    ));
    assert!(matches!(
        p.implicit_value_set(&format!("{base}?fhir_vs=isa/abc"))
            .expect("implicit"),
        Err(ProviderError::MalformedImplicitValueSet { .. })
    ));
    assert!(matches!(
        p.implicit_value_set(&format!("{base}?fhir_vs=ecl/%3C%3C%20"))
            .expect("implicit"),
        Err(ProviderError::MalformedImplicitValueSet { .. })
    ));
    assert!(matches!(
        p.implicit_value_set(&format!("{base}?fhir_vs=nope"))
            .expect("implicit"),
        Err(ProviderError::MalformedImplicitValueSet { .. })
    ));
    // The base may be any edition version (<https://hl7.org/fhir/R4B/snomedct.html>,
    // "Implicit Value Sets"), so another edition is a version this provider does
    // not serve, which the registry asks its other editions about.
    assert!(
        matches!(
            p.implicit_value_set("http://snomed.info/sct/999?fhir_vs")
                .expect("implicit"),
            Err(ProviderError::UnservedImplicitVersion { ref url, ref version })
                if url == SYSTEM && version == "http://snomed.info/sct/999"
        ),
        "another edition"
    );
    assert!(p.implicit_value_set(&format!("{base}?fhir_cm=1")).is_none());
    assert!(matches!(
        p.filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::In,
            value: String::from("abc"),
        }),
        Err(ProviderError::InvalidFilterValue { .. })
    ));
    let declared = p
        .declaration()
        .filters
        .iter()
        .find(|f| f.code == "concept")
        .expect("the concept filter is declared");
    assert!(declared.operators.contains(&FilterOperator::In));
}

#[test]
fn ecl_arrives_as_the_constraint_filter_and_the_ecl_implicit_value_set() {
    let (_dir, p) = provider();
    let animals = sctid(item(ANIMAL));
    let filter = |value: &str| Filter {
        property: String::from("constraint"),
        op: FilterOperator::Equal,
        value: value.to_owned(),
    };
    assert_eq!(
        p.filter(&filter(&format!("< {animals}")))
            .expect("evaluates")
            .iter()
            .collect::<Vec<_>>(),
        [CAT, DOG]
    );
    assert!(
        matches!(
            p.filter(&filter("<< ")),
            Err(ProviderError::InvalidFilterValue { reason, .. }) if reason.contains("byte 3")
        ),
        "malformed ECL names the position"
    );
    assert!(
        matches!(
            p.filter(&filter("<< 999999999")),
            Err(ProviderError::InvalidCode { code, .. }) if code == "999999999"
        ),
        "an unknown identifier in valid ECL is an invalid code"
    );
    assert!(matches!(
        p.filter(&filter("* {{ D moduleId = 900000000000207008 }}")),
        Err(ProviderError::UnsupportedFilter { .. })
    ));
    let expressions = |value: &str| Filter {
        property: String::from("expressions"),
        op: FilterOperator::Equal,
        value: value.to_owned(),
    };
    assert_eq!(p.filter(&expressions("false")).expect("all").len(), 19);
    assert!(matches!(
        p.filter(&expressions("true")),
        Err(ProviderError::UnsupportedFilter { .. })
    ));
    assert!(matches!(
        p.filter(&expressions("maybe")),
        Err(ProviderError::InvalidFilterValue { .. })
    ));
    let codes: Vec<&str> = p
        .declaration()
        .filters
        .iter()
        .map(|f| f.code.as_str())
        .collect();
    assert_eq!(codes, ["concept", "constraint", "expressions"]);

    // The implicit value set carries the URI-encoded expression
    // (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets").
    let encoded = format!("%3C%20{animals}%20%7Canimal%7C");
    for base in ["http://snomed.info/sct", EDITION, VERSION] {
        let compose = p
            .implicit_value_set(&format!("{base}?fhir_vs=ecl/{encoded}"))
            .expect("implicit")
            .expect("compose");
        let include = &compose.include[0];
        assert_eq!(include.filters.len(), 1);
        assert_eq!(include.filters[0].property, "constraint");
        assert_eq!(include.filters[0].op, FilterOperator::Equal);
        assert_eq!(include.filters[0].value, format!("< {animals} |animal|"));
        assert_eq!(
            include.system.as_ref().and_then(|s| s.version.as_deref()),
            (base != "http://snomed.info/sct").then_some(VERSION)
        );
        assert_eq!(
            p.filter(&include.filters[0])
                .expect("evaluates")
                .iter()
                .collect::<Vec<_>>(),
            [CAT, DOG]
        );
    }
    assert!(
        matches!(
            p.implicit_value_set("http://snomed.info/sct?fhir_vs=ecl/%3C%3C%20"),
            Some(Err(ProviderError::MalformedImplicitValueSet { reason, .. })) if reason.contains("byte")
        ),
        "malformed ECL is a malformed implicit value set"
    );
    assert!(
        matches!(
            p.implicit_value_set("http://snomed.info/sct?fhir_vs=ecl/%3"),
            Some(Err(ProviderError::MalformedImplicitValueSet { .. }))
        ),
        "a stray percent is malformed"
    );
    assert!(
        matches!(
            p.implicit_value_set(&format!("http://snomed.info/sct?fhir_vs=ecl/{animals}")),
            Some(Ok(_))
        ),
        "an expression without reserved characters needs no encoding"
    );
}

/// A metadata reference set is served like any other reference set.
///
/// The FHIR SNOMED CT page defines `?fhir_vs=refset` as "all concept ids that
/// correspond to reference sets that are explicitly defined in the specified
/// SNOMED CT edition" and `?fhir_vs=refset/[sctid]` as "all concept ids in the
/// specified reference set" (<https://hl7.org/fhir/R4B/snomedct.html>). Neither
/// excludes a category, so the Module Dependency reference set, whose members
/// are the edition's modules, is listed and expands to them (#272).
#[test]
fn the_module_dependency_reference_set_is_served_like_any_other() {
    let (_dir, p) = provider();
    let members = p
        .implicit_value_set(&format!(
            "{SYSTEM}?fhir_vs=refset/{}",
            snomed::MODULE_DEPENDENCY_SCTID
        ))
        .expect("implicit")
        .expect("compose");
    assert_eq!(members.include[0].filters[0].op, FilterOperator::In);
    let selected = p.filter(&members.include[0].filters[0]).expect("filters");
    assert_eq!(
        selected.iter().collect::<Vec<_>>(),
        [MODULE_CONCEPT],
        "the members are the edition's modules"
    );
}

#[test]
fn the_defined_grammar_and_the_supported_grammar_are_two_declarations() {
    let (_dir, p) = provider();
    // `CodeSystem.compositional` is "The code system defines a compositional
    // (post-coordination) grammar"
    // (<https://hl7.org/fhir/R4B/codesystem-definitions.html#CodeSystem.compositional>),
    // which SNOMED CT does (<http://snomed.org/scg>).
    assert!(p.declaration().compositional.defined());
    // `TerminologyCapabilities.codeSystem.version.compositional` is "If the
    // compositional grammar defined by the code system is supported"
    // (<https://hl7.org/fhir/R4B/terminologycapabilities-definitions.html#TerminologyCapabilities.codeSystem.version.compositional>),
    // which this server does not: every expression is refused.
    assert!(!p.declaration().compositional.supported());
    assert_eq!(p.declaration().compositional, Compositional::Defined);
    let mut registry = Registry::new();
    registry.register(Arc::new(p)).expect("registers");
    let summary = Summary::of(&registry);
    let date = "2026-09-05T00:00:00Z";
    let flags = [
        (
            "r4",
            summary.to_r4(date).code_system[0].version[0]
                .compositional
                .as_ref()
                .and_then(|f| f.value),
        ),
        (
            "r4b",
            summary.to_r4b(date).code_system[0].version[0]
                .compositional
                .as_ref()
                .and_then(|f| f.value),
        ),
        (
            "r5",
            summary.to_r5(date).code_system[0].version[0]
                .compositional
                .as_ref()
                .and_then(|f| f.value),
        ),
        (
            "r6",
            summary.to_r6(date).code_system[0].version[0]
                .compositional
                .as_ref()
                .and_then(|f| f.value),
        ),
    ];
    for (version, value) in flags {
        assert_eq!(
            value,
            Some(false),
            "{version} declares the grammar unsupported"
        );
    }
}

#[test]
fn a_post_coordinated_expression_is_refused_for_the_grammar_not_as_an_unknown_concept() {
    let (_dir, p) = provider();
    let mut registry = Registry::new();
    registry.register(Arc::new(p)).expect("registers");
    // NOTE: SNOMED CT Expressions in Compositional Grammar are valid codes
    // (<https://hl7.org/fhir/R4B/snomedct.html>, "Code"), so a server that will
    // not evaluate one says that instead of reporting an unknown concept.
    let expression = format!("{}:{}={}", sctid(CAT), sctid(COVERING), sctid(FUR));
    let input = lookup::LookupInput {
        system: Some(SYSTEM.to_owned()),
        code: Some(expression.clone()),
        ..lookup::LookupInput::default()
    };
    let error = lookup::lookup(&registry, &Invocation::Type, &input).expect_err("refuses");
    assert!(
        matches!(&error, OperationError::UnsupportedGrammar { system, code }
            if system == SYSTEM && *code == expression),
        "{error:?}"
    );
    assert_eq!(error.issue_code(), "not-supported");
    assert_eq!(error.tx_issue_type(), "not-supported");
    // `$validate-code` answers `false` for the same code, and its message says
    // which of the two failures it is.
    let request = validate_code::ValidateCodeInput {
        url: Some(SYSTEM.to_owned()),
        code: Some(expression.clone()),
        ..validate_code::ValidateCodeInput::default()
    };
    let outcome =
        validate_code::validate_code(&registry, &Invocation::Type, &request).expect("validates");
    assert!(!outcome.result);
    let message = outcome.message.expect("a message");
    assert!(
        message.contains(
            "compositional grammar of the code system, which this server does not evaluate"
        ),
        "{message}"
    );

    // A code that is no expression at all stays the unknown-code outcome.
    let unknown = lookup::LookupInput {
        system: Some(SYSTEM.to_owned()),
        code: Some(String::from("138875004999")),
        ..lookup::LookupInput::default()
    };
    let error = lookup::lookup(&registry, &Invocation::Type, &unknown).expect_err("refuses");
    assert!(
        matches!(error, OperationError::UnknownCode { .. }),
        "{error:?}"
    );
}

/// `$expand` names SNOMED CT as a system whose value sets are "unbounded due to
/// the inclusion of post-coordinated value sets", the case `valueset-unclosed`
/// marks (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>, Notes);
/// the `expressions` filter is what keeps the expressions out
/// (<https://hl7.org/fhir/R4B/snomedct.html>, "Filter Properties").
#[test]
fn a_selection_is_unclosed_unless_it_states_that_expressions_are_excluded() {
    let (_dir, p) = provider();
    let descendants = Filter {
        property: String::from("concept"),
        op: FilterOperator::DescendentOf,
        value: sctid(item(ANIMAL)),
    };
    let no_expressions = Filter {
        property: String::from("expressions"),
        op: FilterOperator::Equal,
        value: String::from("false"),
    };
    assert!(p.unclosed(std::slice::from_ref(&descendants)));
    assert!(!p.unclosed(&[descendants, no_expressions]));
}
