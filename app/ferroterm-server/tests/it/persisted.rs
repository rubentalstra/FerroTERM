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

const LIFECYCLE: &str = "http://ferroterm.test/CodeSystem/lifecycle";
const LIFECYCLE_SET: &str = "http://ferroterm.test/ValueSet/lifecycle";

/// A system carrying one status marker per concept, with the R5 property set
/// (<https://hl7.org/fhir/R5/codesystem-concept-properties.html>). A date the
/// request is behind retires a concept and one still to come does not.
fn lifecycle() -> Value {
    let property = |code: &str, kind: &str| {
        json!({
            "code": code,
            "uri": format!("http://hl7.org/fhir/concept-properties#{code}"),
            "type": kind
        })
    };
    json!({
        "resourceType": "CodeSystem",
        "url": LIFECYCLE,
        "version": "1.0",
        "status": "active",
        "content": "complete",
        "caseSensitive": true,
        "property": [
            property("status", "code"),
            property("inactive", "boolean"),
            property("deprecated", "dateTime"),
            property("deprecationDate", "dateTime"),
            property("retirementDate", "dateTime")
        ],
        "concept": [
            {"code": "current", "display": "Current"},
            {"code": "deprecated", "display": "Deprecated",
             "property": [{"code": "status", "valueCode": "deprecated"},
                          {"code": "deprecated", "valueDateTime": "2001-06-15"},
                          {"code": "deprecationDate", "valueDateTime": "2001-06-15"}]},
            {"code": "flagged", "display": "Flagged",
             "property": [{"code": "inactive", "valueBoolean": true}]},
            {"code": "retired", "display": "Retired",
             "property": [{"code": "status", "valueCode": "retired"}]},
            {"code": "retired-date", "display": "Retired by date",
             "property": [{"code": "retirementDate", "valueDateTime": "2001-06-15"}]},
            {"code": "retiring", "display": "Retiring later",
             "property": [{"code": "retirementDate", "valueDateTime": "2999-01-01"}]},
            {"code": "contradiction", "display": "Flagged active and retired",
             "property": [{"code": "inactive", "valueBoolean": false},
                          {"code": "status", "valueCode": "retired"}]}
        ]
    })
}

fn lifecycle_set() -> Value {
    json!({
        "resourceType": "ValueSet",
        "url": LIFECYCLE_SET,
        "version": "1.0",
        "status": "active",
        "compose": {"include": [{"system": LIFECYCLE}]}
    })
}

/// The concepts of an expansion body, in the order the page carries them.
fn expansion_entries(body: &Value) -> &[Value] {
    body.get("expansion")
        .and_then(|expansion| expansion.get("contains"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The codes of `entries`.
fn expansion_codes(entries: &[Value]) -> Vec<&str> {
    entries
        .iter()
        .filter_map(|entry| entry.get("code").and_then(Value::as_str))
        .collect()
}

// `activeOnly` "controls whether inactive concepts are included or excluded in
// value set expansions" (<https://hl7.org/fhir/R4/valueset-operation-expand.html>,
// <https://hl7.org/fhir/R5/valueset-operation-expand.html>); the markers that
// make a concept inactive are the standard concept properties, which a
// persisted resource carries as written on every version.
#[tokio::test]
async fn the_concept_status_properties_answer_the_same_on_every_served_version() {
    let server = Server::start_persisting();
    assert_eq!(
        server
            .put("/r4/CodeSystem/lifecycle", &lifecycle())
            .await
            .status(),
        StatusCode::CREATED,
        "the R5 concept properties are data, so an R4 write takes them"
    );
    server.put("/r4/ValueSet/lifecycle", &lifecycle_set()).await;

    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$expand?url={LIFECYCLE_SET}&activeOnly=true"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            expansion_codes(expansion_entries(&body)),
            ["current", "deprecated", "retiring"],
            "{version} drops every concept a marker retires"
        );

        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$expand?url={LIFECYCLE_SET}&activeOnly=false"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        let inactive: Vec<&str> = expansion_entries(&body)
            .iter()
            .filter(|entry| entry.get("inactive") == Some(&json!(true)))
            .filter_map(|entry| entry.get("code").and_then(Value::as_str))
            .collect();
        assert_eq!(
            inactive,
            ["contradiction", "flagged", "retired", "retired-date"],
            "{version} keeps them and flags them"
        );
    }
}

// "Inactive is not invalid": the code validates, and the warning and the
// `inactive` output say what it is
// (<https://hl7.org/fhir/R5/valueset-operation-validate-code.html>).
#[tokio::test]
async fn validate_code_calls_a_retired_concept_valid_and_inactive_on_every_version() {
    let server = Server::start_persisting();
    server.put("/r4/CodeSystem/lifecycle", &lifecycle()).await;

    for version in ["r4", "r4b", "r5", "r6"] {
        for code in ["flagged", "retired", "retired-date", "contradiction"] {
            let (status, body) = server
                .get(&format!(
                    "/{version}/CodeSystem/$validate-code?url={LIFECYCLE}&code={code}"
                ))
                .await;
            assert_eq!(status, StatusCode::OK, "{version} {code}: {body}");
            assert_eq!(
                fixture::parameter(&body, "result").expect("result")["valueBoolean"],
                json!(true),
                "{version} {code}: inactive is not invalid"
            );
            assert_eq!(
                fixture::parameter(&body, "inactive").expect("inactive")["valueBoolean"],
                json!(true),
                "{version} {code}: {body}"
            );
        }

        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$validate-code?url={LIFECYCLE}&code=deprecated"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert!(
            fixture::parameter(&body, "inactive").is_none(),
            "{version}: a deprecated concept is still active: {body}"
        );
    }
}

// `$lookup` answers the properties the system states for the concept
// (<https://hl7.org/fhir/R4/codesystem-operation-lookup.html>), so the
// lifecycle properties come back as written.
#[tokio::test]
async fn lookup_answers_the_declared_status_properties_as_written() {
    let server = Server::start_persisting();
    server.put("/r4/CodeSystem/lifecycle", &lifecycle()).await;

    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$lookup?system={LIFECYCLE}&code=retired-date&property=*"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        let properties: Vec<(&str, &Value)> = body["parameter"]
            .as_array()
            .expect("parameters")
            .iter()
            .filter(|parameter| parameter["name"] == "property")
            .filter_map(|parameter| {
                let parts = parameter["part"].as_array()?;
                let code = parts.first()?["valueCode"].as_str()?;
                Some((code, &parts.get(1)?["valueDateTime"]))
            })
            .collect();
        assert_eq!(
            properties
                .iter()
                .map(|(code, _)| *code)
                .collect::<Vec<&str>>(),
            ["inactive", "retirementDate"],
            "{version}: {body}"
        );
        assert_eq!(
            properties[1].1,
            &json!("2001-06-15"),
            "{version}: the date is the one the resource carried"
        );
    }
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
