//! `GET /r4b/metadata` and `?mode=terminology`.

use http::StatusCode;

use crate::fixture::Server;

#[tokio::test]
async fn the_capability_statement_names_the_operations() {
    let server = Server::start();
    let (status, body) = server.get("/r4b/metadata").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "CapabilityStatement");
    assert_eq!(body["kind"], "instance");
    assert_eq!(body["fhirVersion"], "4.3.0");
    assert_eq!(body["status"], "active");
    assert!(body["date"].as_str().is_some_and(|d| d.contains('T')));
    assert_eq!(body["software"]["name"], "FerroTERM");
    let operations = &body["rest"][0]["resource"][0]["operation"];
    let names: Vec<&str> = operations
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["lookup", "validate-code", "subsumes"]);
    assert_eq!(
        operations[1]["definition"],
        "http://hl7.org/fhir/OperationDefinition/CodeSystem-validate-code"
    );
    assert_eq!(body["rest"][0]["resource"][0]["type"], "CodeSystem");
    let (normative, _) = server.get("/r4b/metadata?mode=normative").await;
    assert_eq!(normative, StatusCode::OK);
}

#[tokio::test]
async fn terminology_capabilities_list_the_loaded_edition() {
    let server = Server::start();
    let (status, body) = server.get("/r4b/metadata?mode=terminology").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "TerminologyCapabilities");
    assert_eq!(body["kind"], "instance");
    assert_eq!(body["codeSystem"][0]["uri"], "http://snomed.info/sct");
    assert_eq!(
        body["codeSystem"][0]["version"][0]["code"],
        ferroterm_testkit::snomed::VERSION
    );
    assert_eq!(body["codeSystem"][0]["version"][0]["isDefault"], true);
    assert_eq!(body["codeSystem"][0]["subsumption"], true);
    assert_eq!(body["software"]["name"], "FerroTERM");
    let (bad, outcome) = server.get("/r4b/metadata?mode=xml").await;
    assert_eq!(bad, StatusCode::BAD_REQUEST);
    assert_eq!(outcome["issue"][0]["code"], "value");
}

// NOTE: <https://hl7.org/fhir/uv/tx-ecosystem/requirements.html> (Metadata) requires
// a server to populate `rest[mode = server].security.service`; the binding is extensible,
// so a deployment that authenticates nobody says so in text.
#[tokio::test]
async fn the_capability_statement_names_its_security_service_on_every_version() {
    let server = Server::start();
    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server.get(&format!("/{version}/metadata")).await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        let service = &body["rest"][0]["security"]["service"][0];
        assert!(
            service["coding"].as_array().is_none_or(Vec::is_empty),
            "{version}: no deployment declared one: {service}"
        );
        assert!(
            service["text"]
                .as_str()
                .is_some_and(|text| text.contains("no authentication of its own")),
            "{version}: {service}"
        );
    }
}
