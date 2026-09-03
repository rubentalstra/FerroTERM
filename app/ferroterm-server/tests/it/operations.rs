//! The `CodeSystem` operations over HTTP: GET and POST, type and instance level,
//! and every refusal as an `OperationOutcome`.

use ferroterm_testkit::snomed::{ANIMAL, CAT, DOG, FISH, VERSION, item, sctid};
use http::StatusCode;
use serde_json::json;

use crate::fixture::{Server, parameter, parameters};

const SCT: &str = "http://snomed.info/sct";

#[tokio::test]
async fn lookup_by_get_and_post() {
    let server = Server::start();
    let cat = sctid(item(CAT));
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={SCT}&code={cat}&displayLanguage=nl"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resourceType"], "Parameters");
    assert_eq!(
        parameter(&body, "name").unwrap()["valueString"],
        "SNOMED CT"
    );
    assert_eq!(parameter(&body, "version").unwrap()["valueString"], VERSION);
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Kat");
    let designations = body["parameter"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "designation")
        .count();
    assert_eq!(designations, 5);
    let inactive = body["parameter"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "property" && p["part"][0]["valueCode"] == "inactive")
        .expect("inactive property");
    assert_eq!(inactive["part"][1]["valueBoolean"], false);

    let (status, body) = server
        .post(
            "/r4b/CodeSystem/$lookup",
            &parameters(&[
                (
                    "coding",
                    json!({"valueCoding": {"system": SCT, "code": cat}}),
                ),
                ("property", json!({"valueCode": "sufficientlyDefined"})),
                ("property", json!({"valueCode": "lang.en"})),
            ]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let properties: Vec<&str> = body["parameter"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "property")
        .map(|p| p["part"][0]["valueCode"].as_str().unwrap())
        .collect();
    assert_eq!(properties, ["sufficientlyDefined"]);
    let english = body["parameter"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "designation")
        .count();
    assert_eq!(english, 3, "lang.en keeps the English designations only");
}

#[tokio::test]
async fn lookup_refusals_on_the_wire() {
    let server = Server::start();
    let cat = sctid(item(CAT));
    // Undeclared query parameter.
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={SCT}&code={cat}&colour=red"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
    // A complex parameter on GET.
    let (status, body) = server.get("/r4b/CodeSystem/$lookup?coding=x").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "not-supported");
    // No system.
    let (status, body) = server
        .get(&format!("/r4b/CodeSystem/$lookup?code={cat}"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "required");
    // Unknown code: 400 not-found (the R4B page's example).
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system={SCT}&code={}",
            sctid(4242)
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "not-found");
    // Unknown system: 404.
    let (status, _) = server
        .get(&format!(
            "/r4b/CodeSystem/$lookup?system=http://loinc.org&code={cat}"
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Instance-level $lookup is not an R4B invocation: the route does not exist.
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/{}/$lookup?code={cat}",
            server.snomed_id()
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["resourceType"], "OperationOutcome");
    // A repeated singular parameter in a POST body.
    let (status, body) = server
        .post(
            "/r4b/CodeSystem/$lookup",
            &parameters(&[
                ("system", json!({"valueUri": SCT})),
                ("code", json!({"valueCode": cat})),
                ("code", json!({"valueCode": cat})),
            ]),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "invalid");
    // Not JSON, and not FHIR JSON.
    let (status, body) = server
        .post_raw(
            "/r4b/CodeSystem/$lookup",
            "application/fhir+json",
            "{not json",
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "structure");
    let (status, _) = server
        .post_raw("/r4b/CodeSystem/$lookup", "text/plain", "{}")
        .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let (status, body) = server
        .post_raw(
            "/r4b/CodeSystem/$lookup",
            "application/fhir+json",
            r#"{"resourceType":"Patient"}"#,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "structure");
}

#[tokio::test]
async fn validate_code_at_type_and_instance_level() {
    let server = Server::start();
    let cat = sctid(item(CAT));
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$validate-code?url={SCT}&code={cat}&display=Kat"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Cat");
    // Wrong display: false with the correct display.
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$validate-code?url={SCT}&code={cat}&display=Dog"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], false);
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Cat");
    // Inactive: true with a message.
    let (_, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$validate-code?url={SCT}&code={}",
            sctid(item(FISH))
        ))
        .await;
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    assert!(
        parameter(&body, "message").unwrap()["valueString"]
            .as_str()
            .unwrap()
            .contains("inactive")
    );
    // Instance level: the instance is the system; `system` is not an R4B parameter.
    let id = server.snomed_id();
    let (status, body) = server
        .get(&format!("/r4b/CodeSystem/{id}/$validate-code?code={cat}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/{id}/$validate-code?system={SCT}&code={cat}"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["issue"][0]["diagnostics"]
            .as_str()
            .unwrap()
            .contains("system")
    );
    let (status, _) = server
        .get(&format!("/r4b/CodeSystem/nope/$validate-code?code={cat}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // POST with a codeableConcept.
    let (status, body) = server
        .post(
            &format!("/r4b/CodeSystem/{id}/$validate-code"),
            &parameters(&[(
                "codeableConcept",
                json!({"valueCodeableConcept": {"coding": [
                    {"system": "http://loinc.org", "code": "1234-5"},
                    {"system": SCT, "code": sctid(item(DOG))}
                ]}}),
            )]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Dog");
}

#[tokio::test]
async fn subsumes_at_type_and_instance_level() {
    let server = Server::start();
    let (animal, cat, dog) = (sctid(item(ANIMAL)), sctid(item(CAT)), sctid(item(DOG)));
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$subsumes?system={SCT}&codeA={animal}&codeB={cat}"
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        parameter(&body, "outcome").unwrap()["valueCode"],
        "subsumes"
    );
    let id = server.snomed_id();
    let (_, body) = server
        .get(&format!(
            "/r4b/CodeSystem/{id}/$subsumes?codeA={cat}&codeB={dog}"
        ))
        .await;
    assert_eq!(
        parameter(&body, "outcome").unwrap()["valueCode"],
        "not-subsumed"
    );
    let (status, body) = server
        .post(
            "/r4b/CodeSystem/$subsumes",
            &parameters(&[
                (
                    "codingA",
                    json!({"valueCoding": {"system": SCT, "code": cat}}),
                ),
                (
                    "codingB",
                    json!({"valueCoding": {"system": SCT, "code": animal}}),
                ),
            ]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        parameter(&body, "outcome").unwrap()["valueCode"],
        "subsumed-by"
    );
    // An unknown code is an error, never not-subsumed.
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$subsumes?system={SCT}&codeA={cat}&codeB={}",
            sctid(4242)
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "not-found");
    // No system at type level.
    let (status, body) = server
        .get(&format!(
            "/r4b/CodeSystem/$subsumes?codeA={cat}&codeB={dog}"
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["issue"][0]["code"], "required");
}

/// `Accept-Language` picks the display language when `displayLanguage` is
/// absent, the parameter wins when both are given, and the header's quality
/// order and wildcard are honoured
/// (<https://build.fhir.org/ig/HL7/fhir-tx-ecosystem-ig/languages.html>).
#[tokio::test]
async fn accept_language_selects_the_display_when_no_parameter_does() {
    let server = Server::start();
    let cat = sctid(item(CAT));
    let uri = format!("/r4b/CodeSystem/$lookup?system={SCT}&code={cat}&property=display");
    let (status, body) = server.get_with_header(&uri, "Accept-Language", "nl").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Kat");
    let (_, body) = server
        .get_with_header(
            &format!("{uri}&displayLanguage=en"),
            "Accept-Language",
            "nl",
        )
        .await;
    assert_eq!(
        parameter(&body, "display").unwrap()["valueString"],
        "Cat",
        "the parameter wins"
    );
    let (_, body) = server
        .get_with_header(&uri, "Accept-Language", "fr;q=0.9, nl;q=0.8, en;q=0.7")
        .await;
    assert_eq!(
        parameter(&body, "display").unwrap()["valueString"],
        "Kat",
        "French is not carried, Dutch is the next by quality"
    );
    let (_, body) = server
        .get_with_header(&uri, "Accept-Language", "en, nl;q=0.4")
        .await;
    assert_eq!(parameter(&body, "display").unwrap()["valueString"], "Cat");
    let (_, body) = server.get_with_header(&uri, "Accept-Language", "*").await;
    assert_eq!(
        parameter(&body, "display").unwrap()["valueString"],
        "Cat",
        "any language is the system's own"
    );
    let (status, body) = server
        .post_with_header(
            "/r4b/CodeSystem/$validate-code",
            &parameters(&[
                ("url", json!({"valueUri": SCT})),
                ("code", json!({"valueCode": cat})),
                ("display", json!({"valueString": "Cat"})),
            ]),
            "Accept-Language",
            "nl",
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        parameter(&body, "display").unwrap()["valueString"],
        "Kat",
        "the header picks the display returned on POST too"
    );
    let (status, body) = server
        .get_with_header(
            &format!(
                "/r4b/CodeSystem/$subsumes?system={SCT}&codeA={}&codeB={cat}",
                sctid(item(ANIMAL))
            ),
            "Accept-Language",
            "nl",
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an operation without displayLanguage ignores the header"
    );
    assert_eq!(
        parameter(&body, "outcome").unwrap()["valueCode"],
        "subsumes"
    );
}
