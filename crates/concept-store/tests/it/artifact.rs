use std::path::Path;

use concept_graph::ordinal::Ordinal;
use concept_store::builder::{BuildError, PreferredRule, StoreBuilder};
use concept_store::record::{Concept, Designation, PropertyValue};
use concept_store::store::{Store, StoreError, Vocabulary};
use concept_store::tables;

const FSN: u32 = 0;
const SYNONYM: u32 = 1;
const EN_GB: u32 = 0;
const NL: u32 = 1;
const PREFERRED: u32 = 0;
const ACCEPTABLE: u32 = 1;
const STATUS: u32 = 0;
const PARENT: u32 = 1;

/// A two-concept synthetic artifact: a root with an FSN and two synonyms, and
/// a child with a Dutch synonym.
fn build(path: &Path) -> Store {
    let mut builder = StoreBuilder::create(
        path,
        "http://example.org/synthetic",
        "http://example.org/synthetic/version/20260101",
    )
    .expect("creates");
    builder
        .vocabulary(Vocabulary::DesignationUses, FSN, "fsn")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::DesignationUses, SYNONYM, "synonym")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::LanguageRefsets, EN_GB, "en-GB")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::LanguageRefsets, NL, "nl")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::Acceptabilities, PREFERRED, "preferred")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::Acceptabilities, ACCEPTABLE, "acceptable")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::PropertyKeys, STATUS, "status")
        .expect("vocab");
    builder
        .vocabulary(Vocabulary::PropertyKeys, PARENT, "parent")
        .expect("vocab");
    let root = Ordinal::new(0);
    let child = Ordinal::new(1);
    builder
        .concept(
            root,
            &Concept {
                code: "1000".to_owned(),
                active: true,
                effective_time: Some("20260101".to_owned()),
                module: None,
            },
        )
        .expect("concept");
    builder
        .concept(
            child,
            &Concept {
                code: "1001".to_owned(),
                active: false,
                effective_time: None,
                module: Some(root),
            },
        )
        .expect("concept");
    let d = |term: &str, language: &str, use_ordinal: u32| Designation {
        id: None,
        term: term.to_owned(),
        language: language.to_owned(),
        use_ordinal,
        active: true,
    };
    builder
        .designation(root, 0, &d("Synthetic root (synthetic)", "en", FSN))
        .expect("designation");
    builder
        .designation(root, 1, &d("Root synonym one", "en", SYNONYM))
        .expect("designation");
    builder
        .designation(root, 2, &d("Root synonym two", "en", SYNONYM))
        .expect("designation");
    builder
        .designation(child, 0, &d("Synthetisch kind", "nl", SYNONYM))
        .expect("designation");
    builder
        .acceptability(root, 0, EN_GB, PREFERRED)
        .expect("acceptability");
    builder
        .acceptability(root, 1, EN_GB, ACCEPTABLE)
        .expect("acceptability");
    builder
        .acceptability(root, 2, EN_GB, PREFERRED)
        .expect("acceptability");
    builder
        .acceptability(child, 0, NL, PREFERRED)
        .expect("acceptability");
    builder
        .properties(root, STATUS, &[PropertyValue::Code("active".to_owned())])
        .expect("properties");
    builder
        .properties(child, STATUS, &[PropertyValue::Code("inactive".to_owned())])
        .expect("properties");
    builder
        .properties(child, PARENT, &[PropertyValue::Concept(root)])
        .expect("properties");
    builder
        .finish(&PreferredRule {
            preferred: PREFERRED,
        })
        .expect("finishes");
    Store::open(path).expect("opens")
}

#[test]
fn point_reads_return_what_the_build_wrote() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = build(&dir.path().join("synthetic.redb"));
    assert_eq!(
        store.meta(tables::META_SYSTEM).expect("meta").as_deref(),
        Some("http://example.org/synthetic")
    );
    assert_eq!(
        store.meta(tables::META_CONCEPTS).expect("meta").as_deref(),
        Some("2")
    );
    assert_eq!(
        store.ordinal("1001").expect("ordinal"),
        Some(Ordinal::new(1))
    );
    assert_eq!(store.ordinal("9999").expect("ordinal"), None);
    let child = store
        .concept(Ordinal::new(1))
        .expect("read")
        .expect("present");
    assert_eq!(child.code, "1001");
    assert!(!child.active);
    assert_eq!(child.module, Some(Ordinal::new(0)));
    assert!(store.concept(Ordinal::new(7)).expect("read").is_none());
    let designations = store.designations(Ordinal::new(0)).expect("designations");
    assert_eq!(designations.len(), 3);
    assert_eq!(
        designations.first().map(|d| d.term.as_str()),
        Some("Synthetic root (synthetic)")
    );
    assert_eq!(
        store
            .acceptability(Ordinal::new(0), 1, EN_GB)
            .expect("acceptability"),
        Some(ACCEPTABLE)
    );
    let properties = store.properties(Ordinal::new(1)).expect("properties");
    assert_eq!(
        properties,
        vec![
            (STATUS, vec![PropertyValue::Code("inactive".to_owned())]),
            (PARENT, vec![PropertyValue::Concept(Ordinal::new(0))])
        ]
    );
    assert_eq!(
        store
            .vocabulary(Vocabulary::LanguageRefsets, NL)
            .expect("vocab")
            .as_deref(),
        Some("nl")
    );
    assert_eq!(
        store
            .vocabulary_ordinal(Vocabulary::PropertyKeys, "parent")
            .expect("vocab"),
        Some(PARENT)
    );
    assert_eq!(
        store
            .vocabulary_ordinal(Vocabulary::PropertyKeys, "nope")
            .expect("vocab"),
        None
    );
}

#[test]
fn preferred_designations_are_precomputed_per_refset_and_use() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = build(&dir.path().join("synthetic.redb"));
    let fsn = store
        .preferred(Ordinal::new(0), EN_GB, FSN)
        .expect("read")
        .expect("present");
    assert_eq!(fsn.term, "Synthetic root (synthetic)");
    // Two preferred synonyms: the lowest index wins, deterministically.
    let synonym = store
        .preferred(Ordinal::new(0), EN_GB, SYNONYM)
        .expect("read")
        .expect("present");
    assert_eq!(synonym.term, "Root synonym two");
    assert!(
        store
            .preferred(Ordinal::new(0), NL, SYNONYM)
            .expect("read")
            .is_none()
    );
    let dutch = store
        .preferred(Ordinal::new(1), NL, SYNONYM)
        .expect("read")
        .expect("present");
    assert_eq!(dutch.term, "Synthetisch kind");
}

#[test]
fn two_builds_of_the_same_input_are_byte_identical() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = dir.path().join("a.redb");
    let second = dir.path().join("b.redb");
    drop(build(&first));
    drop(build(&second));
    let a = std::fs::read(&first).expect("read");
    let b = std::fs::read(&second).expect("read");
    assert_eq!(a.len(), b.len());
    assert_eq!(a, b);
}

#[test]
fn a_foreign_or_mislabelled_file_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("other.redb");
    let db = redb::Database::create(&path).expect("creates");
    let txn = db.begin_write().expect("txn");
    {
        let mut meta = txn.open_table(tables::META).expect("table");
        meta.insert(tables::META_LAYOUT, "0").expect("insert");
    }
    txn.commit().expect("commit");
    drop(db);
    assert!(
        matches!(Store::open(&path), Err(StoreError::Layout { found: Some(ref f), .. }) if f == "0")
    );
    assert!(matches!(
        Store::open(&dir.path().join("missing.redb")),
        Err(StoreError::Open { .. })
    ));
}

#[test]
fn a_vocabulary_name_keeps_one_ordinal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut builder = StoreBuilder::create(&dir.path().join("v.redb"), "s", "v").expect("creates");
    builder
        .vocabulary(Vocabulary::PropertyKeys, 0, "status")
        .expect("first");
    builder
        .vocabulary(Vocabulary::PropertyKeys, 0, "status")
        .expect("same ordinal again is fine");
    assert!(matches!(
        builder.vocabulary(Vocabulary::PropertyKeys, 1, "status"),
        Err(BuildError::Vocabulary {
            existing: 0,
            requested: 1,
            ..
        })
    ));
}
