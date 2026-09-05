//! The same four operations as `fhir-terminology`'s `operations` bench, this
//! time through the router: routing, content negotiation, the codec, and the
//! response body included.
//!
//! The engine's bar is one millisecond per point read
//! (`docs/architecture.md`); this bench says what the server adds on top of
//! it. `scripts/checks/bench-bars.sh` reads the medians and fails when one
//! crosses its bar. The edition is generated content in an invented namespace
//! and holds no SNOMED CT content.
#![allow(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "a benchmark harness fails loud and reports to stderr, like a test binary"
)]

use std::sync::Arc;

use axum::body::Body;
use criterion::{Criterion, criterion_group, criterion_main};
use ferroterm_server::config::Config;
use ferroterm_server::state::AppState;
use http::Request;
use tower::ServiceExt;

/// The edition the bench builds, the size the engine bench uses.
const CONCEPTS: u32 = 20_000;
/// A concept deep in the tree, so a read is never the first ordinal.
const DEEP: u32 = 17_777;

fn requests(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    ferroterm_testkit::scaled::write(dir.path(), CONCEPTS).expect("writes the edition");
    let config = Config {
        index: vec![dir.path().to_path_buf()],
        ..Config::default()
    };
    let state = Arc::new(AppState::load(&config).expect("loads"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let root = ferroterm_testkit::scaled::code(ferroterm_testkit::scaled::ROOT);
    let deep = ferroterm_testkit::scaled::code(DEEP);
    let system = "http://snomed.info/sct";

    // The router is built once, as the binary builds it, and cloned per
    // request because `oneshot` consumes the service.
    let built = ferroterm_server::router(Arc::clone(&state));
    let answer = |uri: &str| {
        let router = built.clone();
        let request = Request::get(uri).body(Body::empty()).expect("request");
        runtime.block_on(async {
            let response = router.oneshot(request).await.expect("response");
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body")
        })
    };

    let lookup = format!("/r4b/CodeSystem/$lookup?system={system}&code={deep}");
    let validate = format!("/r4b/CodeSystem/$validate-code?url={system}&code={deep}");
    let subsumes = format!("/r4b/CodeSystem/$subsumes?system={system}&codeA={root}&codeB={deep}");
    let expand = format!(
        "/r4b/ValueSet/$expand?url=http%3A%2F%2Fsnomed.info%2Fsct%3Ffhir_vs%3Disa%2F{root}&count=10&offset=1000"
    );

    let mut group = c.benchmark_group("http");
    group.bench_function("lookup", |b| b.iter(|| answer(&lookup)));
    group.bench_function("validate_code", |b| b.iter(|| answer(&validate)));
    group.bench_function("subsumes", |b| b.iter(|| answer(&subsumes)));
    group.bench_function("expand_page_10", |b| b.iter(|| answer(&expand)));
    group.finish();
}

/// The same router over the local `RxNorm` and SNOMED artifacts, when they are
/// built, so the served figure a record reports has a bench behind it: a
/// record measures a socket too, this measures everything above it (#304).
fn local(c: &mut Criterion) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts");
    let built: Vec<std::path::PathBuf> = ["rxnorm", "nl"]
        .iter()
        .map(|name| root.join(name))
        .filter(|dir| dir.join("manifest.json").exists())
        .collect();
    if built.is_empty() {
        eprintln!("no local artifacts: skipping the served read benchmarks");
        return;
    }
    let config = Config {
        index: built,
        ..Config::default()
    };
    // An artifact of an older layout is as absent as a missing one, as far as
    // this bench is concerned: it skips rather than failing the bars run.
    let state = match AppState::load(&config) {
        Ok(state) => Arc::new(state),
        Err(error) => {
            eprintln!("the local artifacts do not open ({error}): skipping the served benchmarks");
            return;
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let router = ferroterm_server::router(Arc::clone(&state));
    let answer = |uri: &str| {
        let router = router.clone();
        let request = Request::get(uri).body(Body::empty()).expect("request");
        runtime.block_on(async {
            let response = router.oneshot(request).await.expect("response");
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body")
        })
    };
    let mut group = c.benchmark_group("served");
    let rxnorm = "http%3A%2F%2Fwww.nlm.nih.gov%2Fresearch%2Fumls%2Frxnorm";
    group.bench_function("rxnorm_lookup", |b| {
        b.iter(|| {
            answer(&format!(
                "/r4b/CodeSystem/$lookup?system={rxnorm}&code=313782"
            ))
        });
    });
    let sct = "http%3A%2F%2Fsnomed.info%2Fsct";
    group.bench_function("snomed_lookup", |b| {
        b.iter(|| {
            answer(&format!(
                "/r4b/CodeSystem/$lookup?system={sct}&code=404684003"
            ))
        });
    });
    // What writing the same body costs once the object exists, so the served
    // figure splits into building the object and serializing it (#304).
    let body = answer(&format!(
        "/r4b/CodeSystem/$lookup?system={rxnorm}&code=313782"
    ));
    let object: fhir_types::codec::Object = serde_json::from_slice(&body).expect("the body parses");
    group.bench_function("rxnorm_serialize", |b| {
        b.iter(|| serde_json::to_vec(&object).expect("serializes"));
    });
    group.finish();
}

criterion_group!(benches, requests, local);
criterion_main!(benches);
