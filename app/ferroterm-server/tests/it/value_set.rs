//! The `ValueSet` operations on the wire
//! (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>,
//! <https://hl7.org/fhir/R4B/valueset-operation-validate-code.html>).

use http::StatusCode;
use serde_json::json;

use crate::fixture::Server;
use ferroterm_testkit::fhir::{ANIMALS, VS_ALL, VS_PETS};

fn param<'a>(body: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    body["parameter"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

#[tokio::test]
async fn expand_by_get_and_post_returns_the_value_set_with_its_expansion() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$expand?url={VS_PETS}&includeDesignations=true"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["resourceType"], "ValueSet");
    assert_eq!(body["url"], VS_PETS);
    let contains = body["expansion"]["contains"].as_array().expect("contains");
    // The compose is one `is-a` include, so the expansion nests: `pet` is the root
    // and `kitten`, which the fixture subsumes under it, its child.
    let codes: Vec<&str> = contains.iter().filter_map(|c| c["code"].as_str()).collect();
    assert_eq!(codes, ["pet"]);
    assert_eq!(contains[0]["contains"][0]["code"], "kitten");
    assert_eq!(body["expansion"]["total"], 2);
    let used: Vec<&str> = body["expansion"]["parameter"]
        .as_array()
        .expect("parameters")
        .iter()
        .filter(|p| p["name"] == "used-codesystem")
        .filter_map(|p| p["valueUri"].as_str())
        .collect();
    assert_eq!(used, [format!("{ANIMALS}|2.0")]);
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": VS_ALL},
                {"name": "valueSetVersion", "valueString": "1.0"},
                {"name": "count", "valueInteger": 2},
                {"name": "offset", "valueInteger": 1}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["expansion"]["total"], 9);
    assert_eq!(body["expansion"]["offset"], 1);
    // The page carries two concepts; nested, they may be a parent and its child.
    assert_eq!(crate::ecosystem::counted(&body["expansion"]["contains"]), 2);
}

#[tokio::test]
async fn expand_of_an_unknown_value_set_is_not_found() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get("/r4b/ValueSet/$expand?url=http://example.org/fhir/ValueSet/nowhere")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["issue"][0]["code"], "not-found");
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "valueSet", "resource": {"resourceType": "ValueSet", "url": "http://example.org/x", "status": "active",
                    "compose": {"include": [{"valueSet": ["http://example.org/fhir/ValueSet/loop-a"]}]}}}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    // A cycle is a processing failure, "there is no point resubmitting the same
    // content unchanged" (<https://hl7.org/fhir/R4B/valueset-issue-type.html>).
    assert_eq!(body["issue"][0]["code"], "processing");
}

#[tokio::test]
async fn validate_code_carries_the_declared_outputs_and_the_r5_issues() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        true
    );
    assert_eq!(
        param(&body, "display").expect("display")["valueString"],
        "Kitten"
    );
    // NOTE: R4B declares none of these; the terminology ecosystem overlay pre-adopts
    // them from R6 (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>).
    assert_eq!(param(&body, "system").expect("system")["valueUri"], ANIMALS);
    assert_eq!(
        param(&body, "version").expect("version")["valueString"],
        "2.0"
    );
    assert_eq!(param(&body, "code").expect("code")["valueCode"], "kitten");
    assert!(
        param(&body, "issues").is_none(),
        "a clean pass has no issues"
    );
    let (status, body) = server
        .post(
            "/r5/ValueSet/$validate-code",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": VS_PETS},
                {"name": "coding", "valueCoding": {"system": ANIMALS, "code": "dog"}}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        false
    );
    let issues = &param(&body, "issues").expect("issues")["resource"];
    assert_eq!(issues["resourceType"], "OperationOutcome");
    let issue = &issues["issue"][0];
    assert_eq!(issue["severity"], "error");
    assert_eq!(issue["code"], "code-invalid");
    assert_eq!(
        issue["details"]["coding"][0]["system"],
        "http://hl7.org/fhir/tools/CodeSystem/tx-issue-type"
    );
    assert_eq!(issue["details"]["coding"][0]["code"], "not-in-vs");
    assert!(
        issue["details"]["text"]
            .as_str()
            .is_some_and(|t| t.contains("dog"))
    );
    assert_eq!(issue["expression"][0], "Coding.code");
}

#[tokio::test]
async fn validate_code_without_a_code_input_is_a_client_error() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!("/r4b/ValueSet/$validate-code?url={VS_PETS}"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["issue"][0]["code"], "invalid");
}

#[tokio::test]
async fn the_capability_statement_lists_the_value_set_operations() {
    let server = Server::start_with_resources();
    let (status, body) = server.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK);
    let resources = body["rest"][0]["resource"].as_array().expect("resources");
    let value_set = resources
        .iter()
        .find(|r| r["type"] == "ValueSet")
        .expect("ValueSet");
    let names: Vec<&str> = value_set["operation"]
        .as_array()
        .expect("operations")
        .iter()
        .filter_map(|o| o["name"].as_str())
        .collect();
    assert_eq!(names, ["expand", "validate-code"]);
}

/// An implicit SNOMED CT value set carries its template on the wire
/// (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value Sets"): the
/// definition-side fields travel with `includeDefinition`, and the copyright
/// is the page's own notice.
#[tokio::test]
async fn an_implicit_snomed_value_set_carries_the_page_s_template() {
    use ferroterm_testkit::snomed::{ANIMAL, VERSION, item, sctid};
    let server = Server::start();
    let animal = sctid(item(ANIMAL));
    let url = format!("{VERSION}?fhir_vs=isa/{animal}")
        .replace(':', "%3A")
        .replace('/', "%2F")
        .replace('?', "%3F")
        .replace('=', "%3D");
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$expand?url={url}&includeDefinition=true"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], VERSION, "{body}");
    assert_eq!(
        body["name"],
        format!("SNOMED CT Concept {animal} and descendants"),
        "{body}"
    );
    assert_eq!(body["description"], "All SNOMED CT concepts for Animal");
    assert_eq!(
        body["copyright"],
        fhir_terminology::snomed::TEMPLATE_COPYRIGHT
    );
    // Without the definition the expansion keeps the identity and leaves the
    // definition-side fields out, as it does for a stored value set.
    let (status, body) = server
        .get(&format!("/r4b/ValueSet/$expand?url={url}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], VERSION, "{body}");
    assert!(body.get("description").is_none(), "{body}");
    assert!(body.get("copyright").is_none(), "{body}");
}

/// `?fhir_vs=ecl/[ecl]` on the wire, the expression URI-encoded inside the
/// `url` parameter (<https://hl7.org/fhir/R4B/snomedct.html>, "Implicit Value
/// Sets"), on the system, edition, and version URIs.
#[tokio::test]
async fn expand_and_validate_code_answer_an_ecl_implicit_value_set() {
    use ferroterm_testkit::snomed::{ANIMAL, CAT, DOG, EDITION, VERSION, item, sctid};
    let server = Server::start();
    let animals = sctid(item(ANIMAL));
    let ecl = format!("%3C%20{animals}");
    for base in ["http://snomed.info/sct", EDITION, VERSION] {
        let url = format!("{base}?fhir_vs=ecl/{ecl}");
        let encoded = url
            .replace(':', "%3A")
            .replace('/', "%2F")
            .replace('?', "%3F")
            .replace('=', "%3D")
            .replace('%', "%25")
            .replace("%253A", "%3A")
            .replace("%252F", "%2F")
            .replace("%253F", "%3F")
            .replace("%253D", "%3D");
        let (status, body) = server
            .get(&format!("/r4b/ValueSet/$expand?url={encoded}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{base}: {body}");
        let mut codes: Vec<String> = body["expansion"]["contains"]
            .as_array()
            .expect("contains")
            .iter()
            .filter_map(|c| c["code"].as_str().map(str::to_owned))
            .collect();
        codes.sort();
        let mut expected = vec![sctid(item(CAT)), sctid(item(DOG))];
        expected.sort();
        assert_eq!(codes, expected, "{base}");
        assert_eq!(body["expansion"]["total"], 2);
    }
    let url = format!("http://snomed.info/sct?fhir_vs=ecl/{ecl}");
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$validate-code",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": url},
                {"name": "system", "valueUri": "http://snomed.info/sct"},
                {"name": "code", "valueCode": sctid(item(CAT))}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").and_then(|p| p["valueBoolean"].as_bool()),
        Some(true)
    );
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$validate-code",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": url},
                {"name": "system", "valueUri": "http://snomed.info/sct"},
                {"name": "code", "valueCode": animals}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").and_then(|p| p["valueBoolean"].as_bool()),
        Some(false),
        "the focus is not a descendant"
    );
    // Malformed ECL is an OperationOutcome, never a 500; an unknown identifier
    // in valid ECL is an invalid code.
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": "http://snomed.info/sct?fhir_vs=ecl/%3C%3C%20"}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "invalid");
    assert!(
        body["issue"][0]["details"]["text"]
            .as_str()
            .is_some_and(|t| t.contains("byte")),
        "{body}"
    );
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": "http://snomed.info/sct?fhir_vs=ecl/%3C%3C%20999999999"}
            ]}),
        )
        .await;
    assert_ne!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "code-invalid", "{body}");
}

#[tokio::test]
async fn an_expansion_carries_the_publisher_only_with_the_definition() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!("/r4b/ValueSet/$expand?url={VS_PETS}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("publisher").is_none(), "{body}");
    assert!(body.get("compose").is_none(), "{body}");
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$expand?url={VS_PETS}&includeDefinition=true"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["publisher"], "FerroTERM tests");
    assert!(body.get("compose").is_some(), "{body}");
    // An unusable displayLanguage is refused as a processing error.
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$expand?url={VS_PETS}&displayLanguage=zz"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["issue"][0]["code"], "processing");
    assert_eq!(
        body["issue"][0]["details"]["coding"][0]["code"],
        "invalid-display"
    );
    assert_eq!(
        body["issue"][0]["details"]["text"],
        "Invalid displayLanguage: 'zz'"
    );
}

/// An expression that nests deeper than the parser descends is refused, not
/// descended into.
///
/// The parser is recursive, so an unbounded nesting from a client would
/// exhaust the stack and abort the process; a panic unwinds into a 500, but a
/// stack overflow takes the server with it
/// (`.claude/rules/reliability.md`). Found by the `ecl_parse` fuzz target.
#[tokio::test]
async fn a_deeply_nested_expression_is_an_operation_outcome() {
    let server = Server::start();
    let deep = format!("{}138875005{}", "(".repeat(5_000), ")".repeat(5_000));
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": format!("http://snomed.info/sct?fhir_vs=ecl/{deep}")}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert_eq!(body["issue"][0]["code"], "invalid");
    assert!(
        body["issue"][0]["details"]["text"]
            .as_str()
            .is_some_and(|t| t.contains("nests")),
        "the outcome says the expression nests too deep: {body}"
    );
}

/// The membership-only validation the ecosystem requires, under the name the
/// specification declares.
///
/// The terminology ecosystem IG requires, at SHALL level, that a server
/// support "the mode/valueSetMode parameter" on `$validate-code`
/// (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>). No published
/// `OperationDefinition` declares either name, so neither is accepted here:
/// the behaviour is `valueset-membership-only`, which the R6 definition does
/// declare and which this server implements (#275).
#[tokio::test]
async fn validate_code_checks_membership_only_under_the_declared_name() {
    let server = Server::start_with_resources();
    let wrong = format!(
        "/r4b/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&display=Wrong"
    );
    let (status, body) = server
        .get(&format!("{wrong}&valueset-membership-only=true"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        true,
        "membership holds and no display is judged: {body}"
    );
    let (status, body) = server.get(&wrong).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        false,
        "without it the wrong display fails the code: {body}"
    );
    for undeclared in [
        "mode=CHECK_MEMBERSHIP_ONLY",
        "valueSetMode=NO_MEMBERSHIP_CHECK",
    ] {
        let (status, body) = server
            .get(&format!(
                "/r4b/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&{undeclared}"
            ))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{undeclared}: {body}");
        assert_eq!(body["issue"][0]["code"], "invalid", "{undeclared}: {body}");
    }
}
