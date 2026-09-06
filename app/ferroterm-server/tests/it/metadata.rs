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

/// A deployment behind a proxy states the URL clients reach it at.
///
/// The server answers on an address it never sees when a proxy terminates TLS
/// in front of it, so `implementation.url` is configured rather than guessed,
/// and it carries the version prefix the surface is served under
/// (<https://hl7.org/fhir/R4B/capabilitystatement-definitions.html#CapabilityStatement.implementation.url>,
/// #123).
#[tokio::test]
async fn the_capability_statements_state_the_configured_base_url() {
    let server = Server::start();
    let (_, body) = server.get("/r4b/metadata").await;
    assert!(
        body["implementation"]["url"].is_null(),
        "a deployment that names no base states none: {body}"
    );

    let server = Server::start_with_base_url("https://tx.example.org/fhir");
    let (_, body) = server.get("/r4b/metadata").await;
    assert_eq!(
        body["implementation"]["url"], "https://tx.example.org/fhir/r4b",
        "the version prefix is part of the endpoint: {body}"
    );
    let (_, body) = server.get("/r5/metadata").await;
    assert_eq!(
        body["implementation"]["url"],
        "https://tx.example.org/fhir/r5"
    );
    let (_, body) = server.get("/r4b/metadata?mode=terminology").await;
    assert_eq!(
        body["implementation"]["url"], "https://tx.example.org/fhir/r4b",
        "the terminology capabilities say the same: {body}"
    );
}

/// The canonical of the artifact declaration, and its two sub-extensions.
const ARTIFACT: &str = "https://ferroterm.eu/fhir/StructureDefinition/terminology-artifact";

/// The artifact directory names `start_with_every_loader` writes, by system.
const ARTIFACTS: [(&str, &str); 3] = [
    ("http://snomed.info/sct", "snomed"),
    ("http://loinc.org", "loinc"),
    ("http://www.nlm.nih.gov/research/umls/rxnorm", "rxnorm"),
];

// NOTE: no FHIR or SNOMED specification records which index a server read, so
// the declaration is an extension on `codeSystem.version`, a `BackboneElement`
// that admits one in every version (<https://hl7.org/fhir/R4B/extensibility.html>).
#[tokio::test]
async fn every_version_declares_the_artifact_each_index_backed_system_came_from() {
    let server = Server::start_with_every_loader();
    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server
            .get(&format!("/{version}/metadata?mode=terminology"))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        let systems = body["codeSystem"].as_array().expect("codeSystem is a list");
        for entry in systems {
            let uri = entry["uri"].as_str().unwrap_or_default();
            let declared = ARTIFACTS.iter().find(|(system, _)| *system == uri);
            for served in entry["version"].as_array().expect("version is a list") {
                let extension = &served["extension"][0];
                match declared {
                    Some((_, name)) => {
                        assert_eq!(extension["url"], ARTIFACT, "{version} {uri}: {body}");
                        assert_eq!(
                            extension["extension"][0]["valueString"], *name,
                            "{version} {uri} names its artifact: {body}"
                        );
                        assert!(
                            extension["extension"][1]["valueString"].is_string(),
                            "{version} {uri} names its release: {body}"
                        );
                    }
                    None => assert!(
                        served.get("extension").is_none(),
                        "{version} {uri} came from no artifact and declares none: {body}"
                    ),
                }
            }
        }
    }
}
