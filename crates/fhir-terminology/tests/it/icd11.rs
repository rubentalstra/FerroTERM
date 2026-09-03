//! The ICD-11 provider over the testkit's API-shaped artifacts, asserting
//! what the HL7 terminology ecosystem test cases for ICD-11 assert.

use ferroterm_testkit::icd11::{BLOCK, CHOLERA, RELEASE, SEPSIS, VIBRIO, write_artifacts};
use fhir_terminology::filter::{Filter, FilterOperator};
use fhir_terminology::icd11::{Icd11Provider, OpenError};
use fhir_terminology::provider::{
    Capability, CodeSystemProvider, Concept, HierarchyMeaning, PropertyValue, ProviderError,
};

const MMS: &str = "http://id.who.int/icd/release/11/mms";

fn providers() -> (
    tempfile::TempDir,
    Icd11Provider,
    Icd11Provider,
    Icd11Provider,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    write_artifacts(dir.path()).expect("builds");
    let mms = Icd11Provider::open(&dir.path().join("mms")).expect("opens mms");
    let icf = Icd11Provider::open(&dir.path().join("icf")).expect("opens icf");
    let foundation = Icd11Provider::open(&dir.path().join("entity")).expect("opens entity");
    (dir, mms, icf, foundation)
}

fn located(provider: &Icd11Provider, code: &str) -> Concept {
    provider
        .locate(code)
        .expect("reads")
        .expect("a concept with the code")
        .concept
}

fn props(provider: &Icd11Provider, concept: Concept) -> Vec<String> {
    provider
        .properties(concept)
        .expect("reads")
        .into_iter()
        .map(|p| {
            let parts: Vec<String> = p
                .subproperties
                .iter()
                .map(|s| format!("{}={}", s.code, s.value.as_text()))
                .collect();
            if parts.is_empty() {
                format!("{}={}", p.code, p.value.as_text())
            } else {
                format!("{}={} [{}]", p.code, p.value.as_text(), parts.join("; "))
            }
        })
        .collect()
}

#[test]
fn codes_and_entity_uris_in_both_forms_name_the_same_concept() {
    let (_dir, mms, _icf, foundation) = providers();
    assert_eq!(mms.identity().url, MMS);
    assert_eq!(mms.identity().version, RELEASE);
    assert_eq!(
        mms.identity().title.as_deref(),
        Some("ICD-11 for Mortality and Morbidity Statistics")
    );
    assert_eq!(
        mms.declaration().hierarchy_meaning,
        Some(HierarchyMeaning::ClassifiedWith)
    );
    assert!(mms.declaration().compositional);
    assert!(mms.declaration().case_sensitive);
    assert_eq!(mms.declaration().languages, ["en", "fr"]);
    let by_code = located(&mms, "1A00");
    let by_uri = located(&mms, &format!("{MMS}/{CHOLERA}"));
    let by_versioned = located(
        &mms,
        &format!("http://id.who.int/icd/release/11/{RELEASE}/mms/{CHOLERA}"),
    );
    assert_eq!(by_code, by_uri);
    assert_eq!(by_code, by_versioned);
    assert_eq!(
        mms.locate("1A00").expect("reads").expect("located").code,
        "1A00"
    );
    assert!(
        mms.locate("1a00").expect("reads").is_none(),
        "case sensitive"
    );
    assert!(mms.locate("XXXX9").expect("reads").is_none());
    assert_eq!(
        mms.display(by_code, None).expect("reads").as_deref(),
        Some("Cholera")
    );
    assert_eq!(
        mms.display(by_code, Some("fr")).expect("reads").as_deref(),
        Some("Choléra")
    );
    assert_eq!(
        mms.display(by_code, Some("de")).expect("reads").as_deref(),
        Some("Cholera"),
        "English when the language is missing"
    );
    let p = props(&mms, by_code);
    assert!(p.contains(&String::from("code=1A00")));
    assert!(p.contains(&format!("id={MMS}/{CHOLERA}")));
    assert!(p.contains(&format!("parent={MMS}/{BLOCK}")));
    assert!(p.contains(&String::from("classKind=category")));
    assert!(p.contains(&String::from("exclusion=Vibrio vulnificus infection")));
    assert!(p.iter().any(|x| x.starts_with(
        "postcoordinationScale=http://id.who.int/icd/schema/infectiousAgent [valueSet="
    )));
    assert!(!p.iter().any(|x| x.starts_with("notSelectable")));
    assert_eq!(
        mms.definition(by_code).expect("reads").as_deref(),
        Some("An infection of the intestine by Vibrio cholerae.")
    );
    let block = located(&mms, &format!("{MMS}/{BLOCK}"));
    let block_props = props(&mms, block);
    assert!(block_props.contains(&String::from("notSelectable=true")));
    assert!(!block_props.iter().any(|x| x.starts_with("code=")));
    assert!(mms.status(block).expect("reads").abstract_concept);
    assert_eq!(
        mms.display(block, None).expect("reads").as_deref(),
        Some("Bacterial intestinal infections")
    );
    let residual = located(&mms, "1A0Y");
    assert!(props(&mms, residual).contains(&format!("id={MMS}/1001/other")));
    assert_eq!(located(&mms, &format!("{MMS}/1001/other")), residual);

    let entity = located(
        &foundation,
        &format!("http://id.who.int/icd/entity/{CHOLERA}"),
    );
    assert_eq!(
        foundation.display(entity, None).expect("reads").as_deref(),
        Some("Cholera")
    );
    assert!(
        props(&foundation, entity)
            .contains(&format!("parent=http://id.who.int/icd/entity/{BLOCK}"))
    );
    assert!(
        foundation.locate(CHOLERA).expect("reads").is_none(),
        "a bare number is not a Foundation code"
    );
    assert!(
        foundation
            .locate("http://id.who.int/icd/entity/1001/other")
            .expect("reads")
            .is_none(),
        "the Foundation has no residuals"
    );
    assert!(
        !foundation
            .declaration()
            .capabilities
            .contains(&Capability::ImplicitValueSets)
    );
}

#[test]
fn postcoordination_expressions_validate_against_the_axes() {
    let (_dir, mms, icf, _foundation) = providers();
    let simple = located(&mms, "1A00&XN8P1");
    assert_eq!(
        mms.display(simple, None).expect("reads").as_deref(),
        Some("Cholera [Vibrio cholerae O1, biovar cholerae]")
    );
    assert_eq!(
        mms.code(simple).expect("reads").as_deref(),
        Some("1A00&XN8P1")
    );
    let p = props(&mms, simple);
    assert!(p.contains(&String::from("code=1A00&XN8P1")));
    assert!(p.contains(&format!("id={MMS}/{CHOLERA} & {MMS}/2001")));
    assert!(p.contains(&format!(
        "stem=1A00 [stemLabel=Cholera; stemUri={MMS}/{CHOLERA}]"
    )));
    assert!(p.contains(&format!(
        "postcoordinationValues=http://id.who.int/icd/schema/infectiousAgent [code=XN8P1; description=Vibrio cholerae O1, biovar cholerae; value={MMS}/2001]"
    )));
    let uri_form = located(&mms, &format!("{MMS}/{CHOLERA} & {MMS}/2001"));
    assert_eq!(
        mms.display(uri_form, None).expect("reads").as_deref(),
        Some("Cholera [Vibrio cholerae O1, biovar cholerae]")
    );
    assert!(props(&mms, uri_form).contains(&String::from("code=1A00&XN8P1")));
    assert!(
        mms.locate("1A00&XN8P1&XN62R").expect("reads").is_some(),
        "a second value on an AllowAlways axis"
    );
    assert!(matches!(
        mms.locate("1A00&1G41"),
        Err(ProviderError::InvalidCode { ref reason, .. }) if reason.contains("1G41")
    ));
    assert!(
        mms.locate("1A00&XXXX9").is_err(),
        "an unknown value is an error"
    );
    assert!(
        mms.locate("XXXX9&XN8P1").expect("reads").is_none(),
        "an unknown stem is not a code"
    );
    let cluster = located(&mms, "1A01/1G41/1G40");
    assert_eq!(
        mms.display(cluster, None).expect("reads").as_deref(),
        Some(
            "Intestinal infection due to other Vibrio / Sepsis with septic shock / Sepsis without septic shock"
        )
    );
    let p = props(&mms, cluster);
    assert!(p.contains(&String::from("code=1A01/1G41/1G40")));
    assert!(p.contains(&format!(
        "postcoordinationValues=http://id.who.int/icd/schema/hasCausingCondition [code=1G41; description=Sepsis with septic shock; value={MMS}/3001]"
    )), "the required axis gets the first value: {p:?}");
    assert!(p.contains(&format!(
        "postcoordinationValues=http://id.who.int/icd/schema/hasManifestation [code=1G40; description=Sepsis without septic shock; value={MMS}/{SEPSIS}]"
    )));
    let two_stems = located(&mms, "1A00&XN8P1/XN62R");
    assert_eq!(
        mms.display(two_stems, None).expect("reads").as_deref(),
        Some("Cholera [Vibrio cholerae O1, biovar cholerae] / Vibrio cholerae O1, biovar eltor"),
        "the second member fits the unfilled infectious-agent axis or starts a stem; the display renders the syntax either way"
    );
    let dot = located(&icf, "d5409.qp3");
    assert_eq!(
        icf.display(dot, None).expect("reads").as_deref(),
        Some("Dressing, unspecified [SEVERE performance difficulty (high, extreme,...) 50-95 %]")
    );
    assert!(props(&icf, dot).iter().any(|x| x.starts_with(
        "postcoordinationValues=http://id.who.int/icd/schema/performance [code=qp3;"
    )));
    assert!(
        icf.locate("d5409.3").is_err(),
        "the pre-2026 qualifier syntax names no code"
    );
    assert_eq!(located(&icf, "d540"), located(&icf, "d540"));
}

#[test]
fn scales_are_implicit_value_sets_and_the_tree_answers_filters() {
    let (_dir, mms, _icf, _foundation) = providers();
    let url = format!("{MMS}/{CHOLERA}/postcoordinationScale/infectiousAgent");
    let compose = mms
        .implicit_value_set(&url)
        .expect("implicit")
        .expect("compose");
    assert_eq!(compose.include.len(), 1);
    assert_eq!(compose.include[0].filters[0].op, FilterOperator::IsA);
    assert_eq!(
        compose.include[0].filters[0].value,
        format!("{MMS}/{VIBRIO}")
    );
    assert_eq!(
        compose.include[0]
            .system
            .as_ref()
            .and_then(|s| s.version.as_deref()),
        Some(RELEASE)
    );
    assert!(
        mms.implicit_value_set(&format!(
            "http://id.who.int/icd/release/11/{RELEASE}/mms/{CHOLERA}/postcoordinationScale/infectiousAgent"
        ))
        .is_some(),
        "the versioned form"
    );
    assert!(
        mms.implicit_value_set(&format!(
            "{MMS}/999999999/postcoordinationScale/infectiousAgent"
        ))
        .is_none(),
        "an unknown entity is an unknown value set"
    );
    assert!(
        mms.implicit_value_set(&format!("{MMS}/{CHOLERA}/postcoordinationScale/laterality"))
            .is_none()
    );
    let under = mms
        .filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::IsA,
            value: format!("{MMS}/{VIBRIO}"),
        })
        .expect("filters");
    assert_eq!(under.len(), 4);
    let by_code = mms
        .filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::IsA,
            value: String::from("XN7N1"),
        })
        .expect("filters");
    assert_eq!(under, by_code);
    let hierarchy = mms.hierarchy().expect("tree");
    let chapter = located(&mms, "01");
    let cholera = located(&mms, "1A00");
    assert!(hierarchy.ancestors(cholera).contains(chapter.index()));
    assert!(
        hierarchy.ancestors(located(&mms, "1A00&XN8P1")).is_empty(),
        "an expression has no place in the tree"
    );
    assert_eq!(mms.all().expect("all").len(), 12);
    assert_eq!(mms.search("chol", Some("fr")).expect("searches").len(), 1);
    assert_eq!(mms.search("asiatic", None).expect("searches").len(), 1);
    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::loinc::write_artifact(dir.path()).expect("builds");
    assert!(matches!(
        Icd11Provider::open(dir.path()),
        Err(OpenError::NotIcd11(_))
    ));
    assert_eq!(PropertyValue::Uri(String::from("u")).as_text(), "u");
}
