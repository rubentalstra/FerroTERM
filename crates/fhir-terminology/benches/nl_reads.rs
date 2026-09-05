//! Read-path latency over the local NL edition artifact (`artifacts/nl`),
//! when present; the bar is one millisecond per read
//! (`docs/architecture.md`). Skips silently without the artifact.
#![allow(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "a benchmark harness fails loud and reports to stderr, like a test binary"
)]

use std::path::PathBuf;
use std::sync::Arc;

use concept_graph::ordinal::{self, Ordinal};
use concept_store::store::Store;
use criterion::{Criterion, criterion_group, criterion_main};
use fhir_terminology::compose::{Expander, Options};
use fhir_terminology::filter::{Filter, FilterOperator};
use fhir_terminology::operations::{Invocation, lookup, subsumes, validate_code};
use fhir_terminology::provider::CodeSystemProvider;
use fhir_terminology::registry::Registry;
use fhir_terminology::snomed::SnomedProvider;

const SCT: &str = "http://snomed.info/sct";
/// `138875005 |SNOMED CT Concept|`, the root: the one SCTID every edition has.
const ROOT: &str = "138875005";
/// `404684003 |Clinical finding|`, a top-level hierarchy present in every edition.
const FINDING: &str = "404684003";
/// `73211009 |Diabetes mellitus|`, deep in the tree with few children.
const LEAF: &str = "73211009";

/// A dense offsets-and-blob code column, the layout a dense integer key
/// admits, built here to measure what the b-tree costs (#304).
struct Column {
    offsets: Vec<u32>,
    blob: String,
}

impl Column {
    fn code(&self, ordinal: Ordinal) -> &str {
        let at = ordinal::to_usize(ordinal.index());
        let start = self.offsets.get(at).copied().unwrap_or_default();
        let end = self.offsets.get(at + 1).copied().unwrap_or_default();
        self.blob
            .get(ordinal::to_usize(start)..ordinal::to_usize(end))
            .unwrap_or_default()
    }
}

fn column(store: &Store, count: u32) -> Column {
    let codes = store
        .codes((0..count).map(Ordinal::new))
        .expect("the store reads");
    let mut offsets = Vec::with_capacity(codes.len() + 1);
    let mut blob = String::new();
    for code in codes {
        offsets.push(u32::try_from(blob.len()).expect("the blob fits u32"));
        blob.push_str(code.as_deref().unwrap_or_default());
    }
    offsets.push(u32::try_from(blob.len()).expect("the blob fits u32"));
    Column { offsets, blob }
}

fn artifact() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/nl");
    dir.join("manifest.json").exists().then_some(dir)
}

#[expect(
    clippy::too_many_lines,
    reason = "one benchmark group per read, read top to bottom"
)]
fn reads(c: &mut Criterion) {
    let Some(dir) = artifact() else {
        eprintln!("no artifacts/nl: skipping the NL read benchmarks");
        return;
    };
    let provider = SnomedProvider::open(&dir, "en").expect("the NL artifact opens");
    let root = provider.locate(ROOT).expect("reads").expect("root").concept;
    let finding = provider
        .locate(FINDING)
        .expect("reads")
        .expect("finding")
        .concept;
    let hierarchy = provider.hierarchy().expect("snomed has a hierarchy");
    let mut group = c.benchmark_group("provider");
    group.bench_function("locate", |b| {
        b.iter(|| provider.locate(FINDING).expect("reads"));
    });
    group.bench_function("display_nl", |b| {
        b.iter(|| provider.display(finding, Some("nl")).expect("reads"));
    });
    group.bench_function("designations", |b| {
        b.iter(|| provider.designations(finding, None).expect("reads"));
    });
    group.bench_function("properties", |b| {
        b.iter(|| provider.properties(finding).expect("reads"));
    });
    // The children of a concept are a `child` property each, and each code is a
    // store read, so the cost of `$lookup` follows the child count (#304).
    let children = provider
        .hierarchy()
        .expect("snomed has a hierarchy")
        .children(finding)
        .len();
    eprintln!("the finding has {children} children");
    let leaf = provider.locate(LEAF).expect("reads").expect("leaf").concept;
    group.bench_function("properties_leaf", |b| {
        b.iter(|| provider.properties(leaf).expect("reads"));
    });
    group.bench_function("subsumes", |b| b.iter(|| hierarchy.subsumes(root, finding)));
    // The three reads a page materializes per item (#304).
    group.bench_function("code", |b| {
        b.iter(|| provider.code(finding).expect("reads"));
    });
    group.bench_function("display_none", |b| {
        b.iter(|| provider.display(finding, None).expect("reads"));
    });
    group.bench_function("display_en", |b| {
        b.iter(|| provider.display(finding, Some("en")).expect("reads"));
    });
    group.bench_function("status", |b| {
        b.iter(|| provider.status(finding).expect("reads"));
    });
    group.finish();

    // The two batch reads behind the `child` properties, over the same
    // ordinals: the whole record against the code alone (#304).
    let ordinals: Vec<Ordinal> = hierarchy
        .children(finding)
        .iter()
        .map(Ordinal::new)
        .collect();
    let store = Store::open(&dir.join("store.redb")).expect("the NL store opens");
    let mut group = c.benchmark_group("store");
    group.bench_function("concepts_children", |b| {
        b.iter(|| store.concepts(ordinals.iter().copied()).expect("reads"));
    });
    group.bench_function("codes_children", |b| {
        b.iter(|| store.codes(ordinals.iter().copied()).expect("reads"));
    });
    // The same reads against a dense offsets-and-blob column, the layout a
    // dense integer key admits: what the b-tree descent costs, measured (#304).
    let count: u32 = store
        .meta(concept_store::tables::META_CONCEPTS)
        .expect("reads")
        .and_then(|c| c.parse().ok())
        .expect("the artifact records its concept count");
    let column = column(&store, count);
    group.bench_function("column_children", |b| {
        b.iter(|| {
            ordinals
                .iter()
                .map(|o| column.code(*o))
                .collect::<Vec<&str>>()
        });
    });
    group.finish();

    let mut group = c.benchmark_group("provider");
    group.bench_function("search_hart_nl", |b| {
        b.iter(|| provider.search("hart", Some("nl")).expect("reads"));
    });
    // The steps a page of an expansion pays before it cuts anything (#309).
    let isa = Filter {
        property: String::from("concept"),
        op: FilterOperator::IsA,
        value: FINDING.to_owned(),
    };
    group.bench_function("filter_isa_finding", |b| {
        b.iter(|| provider.filter(&isa).expect("filters"));
    });
    group.bench_function("parents", |b| {
        b.iter(|| hierarchy.parents(finding));
    });
    group.bench_function("all", |b| b.iter(|| provider.all().expect("enumerates")));
    group.bench_function("inactive", |b| {
        b.iter(|| provider.inactive().expect("enumerates"));
    });
    group.finish();

    let mut registry = Registry::new();
    registry.register(Arc::new(provider)).expect("registers");
    let mut group = c.benchmark_group("operations");
    let lookup_request = lookup::LookupInput {
        system: Some(SCT.to_owned()),
        code: Some(FINDING.to_owned()),
        display_language: Some(String::from("nl")),
        ..lookup::LookupInput::default()
    };
    group.bench_function("lookup", |b| {
        b.iter(|| lookup::lookup(&registry, &Invocation::Type, &lookup_request).expect("looks up"));
    });
    let validate_request = validate_code::ValidateCodeInput {
        url: Some(SCT.to_owned()),
        code: Some(FINDING.to_owned()),
        display: Some(String::from("Clinical finding")),
        ..validate_code::ValidateCodeInput::default()
    };
    group.bench_function("validate_code", |b| {
        b.iter(|| {
            validate_code::validate_code(&registry, &Invocation::Type, &validate_request)
                .expect("validates")
        });
    });
    let subsumes_request = subsumes::SubsumesInput {
        system: Some(SCT.to_owned()),
        code_a: Some(ROOT.to_owned()),
        code_b: Some(FINDING.to_owned()),
        ..subsumes::SubsumesInput::default()
    };
    group.bench_function("subsumes", |b| {
        b.iter(|| {
            subsumes::subsumes(&registry, &Invocation::Type, &subsumes_request).expect("subsumes")
        });
    });
    group.finish();

    // A paged expansion over a large set: the bar is ten milliseconds, so the
    // page must be cut from the bitmaps before any concept is read (#129).
    let mut group = c.benchmark_group("expand");
    let expander = Expander::new(&registry);
    let finding_set = registry
        .implicit_value_set(&format!("{SCT}?fhir_vs=isa/{FINDING}"))
        .expect("snomed claims the URI")
        .expect("well formed");
    let everything = registry
        .implicit_value_set(&format!("{SCT}?fhir_vs"))
        .expect("snomed claims the URI")
        .expect("well formed");
    let page = Options {
        count: Some(10),
        offset: 1000,
        ..Options::default()
    };
    group.bench_function("isa_finding_page_10", |b| {
        b.iter(|| expander.expand(&finding_set, &page).expect("expands"));
    });
    group.bench_function("all_concepts_page_10", |b| {
        b.iter(|| expander.expand(&everything, &page).expect("expands"));
    });
    let thousand = Options {
        count: Some(1000),
        offset: 0,
        ..Options::default()
    };
    group.bench_function("isa_finding_page_1000", |b| {
        b.iter(|| expander.expand(&finding_set, &thousand).expect("expands"));
    });
    let active_page = Options {
        active_only: true,
        ..page.clone()
    };
    group.bench_function("isa_finding_active_only_page_10", |b| {
        b.iter(|| {
            expander
                .expand(&finding_set, &active_page)
                .expect("expands")
        });
    });
    group.finish();
}

criterion_group!(benches, reads, ecl);
criterion_main!(benches);

/// The ECL evaluator over the NL edition: the descendants of the clinical
/// finding root, and a refinement with a cardinality, the two shapes #111
/// holds to ten milliseconds.
fn ecl(c: &mut Criterion) {
    let Some(dir) = artifact() else {
        eprintln!("no artifacts/nl: skipping the NL ECL benchmarks");
        return;
    };
    let provider = SnomedProvider::open(&dir, "en").expect("the NL artifact opens");
    let descendants = sct_ecl::parse("<< 404684003 |Clinical finding|").expect("parses");
    let refinement = sct_ecl::parse(
        "< 404684003 |Clinical finding| : [1..1] 363698007 |Finding site| = < 91723000 |Anatomical structure|",
    )
    .expect("parses");
    let group_refinement = sct_ecl::parse(
        "< 404684003 |Clinical finding| : { 363698007 |Finding site| = << 39057004 |Pulmonary valve structure|, 116676008 |Associated morphology| = << 415582006 |Stenosis| }",
    )
    .expect("parses");
    let mut group = c.benchmark_group("ecl");
    group.bench_function("descendants_clinical_finding", |b| {
        b.iter(|| sct_ecl::eval::evaluate(&provider, &descendants).expect("evaluates"));
    });
    group.bench_function("refinement_cardinality", |b| {
        b.iter(|| sct_ecl::eval::evaluate(&provider, &refinement).expect("evaluates"));
    });
    group.bench_function("refinement_group", |b| {
        b.iter(|| sct_ecl::eval::evaluate(&provider, &group_refinement).expect("evaluates"));
    });
    group.finish();
}
