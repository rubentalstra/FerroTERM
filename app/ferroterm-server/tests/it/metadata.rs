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
