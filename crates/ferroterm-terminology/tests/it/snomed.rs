//! The SNOMED CT provider through the seam, over the synthetic artifact.

use std::sync::Arc;

use ferroterm_graph::subsumption::Outcome;
use ferroterm_terminology::capabilities::Summary;
use ferroterm_terminology::filter::{Filter, FilterOperator};
use ferroterm_terminology::provider::{Capability, CodeSystemProvider, Concept, PropertyValue};
use ferroterm_terminology::registry::Registry;
use ferroterm_terminology::snomed::{OpenError, SYSTEM, SnomedProvider};

use ferroterm_testkit::snomed;
use ferroterm_testkit::snomed::{
    ANIMAL, CAT, COVERING, DOG, EDITION, FISH, FUR, LEGS, TOP, VERSION, item, sctid,
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
    assert_eq!(p.all().expect("all").len(), 8);
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
        [CAT, DOG, FUR, COVERING, LEGS]
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
        6,
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
    assert!(system.versions[0].compositional);
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
        r#"{"system":"http://loinc.org","edition":"x","version":"2.80"}"#,
    )
    .expect("writes");
    assert!(
        matches!(SnomedProvider::open(dir.path(), "en"), Err(OpenError::NotSnomed(s)) if s == "http://loinc.org")
    );
}
