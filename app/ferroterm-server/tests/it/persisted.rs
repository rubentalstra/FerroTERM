//! The persisted `CodeSystem`, `ValueSet`, and `ConceptMap` resources on the
//! wire (<https://hl7.org/fhir/R4B/http.html>).

use axum::body::Body;
use http::header::{ETAG, IF_MATCH, LAST_MODIFIED, LOCATION};
use http::{Request, StatusCode};
use serde_json::{Value, json};

use crate::fixture::{self, Server, header};

const COLOURS: &str = "http://ferroterm.test/CodeSystem/colours";
const COLOUR_SET: &str = "http://ferroterm.test/ValueSet/colours";

fn colours(version: &str) -> Value {
    json!({
        "resourceType": "CodeSystem",
        "url": COLOURS,
        "version": version,
        "status": "active",
        "content": "complete",
        "concept": [
            {"code": "red", "display": "Red"},
            {"code": "blue", "display": "Blue"}
        ]
    })
}

fn colour_set() -> Value {
    json!({
        "resourceType": "ValueSet",
        "url": COLOUR_SET,
        "version": "1.0",
        "status": "active",
        "compose": {"include": [{"system": COLOURS}]}
    })
}

#[tokio::test]
async fn a_put_creates_then_updates_with_the_fhir_status_codes_and_headers() {
    let server = Server::start_persisting();

    let response = server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(header(&response, ETAG).as_deref(), Some("W/\"1\""));
    assert_eq!(
        header(&response, LOCATION).as_deref(),
        Some("/r4b/CodeSystem/colours/_history/1")
    );
    assert!(
        header(&response, LAST_MODIFIED).is_some_and(|value| value.ends_with("GMT")),
        "the created resource carries an HTTP-date Last-Modified"
    );
    let (_, body) = fixture::json(response).await;
    assert_eq!(body["id"], "colours");
    assert_eq!(body["meta"]["versionId"], "1");
    assert!(
        body["meta"]["lastUpdated"]
            .as_str()
            .is_some_and(|value| value.contains('T')),
        "the stored resource carries meta.lastUpdated: {body}"
    );

    let response = server.put("/r4b/CodeSystem/colours", &colours("2.0")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a second put updates the resource"
    );
    assert_eq!(header(&response, ETAG).as_deref(), Some("W/\"2\""));
    assert!(
        header(&response, LOCATION).is_none(),
        "an update names no new location"
    );
}

#[tokio::test]
async fn a_persisted_code_system_answers_every_operation_and_survives_a_restart() {
    let server = Server::start_persisting();
    assert_eq!(
        server
            .put("/r4b/CodeSystem/colours", &colours("1.0"))
            .await
            .status(),
        StatusCode::CREATED
    );

    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={COLOURS}&code=red"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let display = body["parameter"]
        .as_array()
        .expect("parameters")
        .iter()
        .find(|parameter| parameter["name"] == "display")
        .and_then(|parameter| parameter["valueString"].as_str());
    assert_eq!(display, Some("Red"), "the persisted system answers $lookup");

    let restarted = server.restarted();
    let (status, body) = restarted
        .get(&format!(
            "/r5/CodeSystem/$lookup?system={COLOURS}&code=blue"
        ))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the resource written on R4B answers on R5 after a restart: {body}"
    );
    let (status, body) = restarted.get("/r4b/CodeSystem/colours").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], "1.0");
}

#[tokio::test]
async fn a_persisted_value_set_expands_over_its_persisted_code_system() {
    let server = Server::start_persisting();
    server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    server.put("/r4b/ValueSet/colour-set", &colour_set()).await;

    let (status, body) = server
        .get(&format!("/r4b/ValueSet/$expand?url={COLOUR_SET}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let codes: Vec<&str> = body["expansion"]["contains"]
        .as_array()
        .expect("contains")
        .iter()
        .filter_map(|entry| entry["code"].as_str())
        .collect();
    assert_eq!(codes, ["blue", "red"]);
}

#[tokio::test]
async fn a_persisted_concept_map_answers_translate() {
    let server = Server::start_persisting();
    server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    let map = json!({
        "resourceType": "ConceptMap",
        "url": "http://ferroterm.test/ConceptMap/colours-hues",
        "version": "1.0",
        "status": "active",
        "group": [{
            "source": COLOURS,
            "target": "http://ferroterm.test/CodeSystem/hues",
            "element": [{
                "code": "red",
                "target": [{"code": "crimson", "equivalence": "equivalent"}]
            }]
        }]
    });
    let response = server.put("/r4b/ConceptMap/colours-hues", &map).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let (status, body) = server
        .get(&format!(
            "/r4b/ConceptMap/$translate?url=http://ferroterm.test/ConceptMap/colours-hues&system={COLOURS}&code=red"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let matched = body["parameter"]
        .as_array()
        .expect("parameters")
        .iter()
        .find(|parameter| parameter["name"] == "match")
        .expect("a match");
    let concept = matched["part"]
        .as_array()
        .expect("parts")
        .iter()
        .find(|part| part["name"] == "concept")
        .expect("the concept");
    assert_eq!(concept["valueCoding"]["code"], "crimson");
}

#[tokio::test]
async fn a_version_read_answers_an_earlier_version_and_a_delete_leaves_the_history() {
    let server = Server::start_persisting();
    server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    server.put("/r4b/CodeSystem/colours", &colours("2.0")).await;

    let (status, body) = server.get("/r4b/CodeSystem/colours/_history/1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], "1.0");
    let (status, body) = server.get("/r4b/CodeSystem/colours/_history/9").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let request = Request::delete("/r4b/CodeSystem/colours")
        .body(Body::empty())
        .expect("request");
    let response = server.send(request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let request = Request::delete("/r4b/CodeSystem/colours")
        .body(Body::empty())
        .expect("request");
    assert_eq!(
        server.send(request).await.status(),
        StatusCode::NO_CONTENT,
        "deleting again has no effect and is not an error"
    );

    let (status, body) = server.get("/r4b/CodeSystem/colours").await;
    assert_eq!(
        status,
        StatusCode::GONE,
        "a deleted resource reads as gone: {body}"
    );
    let (status, body) = server.get("/r4b/CodeSystem/colours/_history/2").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the history outlives the delete: {body}"
    );
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={COLOURS}&code=red"
        ))
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the deleted system stops answering: {body}"
    );
}

#[tokio::test]
async fn a_search_returns_the_persisted_and_the_loaded_value_sets() {
    let server = Server::start_persisting();
    server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    server.put("/r4b/ValueSet/colour-set", &colour_set()).await;

    let (status, body) = server.get("/r4b/ValueSet").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let urls: Vec<&str> = body["entry"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry["resource"]["url"].as_str())
        .collect();
    assert!(
        urls.contains(&COLOUR_SET),
        "the persisted value set is in the searchset: {urls:?}"
    );
    assert!(
        urls.len() > 1,
        "the loaded value sets are there too: {urls:?}"
    );

    let (status, body) = server.get(&format!("/r4b/ValueSet?url={COLOUR_SET}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1, "a url narrows the searchset");

    let (status, body) = server.get("/r4b/CodeSystem?url=nothing.example").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 0);
}

#[tokio::test]
async fn an_if_match_that_names_another_version_is_refused() {
    let server = Server::start_persisting();
    server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;

    let request = Request::put("/r4b/CodeSystem/colours")
        .header(http::header::CONTENT_TYPE, "application/fhir+json")
        .header(IF_MATCH, "W/\"7\"")
        .body(Body::from(colours("2.0").to_string()))
        .expect("request");
    let response = server.send(request).await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let request = Request::put("/r4b/CodeSystem/colours")
        .header(http::header::CONTENT_TYPE, "application/fhir+json")
        .header(IF_MATCH, "W/\"1\"")
        .body(Body::from(colours("2.0").to_string()))
        .expect("request");
    assert_eq!(server.send(request).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_write_is_refused_where_the_deployment_persists_nothing() {
    let server = Server::start_with_resources();
    let response = server.put("/r4b/CodeSystem/colours", &colours("1.0")).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let (_, body) = fixture::json(response).await;
    assert_eq!(body["resourceType"], "OperationOutcome");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .is_some_and(|text| text.contains("FERROTERM_RESOURCES")),
        "the refusal names the variable that turns persistence on: {body}"
    );
}

#[tokio::test]
async fn a_body_of_another_type_or_an_id_that_is_not_the_url_id_is_refused() {
    let server = Server::start_persisting();
    let response = server.put("/r4b/CodeSystem/colours", &colour_set()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let mut wrong = colours("1.0");
    wrong["id"] = json!("other");
    let response = server.put("/r4b/CodeSystem/colours", &wrong).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let (status, body) = server.get("/r4b/CodeSystem/never-written").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn the_capability_statement_declares_the_write_interactions_only_when_they_answer() {
    let server = Server::start_persisting();
    let (status, body) = server.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let codes = interactions(&body, "ValueSet");
    for wanted in ["read", "vread", "search-type", "create", "update", "delete"] {
        assert!(codes.contains(&wanted.to_owned()), "{wanted} in {codes:?}");
    }

    let plain = Server::start_with_resources();
    let (status, body) = plain.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let codes = interactions(&body, "ValueSet");
    assert_eq!(
        codes,
        vec![String::from("read"), String::from("search-type")],
        "a deployment that persists nothing declares no write"
    );
}

/// The interaction codes the capability statement declares on `resource_type`.
fn interactions(body: &Value, resource_type: &str) -> Vec<String> {
    body.get("rest")
        .and_then(|rest| rest.get(0))
        .and_then(|rest| rest.get("resource"))
        .and_then(Value::as_array)
        .expect("resources")
        .iter()
        .find(|resource| resource["type"] == resource_type)
        .and_then(|resource| resource["interaction"].as_array())
        .expect("interactions")
        .iter()
        .filter_map(|interaction| interaction["code"].as_str())
        .map(str::to_owned)
        .collect()
}
