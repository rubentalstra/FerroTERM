//! The pipeline end to end over the synthetic release: deterministic output,
//! the manifest, and the artifacts opened by the store, graph, and text crates.

use std::fs;

use ferroterm_build::pipeline::{self, MANIFEST_FILE, STORE_FILE};
use ferroterm_graph::persist::Hierarchy;
use ferroterm_store::store::{Store, Vocabulary};
use ferroterm_store::tables;
use ferroterm_text::index::Query;
use ferroterm_text::persist::read_from;
use serde_json::Value;

use crate::fixture::{self, DATE, GB_LANGUAGE_REFSET, NL_LANGUAGE_REFSET, concept};

#[test]
fn two_builds_of_the_same_release_are_byte_identical() {
    let release = tempfile::tempdir().expect("tempdir");
    fixture::write_release(release.path());
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");
    let a = pipeline::build(release.path(), first.path()).expect("first build");
    let b = pipeline::build(release.path(), second.path()).expect("second build");
    assert_eq!(
        (a.concepts, a.designations, a.is_a_edges),
        (b.concepts, b.designations, b.is_a_edges)
    );
    for file in [STORE_FILE, MANIFEST_FILE] {
        assert_eq!(
            fs::read(first.path().join(file)).expect("first"),
            fs::read(second.path().join(file)).expect("second"),
            "{file} differs between two builds"
        );
    }
}

#[test]
fn the_manifest_records_the_edition_and_the_counts() {
    let release = tempfile::tempdir().expect("tempdir");
    fixture::write_release(release.path());
    let out = tempfile::tempdir().expect("tempdir");
    let report = pipeline::build(release.path(), out.path()).expect("build");
    let module = fixture::module();
    assert_eq!(
        report.edition_uri,
        format!("http://snomed.info/sct/{module}")
    );
    assert_eq!(
        report.version_uri,
        format!("http://snomed.info/sct/{module}/version/{DATE}")
    );
    assert_eq!(
        (report.concepts, report.designations, report.is_a_edges),
        (9, 12, 4)
    );
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(&report.manifest).expect("manifest"))
            .expect("json");
    assert_eq!(manifest["manifest"], pipeline::MANIFEST_VERSION);
    assert_eq!(manifest["hierarchy"], "hierarchy.bin");
    assert_eq!(manifest["text"], "text.bin");
    assert_eq!(manifest["system"], "http://snomed.info/sct");
    assert_eq!(manifest["version"], report.version_uri);
    assert_eq!(manifest["releaseDate"], DATE);
    assert_eq!(manifest["store"], STORE_FILE);
    assert_eq!(manifest["concepts"], 9);
    assert_eq!(manifest["designations"], 12);
    assert_eq!(manifest["isAEdges"], 4);
    assert_eq!(manifest["languages"], serde_json::json!(["en", "nl"]));
    assert_eq!(report.languages, ["en", "nl"]);
}

#[test]
fn the_store_graph_and_text_crates_open_what_the_build_wrote() {
    let release = tempfile::tempdir().expect("tempdir");
    fixture::write_release(release.path());
    let out = tempfile::tempdir().expect("tempdir");
    let report = pipeline::build(release.path(), out.path()).expect("build");
    let store = Store::open(&report.store).expect("store opens");
    assert_eq!(
        store.meta(tables::META_VERSION).expect("read"),
        Some(report.version_uri.clone())
    );

    // Point reads: the cat, its Dutch preferred synonym, its parent property.
    let cat = store.ordinal(&concept(3)).expect("read").expect("cat");
    let animal = store.ordinal(&concept(2)).expect("read").expect("animal");
    let fish = store.ordinal(&concept(5)).expect("read").expect("fish");
    assert!(store.concept(cat).expect("read").expect("cat").active);
    assert!(!store.concept(fish).expect("read").expect("fish").active);
    let nl = store
        .vocabulary_ordinal(Vocabulary::LanguageRefsets, NL_LANGUAGE_REFSET)
        .expect("read")
        .expect("nl refset");
    let gb = store
        .vocabulary_ordinal(Vocabulary::LanguageRefsets, GB_LANGUAGE_REFSET)
        .expect("read")
        .expect("gb refset");
    assert_eq!(
        store
            .preferred(cat, nl, 1)
            .expect("read")
            .expect("nl synonym")
            .term,
        "Kat"
    );
    assert_eq!(
        store
            .preferred(cat, gb, 1)
            .expect("read")
            .expect("gb synonym")
            .term,
        "Cat"
    );
    assert_eq!(
        store
            .preferred(cat, gb, 0)
            .expect("read")
            .expect("gb fsn")
            .term,
        "Cat (synthetic)"
    );
    let parent_key = store
        .vocabulary_ordinal(Vocabulary::PropertyKeys, "parent")
        .expect("read")
        .expect("parent key");
    let parents = |ordinal| {
        store
            .properties(ordinal)
            .expect("read")
            .into_iter()
            .find(|(key, _)| *key == parent_key)
            .map(|(_, values)| values)
    };
    assert_eq!(
        parents(cat),
        Some(vec![ferroterm_store::record::PropertyValue::Concept(
            animal
        )])
    );
    assert!(parents(fish).is_none(), "the inactive edge is not a parent");

    // Attribute properties keyed by the attribute SCTID; concrete values typed.
    let property = |ordinal, key_name: &str| {
        let key = store
            .vocabulary_ordinal(Vocabulary::PropertyKeys, key_name)
            .expect("read")
            .expect("key");
        store
            .properties(ordinal)
            .expect("read")
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, values)| values)
    };
    let fur = store.ordinal(&concept(7)).expect("read").expect("fur");
    assert_eq!(
        property(cat, &concept(6)),
        Some(vec![ferroterm_store::record::PropertyValue::Concept(fur)])
    );
    assert_eq!(
        property(cat, &concept(8)),
        Some(vec![ferroterm_store::record::PropertyValue::Decimal(
            String::from("4")
        )])
    );
    let dog = store.ordinal(&concept(4)).expect("read").expect("dog");
    assert!(
        property(dog, &concept(6)).is_none(),
        "the inactive attribute is not stored"
    );
}

#[test]
fn the_graph_and_text_files_open_beside_the_store() {
    let release = tempfile::tempdir().expect("tempdir");
    fixture::write_release(release.path());
    let out = tempfile::tempdir().expect("tempdir");
    let report = pipeline::build(release.path(), out.path()).expect("build");
    let store = Store::open(&report.store).expect("store opens");
    let cat = store.ordinal(&concept(3)).expect("read").expect("cat");
    let animal = store.ordinal(&concept(2)).expect("read").expect("animal");
    let fish = store.ordinal(&concept(5)).expect("read").expect("fish");
    let nl = store
        .vocabulary_ordinal(Vocabulary::LanguageRefsets, NL_LANGUAGE_REFSET)
        .expect("read")
        .expect("nl refset");

    // The hierarchy file: cat is under animal and the root; fish is under nothing.
    let graph = std::fs::read(&report.hierarchy).expect("hierarchy file");
    let hierarchy = Hierarchy::read_from(&mut graph.as_slice()).expect("hierarchy reads");
    let top = store.ordinal(&concept(1)).expect("read").expect("top");
    assert!(hierarchy.closure.is_ancestor(animal, cat));
    assert!(hierarchy.closure.is_ancestor(top, cat));
    assert!(hierarchy.closure.ancestors(fish).is_empty());
    assert_eq!(hierarchy.is_a.nodes(), 9);

    // The text file: a Dutch prefix, filtered by the NL refset, ranks the shortest first.
    let text = std::fs::read(&report.text).expect("text file");
    let index = read_from(&mut text.as_slice()).expect("index reads");
    assert_eq!(index.len(), 12);
    let hits = index.search(
        &Query {
            text: String::from("po"),
            refset: Some(nl),
            active_only: true,
            ..Query::default()
        },
        0,
        10,
    );
    assert_eq!(hits.total, 1);
    let entry = index.entry(hits.designations[0]).expect("entry");
    assert_eq!(entry.concept, cat);
    let synth = index.search(
        &Query {
            text: String::from("synth"),
            ..Query::default()
        },
        0,
        10,
    );
    assert_eq!(synth.total, 5, "every FSN carries the semantic tag");
}

#[test]
fn a_release_without_module_dependencies_is_refused() {
    let release = tempfile::tempdir().expect("tempdir");
    fixture::write_release(release.path());
    let dependency = fs::read_dir(release.path().join("Snapshot/Refset/Metadata"))
        .expect("dir")
        .next()
        .expect("file")
        .expect("entry")
        .path();
    fs::remove_file(dependency).expect("remove");
    let out = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        pipeline::build(release.path(), out.path()),
        Err(pipeline::Error::MissingFile(_))
    ));
}
