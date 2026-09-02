//! `ConceptMap/$translate` on the wire
//! (<https://hl7.org/fhir/R4B/conceptmap-operation-translate.html>).

use http::StatusCode;
use serde_json::{Value, json};

use crate::fixture::Server;
use ferroterm_testkit::fhir::{ANIMALS, CM_ANIMALS_COLOURS, COLOURS};

fn param<'a>(body: &'a Value, name: &str) -> Option<&'a Value> {
    body["parameter"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

fn part<'a>(m: &'a Value, name: &str) -> Option<&'a Value> {
    m["part"].as_array()?.iter().find(|p| p["name"] == name)
}

#[tokio::test]
async fn translate_by_get_and_post_answers_matches_with_their_origin() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r4b/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&system={ANIMALS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        true
    );
    let m = param(&body, "match").expect("match");
    assert_eq!(
        part(m, "equivalence").expect("equivalence")["valueCode"],
        "equivalent"
    );
    assert_eq!(
        part(m, "concept").expect("concept")["valueCoding"]["code"],
        "RED"
    );
    assert_eq!(
        part(m, "concept").expect("concept")["valueCoding"]["system"],
        COLOURS
    );
    assert_eq!(
        part(m, "source").expect("source")["valueUri"],
        format!("{CM_ANIMALS_COLOURS}|1.0")
    );
    assert_eq!(
        part(m, "originMap").expect("originMap")["valueCanonical"],
        format!("{CM_ANIMALS_COLOURS}|1.0")
    );
    assert_eq!(
        part(m, "sourceConcept").expect("sourceConcept")["valueCoding"]["code"],
        "cat"
    );
    let (status, body) = server
        .post(
            "/r4b/ConceptMap/$translate",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "coding", "valueCoding": {"system": ANIMALS, "code": "fish"}},
                {"name": "targetsystem", "valueUri": COLOURS}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        false
    );
    let m = param(&body, "match").expect("match");
    assert_eq!(part(m, "noMap").expect("noMap")["valueBoolean"], true);
    assert_eq!(
        part(m, "sourceComment").expect("comment")["valueString"],
        "fish have no colour"
    );
    assert!(param(&body, "message").is_some());
}

#[tokio::test]
async fn translate_refuses_what_it_cannot_answer() {
    let server = Server::start_with_resources();
    let (status, body) = server
        .get(&format!(
            "/r4b/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&code=cat"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["issue"][0]["code"], "required");
    let (status, body) = server
        .get(&format!("/r4b/ConceptMap/$translate?url=http://example.org/fhir/ConceptMap/nowhere&system={ANIMALS}&code=cat"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = server.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK);
    let concept_map = body["rest"][0]["resource"]
        .as_array()
        .expect("resources")
        .iter()
        .find(|r| r["type"] == "ConceptMap")
        .expect("ConceptMap");
    assert_eq!(concept_map["operation"][0]["name"], "translate");
}
