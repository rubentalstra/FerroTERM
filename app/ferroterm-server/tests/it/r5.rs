//! FHIR R5 under `/r5`: the 5.0.0 resources and the operation shapes R5 adds
//! (<https://hl7.org/fhir/R5/terminology-service.html>), and the R4-family
//! endpoints staying within their declared outputs.

use ferroterm_testkit::fhir::{ANIMALS, CM_ANIMALS_COLOURS, COLOURS, VS_PETS};
use ferroterm_testkit::snomed::{CAT, VERSION, item, sctid};
use fhir_types::codec::{Json, Path, expect_object};
use http::StatusCode;
use serde_json::{Value, json};

use crate::fixture::{Server, parameter};

const SCT: &str = "http://snomed.info/sct";

fn part<'a>(m: &'a Value, name: &str) -> Option<&'a Value> {
    m["part"].as_array()?.iter().find(|p| p["name"] == name)
}

#[tokio::test]
async fn the_metadata_resources_are_r5() {
    let server = Server::start();
    let (status, body) = server.get("/r5/metadata").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["fhirVersion"], "5.0.0");
    assert_eq!(
        body["url"],
        "https://ferroterm.eu/fhir/CapabilityStatement/ferroterm-r5"
    );
    let mut path = Path::root("CapabilityStatement");
    let object = expect_object(&body, &path).expect("object");
    let decoded =
        fhir_types::r5::capability_statement::CapabilityStatement::from_json(object, &mut path)
            .expect("an R5 CapabilityStatement");
    assert_eq!(Value::Object(decoded.to_json().expect("encodes")), body);
    let (status, body) = server.get("/r5/metadata?mode=terminology").await;
    assert_eq!(status, StatusCode::OK);
    // R5 makes codeSystem.content mandatory; the R4 family has no such element.
    assert_eq!(body["codeSystem"][0]["content"], "not-present");
    let mut path = Path::root("TerminologyCapabilities");
    let object = expect_object(&body, &path).expect("object");
    fhir_types::r5::terminology_capabilities::TerminologyCapabilities::from_json(object, &mut path)
        .expect("an R5 TerminologyCapabilities");
    let (status, body) = server.get("/r5/$versions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "version").unwrap()["valueCode"], "5.0");
}

#[tokio::test]
async fn lookup_answers_definition_under_r5_only() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r5/CodeSystem/$lookup?system={ANIMALS}&code=animal"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        parameter(&body, "definition").unwrap()["valueString"],
        "A living thing that is not a plant."
    );
    let property_codes = |body: &serde_json::Value| -> Vec<String> {
        body["parameter"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["name"] == "property")
            .map(|p| p["part"][0]["valueCode"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(
        !property_codes(&body).contains(&String::from("definition")),
        "R5 answers definition once, as the named output: {body}"
    );
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={ANIMALS}&code=animal"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        parameter(&body, "definition").is_none(),
        "R4B declares definition as a property, not an output: {body}"
    );
    assert!(
        property_codes(&body).contains(&String::from("definition")),
        "R4B answers definition in the property group: {body}"
    );
    let (status, body) = server
        .get(&format!(
            "/r5/CodeSystem/$lookup?system={SCT}&code={}&useSupplement=http://example.org/none",
            sctid(item(CAT))
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "useSupplement is declared: {body}");
    assert_eq!(parameter(&body, "version").unwrap()["valueString"], VERSION);
}

#[tokio::test]
async fn validate_code_itemises_issues_under_r5_and_under_the_overlay_on_r4b() {
    let server = Server::start_with_resources();
    let request = json!({"resourceType": "Parameters", "parameter": [
        {"name": "url", "valueUri": ANIMALS},
        {"name": "coding", "valueCoding": {"system": ANIMALS, "code": "cat", "display": "Dog"}}
    ]});
    let (status, body) = server.post("/r5/CodeSystem/$validate-code", &request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], false);
    assert_eq!(parameter(&body, "code").unwrap()["valueCode"], "cat");
    assert_eq!(parameter(&body, "system").unwrap()["valueUri"], ANIMALS);
    assert_eq!(parameter(&body, "version").unwrap()["valueString"], "2.0");
    let issues = &parameter(&body, "issues").expect("issues")["resource"];
    assert_eq!(issues["resourceType"], "OperationOutcome");
    assert_eq!(
        issues["issue"][0]["details"]["coding"][0]["code"],
        "invalid-display"
    );
    assert_eq!(issues["issue"][0]["expression"][0], "Coding.display");
    let (status, body) = server
        .post("/r4b/CodeSystem/$validate-code", &request)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], false);
    // NOTE: R4B declares none of these; the terminology ecosystem overlay pre-adopts
    // them from R6 (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>).
    for adopted in ["code", "system", "version", "issues"] {
        assert!(
            parameter(&body, adopted).is_some(),
            "R4B answers the pre-adopted `{adopted}` output: {body}"
        );
    }
    assert_eq!(
        parameter(&body, "issues").unwrap()["resource"]["issue"][0]["details"]["coding"][0]["code"],
        "invalid-display"
    );
    let (status, body) = server
        .get(&format!(
            "/r5/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], false);
    let issues = &parameter(&body, "issues").expect("issues")["resource"];
    assert_eq!(
        issues["issue"][0]["details"]["coding"][0]["code"],
        "not-in-vs"
    );
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let issues = &parameter(&body, "issues").expect("the overlay's issues on R4B")["resource"];
    assert_eq!(
        issues["issue"][0]["details"]["coding"][0]["code"], "not-in-vs",
        "{body}"
    );
}

#[tokio::test]
async fn expand_returns_properties_under_r5() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!("/r5/ValueSet/$expand?url={VS_PETS}&property=legs"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut path = Path::root("ValueSet");
    let object = expect_object(&body, &path).expect("object");
    fhir_types::r5::value_set::ValueSet::from_json(object, &mut path).expect("an R5 ValueSet");
    assert_eq!(body["expansion"]["property"][0]["code"], "legs");
    assert_eq!(
        body["expansion"]["property"][0]["uri"],
        "http://example.org/legs"
    );
    let kitten = body["expansion"]["contains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["code"] == "kitten")
        .expect("kitten");
    assert_eq!(kitten["property"][0]["code"], "legs");
    assert_eq!(kitten["property"][0]["valueInteger"], 4);
    let (status, body) = server
        .get(&format!("/r5/ValueSet/$expand?url={VS_PETS}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["expansion"].get("property").is_none(), "{body}");
    // R4B declares no `property` on $expand.
    let (status, body) = server
        .get(&format!(
            "/r4b/ValueSet/$expand?url={VS_PETS}&property=legs"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn translate_speaks_the_r5_parameter_names() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r5/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&system={ANIMALS}&sourceCode=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    let m = parameter(&body, "match").expect("match");
    assert_eq!(
        part(m, "relationship").expect("relationship")["valueCode"],
        "equivalent"
    );
    assert!(
        part(m, "equivalence").is_none(),
        "R5 has no equivalence part"
    );
    assert_eq!(
        part(m, "concept").expect("concept")["valueCoding"]["code"],
        "RED"
    );
    assert_eq!(
        part(m, "originMap").expect("originMap")["valueUri"],
        format!("{CM_ANIMALS_COLOURS}|1.0")
    );
    // `code` is the R4 name; R5 declares `sourceCode`.
    let (status, body) = server
        .get(&format!(
            "/r5/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&system={ANIMALS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // A targetCode is translated in reverse.
    let (status, body) = server
        .get(&format!(
            "/r5/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&system={COLOURS}&targetCode=RED"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    let m = parameter(&body, "match").expect("match");
    assert_eq!(
        part(m, "concept").expect("concept")["valueCoding"]["code"],
        "cat"
    );
    // R4B emits only its declared match parts.
    let (status, body) = server
        .get(&format!(
            "/r4b/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&system={ANIMALS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let m = parameter(&body, "match").expect("match");
    assert!(part(m, "originMap").is_none(), "{body}");
    assert_eq!(
        part(m, "equivalence").expect("equivalence")["valueCode"],
        "equivalent"
    );
}
