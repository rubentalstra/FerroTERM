//! The one integration-test binary for `notio-fhir-codegen`; one module per topic.
//!
//! The tests read the vendored `hl7.fhir.r4b.core` 4.3.0 package, so every
//! expectation below is a fact of that package.
#![allow(
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test assertions"
)]

use std::path::PathBuf;
use std::sync::LazyLock;

use notio_fhir_codegen::package::Package;

mod package;
mod roots;
mod snapshot;

/// The vendored R4B core package, loaded once for every test.
static R4B: LazyLock<Package> = LazyLock::new(|| {
    Package::open(r4b_dir()).expect("the vendored hl7.fhir.r4b.core package should load")
});

fn r4b_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/hl7.fhir.r4b.core")
}
