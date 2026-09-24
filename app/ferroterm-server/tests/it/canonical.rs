//! One canonical resource per `url` and `version`.
//!
//! The `url` and `version` of a canonical resource identify one resource
//! (<https://hl7.org/fhir/R4B/resource.html#canonical>,
//! <https://hl7.org/fhir/R4B/references.html#canonical>), so a write whose
//! pair another resource of the type already carries is refused with 409 and
//! a `duplicate` issue (<https://hl7.org/fhir/R4B/valueset-issue-type.html>).

use ferroterm_server::config::Config;
use ferroterm_server::persistence::{Record, ResourceStore};
use http::StatusCode;
use serde_json::{Value, json};

use crate::fixture::{self, Server};

/// Every served FHIR base.
const BASES: [&str; 4] = ["r4", "r4b", "r5", "r6"];

/// A complete `CodeSystem` with `url`, `version`, and one concept `code`.
fn code_system(url: &str, version: &str, code: &str) -> Value {
    json!({
        "resourceType": "CodeSystem",
        "url": url,
        "version": version,
        "status": "active",
        "content": "complete",
        "concept": [{"code": code, "display": code}]
    })
}

/// The canonical this test writes on `base`, so the four bases do not share
/// one.
fn url_on(base: &str) -> String {
    format!("http://ferroterm.test/CodeSystem/shared-{base}")
}

/// Asserts that `body` is the `duplicate` refusal naming `holder`.
fn assert_duplicate(status: StatusCode, body: &Value, holder: &str) {
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["resourceType"], "OperationOutcome", "{body}");
    assert_eq!(
        body.pointer("/issue/0/code").and_then(Value::as_str),
        Some("duplicate"),
        "{body}"
    );
    let diagnostics = body
        .pointer("/issue/0/diagnostics")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        diagnostics.contains(holder),
        "the refusal names the resource that holds the canonical: {body}"
    );
}

#[tokio::test]
async fn a_create_onto_a_persisted_canonical_is_refused() {
    let server = Server::start_persisting();
    for base in BASES {
        let url = url_on(base);
        let holder = format!("holder-{base}");
        let response = server
            .put(
                &format!("/{base}/CodeSystem/{holder}"),
                &code_system(&url, "1", "a"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{base}");

        let (status, body) = server
            .post(&format!("/{base}/CodeSystem"), &code_system(&url, "1", "b"))
            .await;
        assert_duplicate(status, &body, &holder);
    }
}

#[tokio::test]
async fn an_update_onto_a_persisted_canonical_is_refused() {
    let server = Server::start_persisting();
    for base in BASES {
        let url = url_on(base);
        let holder = format!("holder-{base}");
        let response = server
            .put(
                &format!("/{base}/CodeSystem/{holder}"),
                &code_system(&url, "1", "a"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{base}");

        // A new id and an existing one are both refused.
        let other = format!("other-{base}");
        let (status, body) = fixture::json(
            server
                .put(
                    &format!("/{base}/CodeSystem/{other}"),
                    &code_system(&url, "1", "b"),
                )
                .await,
        )
        .await;
        assert_duplicate(status, &body, &holder);

        let response = server
            .put(
                &format!("/{base}/CodeSystem/{other}"),
                &code_system(&url, "2", "b"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{base}");
        let (status, body) = fixture::json(
            server
                .put(
                    &format!("/{base}/CodeSystem/{other}"),
                    &code_system(&url, "1", "b"),
                )
                .await,
        )
        .await;
        assert_duplicate(status, &body, &holder);

        let (status, body) = server.get(&format!("/{base}/CodeSystem/{holder}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["concept"][0]["code"], "a",
            "the holder is untouched: {body}"
        );
    }
}

#[tokio::test]
async fn the_holder_updates_and_another_version_is_written() {
    let server = Server::start_persisting();
    for base in BASES {
        let url = url_on(base);
        let holder = format!("holder-{base}");
        let path = format!("/{base}/CodeSystem/{holder}");
        assert_eq!(
            server
                .put(&path, &code_system(&url, "1", "a"))
                .await
                .status(),
            StatusCode::CREATED,
            "{base}"
        );
        assert_eq!(
            server
                .put(&path, &code_system(&url, "1", "b"))
                .await
                .status(),
            StatusCode::OK,
            "{base}: the resource that holds the canonical updates it"
        );
        let (status, body) = server
            .post(&format!("/{base}/CodeSystem"), &code_system(&url, "2", "c"))
            .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "{base}: the same url under another version is another resource: {body}"
        );
    }
}

#[tokio::test]
async fn a_write_onto_a_loaded_canonical_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resources = dir.path().to_path_buf();
    std::fs::write(
        resources.join("loaded.json"),
        json!({
            "resourceType": "ValueSet",
            "id": "loaded",
            "url": "https://a.example/vs",
            "version": "1",
            "status": "active",
        })
        .to_string(),
    )
    .expect("writes");
    let server = Server::start_with_config(
        dir,
        Config {
            code_systems: vec![resources.clone()],
            resources: Some(resources.join("resources.redb")),
            ..Config::default()
        },
    );
    for base in BASES {
        let written = json!({
            "resourceType": "ValueSet",
            "url": "https://a.example/vs",
            "version": "1",
            "status": "active",
        });
        let (status, body) = server.post(&format!("/{base}/ValueSet"), &written).await;
        assert_duplicate(status, &body, "loaded");
        let (status, body) = fixture::json(
            server
                .put(&format!("/{base}/ValueSet/mine"), &written)
                .await,
        )
        .await;
        assert_duplicate(status, &body, "loaded");

        let mut other = written.clone();
        other["version"] = json!(format!("2-{base}"));
        let (status, body) = fixture::json(
            server
                .put(&format!("/{base}/ValueSet/mine-{base}"), &other)
                .await,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "{base}: another version of a loaded url is written: {body}"
        );
    }
}

#[tokio::test]
async fn a_stored_duplicate_answers_from_the_most_recent_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let database = dir.path().join("resources.redb");
    let url = "http://ferroterm.test/CodeSystem/stored-twice";
    let store = ResourceStore::open(&database).expect("opens");
    // The older write carries the id that sorts last, so an id order would
    // answer from it.
    for (id, code, written) in [
        ("zz-older", "older", "2026-01-01T00:00:00Z"),
        ("aa-newer", "newer", "2026-02-01T00:00:00Z"),
    ] {
        let mut resource = match fixture::document(&code_system(url, "1", code)) {
            fhir_types::codec::Value::Object(object) => object,
            other => panic!("the body is a resource: {other:?}"),
        };
        resource.insert(
            String::from("id"),
            fhir_types::codec::Value::String(id.to_owned()),
        );
        store
            .put(&Record {
                resource_type: String::from("CodeSystem"),
                id: id.to_owned(),
                url: Some(url.to_owned()),
                version: Some(String::from("1")),
                fhir_version: String::from("4.3.0"),
                version_id: 1,
                last_modified: written.to_owned(),
                resource,
            })
            .expect("writes");
    }
    // `redb` holds the database file for as long as its handle lives
    // (<https://docs.rs/redb/latest/redb/struct.Database.html>).
    drop(store);

    let server = Server::start_with_config(
        dir,
        Config {
            resources: Some(database),
            ..Config::default()
        },
    );
    for base in BASES {
        let (status, body) = server
            .get(&format!(
                "/{base}/CodeSystem/$validate-code?url={url}&version=1&code=newer"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{base}: {body}");
        assert_eq!(
            parameter(&body, "result")["valueBoolean"],
            true,
            "{base}: the most recent write answers for the canonical: {body}"
        );
        let (status, body) = server
            .get(&format!(
                "/{base}/CodeSystem/$validate-code?url={url}&version=1&code=older"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{base}: {body}");
        assert_eq!(
            parameter(&body, "result")["valueBoolean"],
            false,
            "{base}: {body}"
        );
    }
}

/// The `Parameters.parameter` of `name`, or `Value::Null`.
fn parameter(body: &Value, name: &str) -> Value {
    body.get("parameter")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|part| part.get("name").and_then(Value::as_str) == Some(name))
        .cloned()
        .unwrap_or(Value::Null)
}
