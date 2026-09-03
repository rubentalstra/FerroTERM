//! `TerminologyCapabilities` from the registry, rendered per FHIR version.

use ferroterm_fhir::codec::Json;
use ferroterm_terminology::capabilities::Summary;
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
    let mut path = ferroterm_fhir::codec::Path::root("TerminologyCapabilities");
    let object = ferroterm_fhir::codec::expect_object(&r5, &path).expect("object");
    let decoded = ferroterm_fhir::r5::terminology_capabilities::TerminologyCapabilities::from_json(
        object, &mut path,
    )
    .expect("decodes");
    assert_eq!(decoded, summary.to_r5("2026-09-02T00:00:00Z"));
}
