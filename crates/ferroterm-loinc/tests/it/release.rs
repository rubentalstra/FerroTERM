//! Reading the synthetic LOINC-shaped release.

use ferroterm_loinc::release::Release;
use ferroterm_loinc::{answer, part, term, variant};
use ferroterm_testkit::loinc::{
    ANSWER_LIST, GLUCOSE, GLUCOSE_PART, OLD_GLUCOSE, SODIUM, SURVEY, code, write_release,
};

#[test]
fn every_file_of_the_release_reads_by_column_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_release(dir.path()).expect("writes");
    let release = Release::open(dir.path()).expect("opens");
    let terms = term::read(&release).expect("terms");
    assert_eq!(terms.rows.len(), 4);
    assert!(terms.columns.iter().any(|c| c == "LONG_COMMON_NAME"));
    let glucose = terms
        .rows
        .iter()
        .find(|t| t.code == code(GLUCOSE))
        .expect("glucose");
    assert_eq!(glucose.long_common_name, "Glucose [Mass/volume] in Blood");
    assert_eq!(
        glucose.fields.get("CLASS").map(String::as_str),
        Some("CHEM")
    );
    assert!(glucose.active());
    assert!(
        !terms
            .rows
            .iter()
            .find(|t| t.code == code(OLD_GLUCOSE))
            .expect("old")
            .active()
    );
    assert!(
        terms
            .rows
            .iter()
            .find(|t| t.code == code(SODIUM))
            .expect("sodium")
            .external_copyright
            .is_some()
    );
    let parts = part::read_parts(&release).expect("parts");
    assert_eq!(parts.len(), 3);
    let edges = part::read_hierarchy(&release).expect("hierarchy");
    assert_eq!(edges.len(), 6);
    assert!(edges.iter().any(
        |e| e.code == code(GLUCOSE) && e.parent.as_deref() == Some(code(GLUCOSE_PART).as_str())
    ));
    assert!(
        edges.iter().any(|e| e.parent.is_none()),
        "a root has no parent"
    );
    let lists = answer::read(&release).expect("answers");
    let list = lists.get(&code(ANSWER_LIST)).expect("list");
    assert_eq!(list.answers.len(), 2);
    assert_eq!(list.terms, [code(SURVEY)]);
    let variants = variant::read(&release).expect("variants");
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].language, "nl-NL");
    assert_eq!(
        variants[0]
            .terms
            .get(&code(GLUCOSE))
            .and_then(|t| t.long_common_name.as_deref()),
        Some("Glucose [massa/volume] in bloed")
    );
}

#[test]
fn a_release_without_the_term_table_or_with_a_bad_code_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(Release::open(dir.path()).is_err());
    write_release(dir.path()).expect("writes");
    let path = dir.path().join("LoincTable/Loinc.csv");
    let text = std::fs::read_to_string(&path)
        .expect("reads")
        .replace(&code(GLUCOSE), "90001-0");
    std::fs::write(&path, text).expect("writes");
    let release = Release::open(dir.path()).expect("opens");
    assert!(matches!(
        term::read(&release),
        Err(ferroterm_loinc::release::ReleaseError::Code { .. })
    ));
}
