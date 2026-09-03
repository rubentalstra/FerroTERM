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
    let codes: Vec<&str> = contains.iter().filter_map(|c| c["code"].as_str()).collect();
    assert_eq!(codes, ["kitten", "pet"]);
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
    assert_eq!(
        body["expansion"]["contains"].as_array().map(Vec::len),
        Some(2)
    );
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
    assert_eq!(body["issue"][0]["code"], "invalid");
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
    for undeclared in ["system", "version", "code", "issues"] {
        assert!(
            param(&body, undeclared).is_none(),
            "R4B declares no `{undeclared}` output: {body}"
        );
    }
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
    assert_eq!(issue["expression"][0], "coding");
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
