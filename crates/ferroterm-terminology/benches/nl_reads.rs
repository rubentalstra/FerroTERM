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

use criterion::{Criterion, criterion_group, criterion_main};
use ferroterm_fhir::r4b::operations::code_system_lookup::CodeSystemLookupRequest;
use ferroterm_fhir::r4b::operations::code_system_subsumes::CodeSystemSubsumesRequest;
use ferroterm_fhir::r4b::operations::code_system_validate_code::CodeSystemValidateCodeRequest;
use ferroterm_terminology::operations::{Invocation, lookup, subsumes, validate_code};
use ferroterm_terminology::provider::CodeSystemProvider;
use ferroterm_terminology::registry::Registry;
use ferroterm_terminology::snomed::SnomedProvider;

const SCT: &str = "http://snomed.info/sct";
/// `138875005 |SNOMED CT Concept|`, the root: the one SCTID every edition has.
const ROOT: &str = "138875005";
/// `404684003 |Clinical finding|`, a top-level hierarchy present in every edition.
const FINDING: &str = "404684003";

fn artifact() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/nl");
    dir.join("manifest.json").exists().then_some(dir)
}

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
        b.iter(|| provider.locate(FINDING).expect("reads"))
    });
    group.bench_function("display_nl", |b| {
        b.iter(|| provider.display(finding, Some("nl")).expect("reads"))
    });
    group.bench_function("designations", |b| {
        b.iter(|| provider.designations(finding, None).expect("reads"))
    });
    group.bench_function("properties", |b| {
        b.iter(|| provider.properties(finding).expect("reads"))
    });
    group.bench_function("subsumes", |b| b.iter(|| hierarchy.subsumes(root, finding)));
    group.bench_function("search_hart_nl", |b| {
        b.iter(|| provider.search("hart", Some("nl")).expect("reads"))
    });
    group.finish();

    let mut registry = Registry::new();
    registry.register(Arc::new(provider)).expect("registers");
    let mut group = c.benchmark_group("operations");
    let lookup_request = CodeSystemLookupRequest {
        system: Some(SCT.into()),
        code: Some(FINDING.into()),
        display_language: Some("nl".into()),
        ..Default::default()
    };
    group.bench_function("lookup", |b| {
        b.iter(|| lookup::lookup(&registry, &Invocation::Type, &lookup_request).expect("looks up"))
    });
    let validate_request = CodeSystemValidateCodeRequest {
        url: Some(SCT.into()),
        code: Some(FINDING.into()),
        display: Some("Clinical finding".into()),
        ..Default::default()
    };
    group.bench_function("validate_code", |b| {
        b.iter(|| {
            validate_code::validate_code(&registry, &Invocation::Type, &validate_request)
                .expect("validates")
        })
    });
    let subsumes_request = CodeSystemSubsumesRequest {
        system: Some(SCT.into()),
        code_a: Some(ROOT.into()),
        code_b: Some(FINDING.into()),
        ..Default::default()
    };
    group.bench_function("subsumes", |b| {
        b.iter(|| {
            subsumes::subsumes(&registry, &Invocation::Type, &subsumes_request).expect("subsumes")
        })
    });
    group.finish();
}

criterion_group!(benches, reads);
criterion_main!(benches);
