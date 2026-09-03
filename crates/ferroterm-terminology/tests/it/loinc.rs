//! The LOINC provider over the testkit's release-shaped artifact
//! (<https://hl7.org/fhir/R4B/loinc.html>).

use ferroterm_terminology::filter::{Filter, FilterOperator};
use ferroterm_terminology::loinc::{LoincProvider, OpenError, SYSTEM};
use ferroterm_terminology::provider::{CodeSystemProvider, Concept, ProviderError};
use ferroterm_testkit::loinc::{
    ANSWER_LIST, CHEMISTRY_PART, GLUCOSE, GLUCOSE_PART, NO, OLD_GLUCOSE, SODIUM, SURVEY, VERSION,
    YES, code, write_artifact,
};

fn provider() -> (tempfile::TempDir, LoincProvider) {
    let dir = tempfile::tempdir().expect("tempdir");
    write_artifact(dir.path()).expect("builds");
    let provider = LoincProvider::open(dir.path()).expect("opens");
    (dir, provider)
}

fn filter(property: &str, op: FilterOperator, value: &str) -> Filter {
    Filter {
        property: property.to_owned(),
        op,
        value: value.to_owned(),
    }
}

fn codes(provider: &LoincProvider, set: impl IntoIterator<Item = u32>) -> Vec<String> {
    let mut out: Vec<String> = set
        .into_iter()
        .filter_map(|i| provider.code(Concept::new(i)).expect("reads"))
        .collect();
    out.sort();
    out
}

#[test]
fn codes_locate_without_case_and_display_the_long_common_name() {
    let (_dir, provider) = provider();
    assert_eq!(provider.identity().url, SYSTEM);
    assert_eq!(provider.identity().version, VERSION);
    assert_eq!(provider.declaration().languages, ["en", "nl-NL"]);
    let glucose = provider
        .locate(&code(GLUCOSE))
        .expect("reads")
        .expect("glucose");
    let part = provider
        .locate(&code(GLUCOSE_PART).to_ascii_lowercase())
        .expect("reads")
        .expect("part, lower case");
    assert_eq!(
        part.code,
        code(GLUCOSE_PART),
        "the stored spelling comes back"
    );
    assert_eq!(
        provider
            .display(glucose.concept, None)
            .expect("reads")
            .as_deref(),
        Some("Glucose [Mass/volume] in Blood")
    );
    assert_eq!(
        provider
            .display(glucose.concept, Some("nl-NL"))
            .expect("reads")
            .as_deref(),
        Some("Glucose [massa/volume] in bloed")
    );
    assert_eq!(
        provider
            .display(glucose.concept, Some("de"))
            .expect("reads")
            .as_deref(),
        Some("Glucose [Mass/volume] in Blood"),
        "English when the language is missing"
    );
    assert_eq!(
        provider
            .display(part.concept, None)
            .expect("reads")
            .as_deref(),
        Some("Glucose")
    );
    let designations = provider
        .designations(glucose.concept, Some("nl"))
        .expect("reads");
    assert_eq!(
        designations.len(),
        3,
        "the Dutch long common name, short name, and display name"
    );
    assert!(provider.locate("99999-9").expect("reads").is_none());
    let old = provider
        .locate(&code(OLD_GLUCOSE))
        .expect("reads")
        .expect("old");
    let status = provider.status(old.concept).expect("reads");
    assert!(!status.active);
    assert_eq!(status.inactive_reason.as_deref(), Some("DEPRECATED"));
}

#[test]
fn properties_carry_every_field_and_the_hierarchy() {
    let (_dir, provider) = provider();
    let glucose = provider
        .locate(&code(GLUCOSE))
        .expect("reads")
        .expect("glucose");
    let props: Vec<String> = provider
        .properties(glucose.concept)
        .expect("reads")
        .into_iter()
        .map(|p| format!("{}={}", p.code, p.value.as_text()))
        .collect();
    assert!(props.contains(&String::from("CLASS=CHEM")));
    assert!(props.contains(&String::from("COMPONENT=Glucose")));
    assert!(props.contains(&String::from("copyright=LOINC")));
    assert!(props.contains(&format!("parent={}", code(GLUCOSE_PART))));
    assert!(props.contains(&String::from("inactive=false")));
    let sodium = provider
        .locate(&code(SODIUM))
        .expect("reads")
        .expect("sodium");
    let sodium_props: Vec<String> = provider
        .properties(sodium.concept)
        .expect("reads")
        .into_iter()
        .map(|p| format!("{}={}", p.code, p.value.as_text()))
        .collect();
    assert!(sodium_props.contains(&String::from("copyright=3rdParty")));
    let survey = provider
        .locate(&code(SURVEY))
        .expect("reads")
        .expect("survey");
    let survey_props: Vec<String> = provider
        .properties(survey.concept)
        .expect("reads")
        .into_iter()
        .map(|p| format!("{}={}", p.code, p.value.as_text()))
        .collect();
    assert!(survey_props.contains(&format!("answer-list={}", code(ANSWER_LIST))));
    let hierarchy = provider.hierarchy().expect("a hierarchy");
    let chemistry = provider
        .locate(&code(CHEMISTRY_PART))
        .expect("reads")
        .expect("chemistry");
    assert!(
        hierarchy
            .ancestors(glucose.concept)
            .contains(chemistry.concept.index())
    );
}

#[test]
fn filters_follow_the_fhir_page() {
    let (_dir, provider) = provider();
    let chem = provider
        .filter(&filter("CLASS", FilterOperator::Equal, "CHEM"))
        .expect("filters");
    let mut expected = vec![code(GLUCOSE), code(OLD_GLUCOSE), code(SODIUM)];
    expected.sort();
    assert_eq!(codes(&provider, chem), expected);
    let regex = provider
        .filter(&filter("COMPONENT", FilterOperator::Regex, "^Gluc"))
        .expect("filters");
    assert_eq!(regex.len(), 2);
    let third_party = provider
        .filter(&filter("copyright", FilterOperator::Equal, "3rdParty"))
        .expect("filters");
    assert_eq!(codes(&provider, third_party), [code(SODIUM)]);
    let parent = provider
        .filter(&filter(
            "parent",
            FilterOperator::Equal,
            &code(GLUCOSE_PART),
        ))
        .expect("filters");
    assert_eq!(
        parent.len(),
        2,
        "the two glucose terms sit directly under the part"
    );
    let ancestor = provider
        .filter(&filter(
            "ancestor",
            FilterOperator::Equal,
            &code(CHEMISTRY_PART),
        ))
        .expect("filters");
    assert_eq!(
        ancestor.len(),
        4,
        "the glucose part and the three terms under chemistry"
    );
    assert!(matches!(
        provider.filter(&filter("ancestor", FilterOperator::Equal, "LP99999-9")),
        Err(ProviderError::UnknownCode(_))
    ));
    assert_eq!(
        provider.all().expect("all").len(),
        10,
        "4 terms, 3 parts, 1 list, 2 answers"
    );
    let hits = provider.search("bloed", Some("nl")).expect("searches");
    assert_eq!(hits.len(), 1);
}

#[test]
fn implicit_value_sets_cover_all_answer_lists_and_parts() {
    let (_dir, provider) = provider();
    let all = provider
        .implicit_value_set("http://loinc.org/vs")
        .expect("implicit")
        .expect("compose");
    assert!(all.include[0].filters.is_empty() && all.include[0].concepts.is_empty());
    let list = provider
        .implicit_value_set(&format!("http://loinc.org/vs/{}", code(ANSWER_LIST)))
        .expect("implicit")
        .expect("compose");
    let answers: Vec<&str> = list.include[0]
        .concepts
        .iter()
        .map(|c| c.code.as_str())
        .collect();
    assert_eq!(answers, [code(YES), code(NO)]);
    let part = provider
        .implicit_value_set(&format!(
            "http://loinc.org/vs/{}",
            code(GLUCOSE_PART).to_ascii_lowercase()
        ))
        .expect("implicit")
        .expect("compose");
    assert_eq!(part.include[0].filters[0].property, "ancestor");
    assert!(
        provider
            .implicit_value_set("http://loinc.org/vs/LL99999-9")
            .expect("implicit")
            .is_err()
    );
    assert!(
        provider
            .implicit_value_set(&format!("http://loinc.org/vs/{}", code(GLUCOSE)))
            .expect("implicit")
            .is_err(),
        "a term is neither a list nor a part"
    );
    assert!(
        provider
            .implicit_value_set("http://loinc.org/other")
            .is_none()
    );
    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::snomed::write(dir.path()).expect("writes");
    assert!(matches!(
        LoincProvider::open(dir.path()),
        Err(OpenError::NotLoinc(_))
    ));
}
