//! The one integration-test binary for `ferroterm-fhir-codegen`; one module per topic.
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

use ferroterm_fhir_codegen::package::Package;

mod closure;
mod codec;
mod emit;
mod operations;
mod package;
mod roots;
mod snapshot;

/// The vendored R4B core package, loaded once for every test.
static R4B: LazyLock<Package> = LazyLock::new(|| {
    Package::open(r4b_dir()).expect("the vendored hl7.fhir.r4b.core package should load")
});

fn vendor_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor")
}

fn r4b_dir() -> PathBuf {
    vendor_dir().join("hl7.fhir.r4b.core")
}

/// The vendored R4 core package, loaded once for every test.
static R4: LazyLock<Package> = LazyLock::new(|| {
    Package::open(vendor_dir().join("hl7.fhir.r4.core"))
        .expect("the vendored hl7.fhir.r4.core package should load")
});

/// The vendored R6 ballot package, loaded once for every test.
static R6: LazyLock<Package> = LazyLock::new(|| {
    Package::open(vendor_dir().join("hl7.fhir.r6.core"))
        .expect("the vendored hl7.fhir.r6.core package should load")
});

/// The vendored R5 core package, loaded once for every test.
static R5: LazyLock<Package> = LazyLock::new(|| {
    Package::open(vendor_dir().join("hl7.fhir.r5.core"))
        .expect("the vendored hl7.fhir.r5.core package should load")
});
