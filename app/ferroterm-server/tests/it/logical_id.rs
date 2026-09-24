//! The logical id a loaded `CodeSystem`, `ValueSet`, or `ConceptMap` answers
//! on.
//!
//! The read interaction addresses a resource by its logical id
//! (<https://hl7.org/fhir/R4B/http.html#read>), and `Resource.id` is that id on
//! the server that holds the resource
//! (<https://hl7.org/fhir/R4B/resource.html#id>). A resource loaded from disk
//! keeps the id it was authored with, so a client that knows the id reads it
//! there; a resource that carries none keeps the id the server mints.

use ferroterm_server::config::Config;
use ferroterm_server::state::{AppState, LoadError, instance_id};
use http::StatusCode;
use serde_json::Value;

use crate::fixture::Server;

/// The `id` of the loaded `CodeSystem`.
const CODE_SYSTEM: &str = "e2e-taxonomy";
/// The `id` of the loaded `ValueSet`.
const VALUE_SET: &str = "e2e-taxonomy-all";
/// The `id` of the loaded `ConceptMap`.
const CONCEPT_MAP: &str = "e2e-taxonomy-map";
/// The canonical of the loaded `CodeSystem`.
const SYSTEM_URL: &str = "https://ferroterm.eu/fhir/CodeSystem/e2e-taxonomy";
/// The canonical of the loaded `ValueSet`.
const VALUE_SET_URL: &str = "https://ferroterm.eu/fhir/ValueSet/e2e-taxonomy-all";
/// The canonical of the loaded `ConceptMap`.
const CONCEPT_MAP_URL: &str = "https://ferroterm.eu/fhir/ConceptMap/e2e-taxonomy-map";

#[tokio::test]
async fn a_loaded_code_system_answers_at_the_id_it_was_authored_with() {
    let server = Server::start_with_authored_ids();
    let (status, body) = server.get(&format!("/r4b/CodeSystem/{CODE_SYSTEM}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], CODE_SYSTEM);
    assert_eq!(body["url"], SYSTEM_URL);

    let (status, body) = server
        .get(&format!("/r4b/CodeSystem?url={SYSTEM_URL}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(
        body["entry"][0]["fullUrl"],
        format!("CodeSystem/{CODE_SYSTEM}"),
        "a search names the resource by the id a read answers on"
    );
    assert_eq!(body["entry"][0]["resource"]["id"], CODE_SYSTEM);

    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/{CODE_SYSTEM}/$validate-code?code=aa-root"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameter(&body, "result")["valueBoolean"], true);

    // R5 declares `$lookup` at the instance level, R4B does not
    // (<https://hl7.org/fhir/R5/codesystem-operation-lookup.html>).
    let (status, body) = server
        .get(&format!(
            "/r5/CodeSystem/{CODE_SYSTEM}/$lookup?code=aa-root"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        parameter(&body, "display")["valueString"],
        "Root of the shaped taxonomy"
    );
}

#[tokio::test]
async fn a_loaded_value_set_answers_at_the_id_it_was_authored_with() {
    let server = Server::start_with_authored_ids();
    let (status, body) = server.get(&format!("/r4b/ValueSet/{VALUE_SET}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], VALUE_SET);
    assert_eq!(body["url"], VALUE_SET_URL);

    let (status, body) = server
        .get(&format!("/r4b/ValueSet?url={VALUE_SET_URL}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(
        body["entry"][0]["fullUrl"],
        format!("ValueSet/{VALUE_SET}"),
        "a search names the resource by the id a read answers on"
    );
    assert_eq!(body["entry"][0]["resource"]["id"], VALUE_SET);

    for base in ["r4", "r5", "r6"] {
        let (status, body) = server.get(&format!("/{base}/ValueSet/{VALUE_SET}")).await;
        assert_eq!(status, StatusCode::OK, "{base}: {body}");
        assert_eq!(body["id"], VALUE_SET, "{base}");
    }
}

#[tokio::test]
async fn a_loaded_concept_map_answers_at_the_id_it_was_authored_with() {
    let server = Server::start_with_authored_ids();
    let (status, body) = server.get(&format!("/r4b/ConceptMap/{CONCEPT_MAP}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["id"], CONCEPT_MAP);
    assert_eq!(body["url"], CONCEPT_MAP_URL);

    let (status, body) = server
        .get(&format!("/r4b/ConceptMap?url={CONCEPT_MAP_URL}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(
        body["entry"][0]["fullUrl"],
        format!("ConceptMap/{CONCEPT_MAP}"),
        "a search names the resource by the id a read answers on"
    );
    assert_eq!(body["entry"][0]["resource"]["id"], CONCEPT_MAP);

    for base in ["r4", "r5", "r6"] {
        let (status, body) = server
            .get(&format!("/{base}/ConceptMap/{CONCEPT_MAP}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{base}: {body}");
        assert_eq!(body["id"], CONCEPT_MAP, "{base}");
    }
}

#[test]
fn a_resource_without_an_id_keeps_the_one_the_server_mints() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_value_set(dir.path(), "anonymous.json", None, "https://a.example/vs");
    let state = AppState::load(&Config {
        code_systems: vec![dir.path().to_path_buf()],
        ..Config::default()
    })
    .expect("loads");
    let ids: Vec<String> = state
        .value_set_instances()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, [instance_id("https://a.example/vs", "1")]);
}

#[test]
fn two_loaded_resources_with_one_id_refuse_the_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_value_set(
        dir.path(),
        "first.json",
        Some("shared"),
        "https://a.example/first",
    );
    write_value_set(
        dir.path(),
        "second.json",
        Some("shared"),
        "https://a.example/second",
    );
    let loaded = AppState::load(&Config {
        code_systems: vec![dir.path().to_path_buf()],
        ..Config::default()
    });
    let Err(error) = loaded else {
        panic!("two resources carrying one id refuse the load");
    };
    assert!(
        matches!(&error, LoadError::DuplicateId { id, .. } if id == "shared"),
        "{error}"
    );
    let text = error.to_string();
    assert!(text.contains("https://a.example/first|1"), "{text}");
    assert!(text.contains("https://a.example/second|1"), "{text}");
}

#[test]
fn an_id_outside_the_fhir_id_alphabet_refuses_the_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_value_set(
        dir.path(),
        "outside.json",
        Some("not an id"),
        "https://a.example/vs",
    );
    let loaded = AppState::load(&Config {
        code_systems: vec![dir.path().to_path_buf()],
        ..Config::default()
    });
    let Err(error) = loaded else {
        panic!("an id outside the alphabet refuses the load");
    };
    // An `id` is at most 64 characters of `A-Z`, `a-z`, `0-9`, `-`, and `.`
    // (<https://hl7.org/fhir/R4B/datatypes.html#id>), which the generated codec
    // checks as it decodes, so the resource never reaches the model.
    let mut text = error.to_string();
    let mut cause: &dyn std::error::Error = &error;
    while let Some(source) = cause.source() {
        text = source.to_string();
        cause = source;
    }
    assert_eq!(text, "ValueSet.id: invalid value");
}

#[test]
fn a_loaded_id_a_persisted_record_holds_refuses_the_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_value_set(
        dir.path(),
        "authored.json",
        Some("held"),
        "https://a.example/vs",
    );
    let config = Config {
        code_systems: vec![dir.path().to_path_buf()],
        resources: Some(dir.path().join("resources.redb")),
        ..Config::default()
    };
    let state = AppState::load(&config).expect("loads");
    let written = serde_json::json!({
        "resourceType": "ValueSet",
        "id": "held",
        "url": "https://b.example/vs",
        "version": "1",
        "status": "active",
    });
    let object = match crate::fixture::document(&written) {
        fhir_types::codec::Value::Object(object) => object,
        other => panic!("the body is a resource: {other:?}"),
    };
    state
        .put_persisted(
            ferroterm_server::persistence::ResourceType::ValueSet,
            "held",
            "4.3.0",
            object,
        )
        .expect("writes");
    // `redb` holds the database file for as long as its handle lives
    // (<https://docs.rs/redb/latest/redb/struct.Database.html>).
    drop(state);

    let loaded = AppState::load(&config);
    let Err(error) = loaded else {
        panic!("a loaded id a record holds refuses the load");
    };
    assert!(
        matches!(&error, LoadError::PersistedId { id, .. } if id == "held"),
        "{error}"
    );
    assert!(
        error.to_string().contains("https://a.example/vs|1"),
        "{error}"
    );
}

/// The `Parameters.parameter` of `name`, or `Value::Null`.
fn parameter(body: &Value, name: &str) -> Value {
    body["parameter"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == name)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Writes a minimal `ValueSet` resource, with `id` when one is given.
fn write_value_set(dir: &std::path::Path, name: &str, id: Option<&str>, url: &str) {
    let mut resource = serde_json::json!({
        "resourceType": "ValueSet",
        "url": url,
        "version": "1",
        "status": "active",
    });
    if let Some(id) = id
        && let Value::Object(object) = &mut resource
    {
        object.insert(String::from("id"), Value::String(id.to_owned()));
    }
    std::fs::write(
        dir.join(name),
        serde_json::to_string_pretty(&resource).expect("writes"),
    )
    .expect("writes");
}
