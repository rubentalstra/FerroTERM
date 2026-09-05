//! `TerminologyCapabilities` from the registry, rendered per FHIR version.

use std::sync::Arc;

use fhir_terminology::capabilities::Summary;
use fhir_terminology::fhir_codesystem::load::{FhirVersion, load_file};
use fhir_terminology::fhir_codesystem::provider::FhirCodeSystem;
use fhir_terminology::provider::{CodeSystemProvider, Compositional};
use fhir_terminology::registries::ucum::provider::{URL as UCUM, UcumProvider};
use fhir_terminology::registry::Registry;
use fhir_types::codec::Json;
use serde_json::Value;

use crate::fixture::{FLAT_URL, URL, registry};

#[test]
fn the_summary_reads_every_declaration_and_marks_the_default() {
    let mut registry = registry();
    registry.set_default(URL, "2024").expect("sets default");
    let summary = Summary::of(&registry);
    assert_eq!(summary.systems.len(), 2);
    let fixture = summary
        .systems
        .iter()
        .find(|s| s.url == URL)
        .expect("fixture");
    assert!(fixture.subsumption);
    let defaults: Vec<(&str, bool)> = fixture
        .versions
        .iter()
        .map(|v| (v.code.as_str(), v.is_default))
        .collect();
    assert_eq!(defaults, [("2024", true), ("2025", false)]);
    assert_eq!(fixture.versions[0].properties, ["legs", "kingdom"]);
    assert_eq!(fixture.versions[0].filters[0].operators.len(), 11);
    let flat = summary
        .systems
        .iter()
        .find(|s| s.url == FLAT_URL)
        .expect("flat");
    assert!(!flat.subsumption);
    assert_eq!(flat.versions[0].filters[0].operators.len(), 5);
}

#[test]
fn r4_r4b_and_r5_render_their_own_shapes() {
    let summary = Summary::of(&registry());
    let r4 = Value::Object(
        summary
            .to_r4("2026-09-02T00:00:00Z")
            .to_json()
            .expect("encodes"),
    );
    let r4b = Value::Object(
        summary
            .to_r4b("2026-09-02T00:00:00Z")
            .to_json()
            .expect("encodes"),
    );
    let r5 = Value::Object(
        summary
            .to_r5("2026-09-02T00:00:00Z")
            .to_json()
            .expect("encodes"),
    );
    for (version, json) in [("r4", &r4), ("r4b", &r4b), ("r5", &r5)] {
        assert_eq!(json["resourceType"], "TerminologyCapabilities", "{version}");
        assert_eq!(json["status"], "active", "{version}");
        assert_eq!(json["kind"], "instance", "{version}");
        assert_eq!(json["codeSystem"][0]["uri"], URL, "{version}");
        assert_eq!(json["codeSystem"][0]["subsumption"], true, "{version}");
        assert_eq!(
            json["codeSystem"][0]["version"][1]["isDefault"], true,
            "{version}"
        );
        assert_eq!(
            json["codeSystem"][0]["version"][0]["filter"][0]["code"], "concept",
            "{version}"
        );
        assert_eq!(
            json["codeSystem"][0]["version"][0]["language"],
            serde_json::json!(["en", "nl"]),
            "{version}"
        );
        assert_eq!(json["expansion"]["paging"], true, "{version}");
        assert!(json["expansion"]["textFilter"].is_string(), "{version}");
    }
    // R5 adds the mandatory codeSystem.content; R4 and R4B have no such element.
    assert_eq!(r5["codeSystem"][1]["content"], "not-present");
    assert_eq!(r5["codeSystem"][1]["uri"], FLAT_URL);
    assert!(r4b["codeSystem"][1].get("content").is_none());
    assert!(r4["codeSystem"][1].get("content").is_none());
    assert_eq!(r4, r4b, "R4 and R4B fill the same elements");
    // The rendered resources decode again through the generated codec.
    let mut path = fhir_types::codec::Path::root("TerminologyCapabilities");
    let object = fhir_types::codec::expect_object(&r5, &path).expect("object");
    let decoded = fhir_types::r5::terminology_capabilities::TerminologyCapabilities::from_json(
        object, &mut path,
    )
    .expect("decodes");
    assert_eq!(decoded, summary.to_r5("2026-09-02T00:00:00Z"));
}

/// A `CodeSystem` resource that declares a compositional grammar, written to a
/// file so the generic provider loads it the way a deployment's package does.
const GRAMMAR_SYSTEM: &str = "http://example.org/fhir/CodeSystem/grammar";

fn declares_a_grammar() -> (tempfile::TempDir, FhirCodeSystem) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("CodeSystem-grammar.json");
    let resource = serde_json::json!({
        "resourceType": "CodeSystem",
        "url": GRAMMAR_SYSTEM,
        "version": "1.0",
        "status": "active",
        "content": "complete",
        "caseSensitive": true,
        "compositional": true,
        "concept": [{"code": "a", "display": "A"}]
    });
    std::fs::write(&path, resource.to_string()).expect("writes");
    let model = load_file(&path, FhirVersion::R5).expect("loads");
    assert!(model.compositional, "the resource declares the grammar");
    let provider = FhirCodeSystem::new(model).expect("builds");
    (dir, provider)
}

#[test]
fn a_declared_grammar_and_a_supported_grammar_are_two_different_statements() {
    // `CodeSystem.compositional` is "The code system defines a compositional
    // (post-coordination) grammar"
    // (<https://hl7.org/fhir/R4B/codesystem-definitions.html#CodeSystem.compositional>);
    // `TerminologyCapabilities.codeSystem.version.compositional` is "If the
    // compositional grammar defined by the code system is supported"
    // (<https://hl7.org/fhir/R4B/terminologycapabilities-definitions.html#TerminologyCapabilities.codeSystem.version.compositional>).
    // The generic provider serves the concepts a resource enumerates and
    // evaluates no grammar, so the two disagree for it.
    let (_dir, grammar) = declares_a_grammar();
    assert_eq!(grammar.declaration().compositional, Compositional::Defined);
    assert!(grammar.declaration().compositional.defined());
    assert!(!grammar.declaration().compositional.supported());
    // UCUM's own provider parses the unit expression grammar, so both hold.
    let ucum = UcumProvider::new();
    assert_eq!(ucum.declaration().compositional, Compositional::Supported);
    assert!(ucum.declaration().compositional.defined());
    assert!(ucum.declaration().compositional.supported());

    let mut registry = Registry::new();
    registry.register(Arc::new(grammar)).expect("registers");
    registry.register(Arc::new(ucum)).expect("registers");
    let summary = Summary::of(&registry);
    let flags: Vec<(&str, bool)> = summary
        .systems
        .iter()
        .map(|system| (system.url.as_str(), system.versions[0].compositional))
        .collect();
    assert_eq!(flags, [(GRAMMAR_SYSTEM, false), (UCUM, true)]);
    let json = Value::Object(
        summary
            .to_r4b("2026-09-05T00:00:00Z")
            .to_json()
            .expect("encodes"),
    );
    assert_eq!(json["codeSystem"][0]["uri"], GRAMMAR_SYSTEM);
    assert_eq!(json["codeSystem"][0]["version"][0]["compositional"], false);
    assert_eq!(json["codeSystem"][1]["uri"], UCUM);
    assert_eq!(json["codeSystem"][1]["version"][0]["compositional"], true);
}
