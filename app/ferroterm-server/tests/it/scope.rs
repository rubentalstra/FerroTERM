//! Request-scoped resources: `tx-resource`, `$cache-control` with
//! `X-Cache-Id`, `$versions`, and the `ValueSet` read and search
//! (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/requirements.html>).

use axum::body::Body;
use http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::fixture::{Server, json as read_json};
use ferroterm_testkit::fhir::{ANIMALS, VS_PETS};

const INLINE_CS: &str = "http://example.org/fhir/CodeSystem/inline";
const INLINE_VS: &str = "http://example.org/fhir/ValueSet/inline";

fn inline_code_system() -> Value {
    json!({"resourceType": "CodeSystem", "url": INLINE_CS, "version": "1", "status": "active",
        "content": "complete", "concept": [{"code": "x", "display": "Ex"}, {"code": "y", "display": "Why"}]})
}

fn inline_value_set() -> Value {
    json!({"resourceType": "ValueSet", "url": INLINE_VS, "status": "active",
        "compose": {"include": [{"system": INLINE_CS, "concept": [{"code": "x"}]}]}})
}

fn param<'a>(body: &'a Value, name: &str) -> Option<&'a Value> {
    body["parameter"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

async fn post_with(
    server: &Server,
    uri: &str,
    cache: Option<&str>,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request =
        Request::post(uri).header(http::header::CONTENT_TYPE, "application/fhir+json");
    if let Some(cache) = cache {
        request = request.header("X-Cache-Id", cache);
    }
    let request = request.body(Body::from(body.to_string())).expect("request");
    read_json(server.router().oneshot(request).await.expect("response")).await
}

#[tokio::test]
async fn tx_resources_serve_a_request_and_only_that_request() {
    let server = Server::start();
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$expand",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": INLINE_VS},
                {"name": "tx-resource", "resource": inline_code_system()},
                {"name": "tx-resource", "resource": inline_value_set()}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["expansion"]["contains"][0]["code"], "x");
    assert_eq!(body["expansion"]["contains"][0]["display"], "Ex");
    let (status, body) = server
        .post(
            "/r4b/CodeSystem/$lookup",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "system", "valueUri": INLINE_CS},
                {"name": "code", "valueCode": "y"},
                {"name": "tx-resource", "resource": inline_code_system()}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "display").expect("display")["valueString"],
        "Why"
    );
    let (status, _) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={INLINE_CS}&code=y"
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "gone after the request");
}

#[tokio::test]
async fn a_tx_resource_of_another_type_is_refused() {
    let server = Server::start();
    let (status, body) = server
        .post(
            "/r4b/ValueSet/$validate-code",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "url", "valueUri": VS_PETS},
                {"name": "code", "valueCode": "cat"},
                {"name": "system", "valueUri": ANIMALS},
                {"name": "tx-resource", "resource": {"resourceType": "Parameters"}}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["issue"][0]["code"], "not-supported");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .is_some_and(|d| d.contains("Parameters"))
    );
}

#[tokio::test]
async fn a_cache_front_loads_resources_and_ends() {
    let server = Server::start();
    let (status, body) = server
        .post(
            "/r4b/$cache-control?mode=start",
            &json!({"resourceType": "Parameters", "parameter": [
                {"name": "tx-resource", "resource": inline_code_system()},
                {"name": "tx-resource", "resource": inline_value_set()}
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id = param(&body, "cache-id").expect("cache-id")["valueId"]
        .as_str()
        .expect("id")
        .to_owned();
    let validate = json!({"resourceType": "Parameters", "parameter": [
        {"name": "url", "valueUri": INLINE_VS},
        {"name": "code", "valueCode": "x"},
        {"name": "system", "valueUri": INLINE_CS}
    ]});
    let (status, body) = post_with(
        &server,
        "/r4b/ValueSet/$validate-code",
        Some(&id),
        &validate,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        param(&body, "result").expect("result")["valueBoolean"],
        true
    );
    let (status, body) = post_with(&server, "/r4b/ValueSet/$validate-code", None, &validate).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "without the cache the value set is unknown: {body}"
    );
    let (status, _) = post_with(
        &server,
        "/r4b/$cache-control?mode=end",
        Some(&id),
        &json!({"resourceType": "Parameters"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post_with(
        &server,
        "/r4b/ValueSet/$validate-code",
        Some(&id),
        &validate,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .is_some_and(|d| d.contains("cache"))
    );
    let (status, body) = server
        .post(
            "/r4b/$cache-control?mode=sideways",
            &json!({"resourceType": "Parameters"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn versions_and_the_capability_statement_carry_what_the_runner_reads() {
    let server = Server::start_with_resources();
    let (status, body) = server.get("/r4b/$versions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        param(&body, "default").expect("default")["valueCode"],
        "4.3"
    );
    assert_eq!(
        param(&body, "version").expect("version")["valueCode"],
        "4.3"
    );
    let (status, body) = server.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["instantiates"][0],
        "http://hl7.org/fhir/CapabilityStatement/terminology-server"
    );
    assert!(body["url"].as_str().is_some());
    let definitions: Vec<&str> = body["extension"]
        .as_array()
        .expect("features")
        .iter()
        .filter_map(|f| f["extension"][0]["valueCanonical"].as_str())
        .collect();
    assert_eq!(definitions.len(), 2);
    assert!(
        definitions
            .iter()
            .any(|d| d.ends_with("/CodeSystemAsParameter"))
    );
    let system_ops: Vec<&str> = body["rest"][0]["operation"]
        .as_array()
        .expect("operations")
        .iter()
        .filter_map(|o| o["name"].as_str())
        .collect();
    assert_eq!(system_ops, ["versions", "cache-control"]);
    let value_set = body["rest"][0]["resource"]
        .as_array()
        .expect("resources")
        .iter()
        .find(|r| r["type"] == "ValueSet")
        .expect("ValueSet")
        .clone();
    let interactions: Vec<&str> = value_set["interaction"]
        .as_array()
        .expect("interactions")
        .iter()
        .filter_map(|i| i["code"].as_str())
        .collect();
    assert_eq!(interactions, ["read", "search-type"]);
    let (status, body) = server.get("/r4b/metadata?mode=terminology").await;
    assert_eq!(status, StatusCode::OK);
    let parameters: Vec<&str> = body["expansion"]["parameter"]
        .as_array()
        .expect("parameters")
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert!(parameters.contains(&"tx-resource"));
    assert!(parameters.contains(&"displayLanguage"));
    assert_eq!(body["name"], "FerroTERM");
}

#[tokio::test]
async fn value_sets_read_and_search_by_url() {
    let server = Server::start_with_resources();
    let (status, body) = server.get(&format!("/r4b/ValueSet?url={VS_PETS}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["resourceType"], "Bundle");
    assert_eq!(body["type"], "searchset");
    assert_eq!(body["total"], 1);
    let entry = &body["entry"][0];
    assert_eq!(entry["resource"]["url"], VS_PETS);
    assert_eq!(entry["search"]["mode"], "match");
    let id = entry["fullUrl"]
        .as_str()
        .and_then(|u| u.strip_prefix("ValueSet/"))
        .expect("id")
        .to_owned();
    let (status, body) = server.get(&format!("/r4b/ValueSet/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["resourceType"], "ValueSet");
    assert_eq!(body["url"], VS_PETS);
    assert_eq!(body["compose"]["include"][0]["filter"][0]["op"], "is-a");
    let (status, body) = server
        .get("/r4b/ValueSet?url=http://example.org/fhir/ValueSet/animals-all&version=1.0")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    let (status, body) = server.get("/r4b/ValueSet").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 7);
    let (status, _) = server.get("/r4b/ValueSet/nowhere").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = server.get("/r4b/ValueSet?name=pets").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
