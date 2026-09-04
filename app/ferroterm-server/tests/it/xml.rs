//! FHIR XML on the wire: `_format`, `Accept`, and `Content-Type` choose the
//! format on every route (<https://hl7.org/fhir/R4B/http.html#mime-type>).

use ferroterm_testkit::fhir::{ANIMALS, VS_ALL, VS_PETS};
use fhir_types::codec::Object;
use fhir_types::xml::{Schemas, from_xml, to_xml};
use http::StatusCode;
use serde_json::{Value, json};

use crate::fixture::Server;

const XML: &str = "application/fhir+xml; charset=utf-8";
const JSON: &str = "application/fhir+json; charset=utf-8";

fn parsed(schemas: &Schemas, body: &str) -> Object {
    from_xml(schemas, body).expect("well-formed FHIR XML")
}

fn parameter<'a>(object: &'a Object, name: &str) -> Option<&'a Value> {
    object
        .get("parameter")?
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)
}

#[tokio::test]
async fn format_and_accept_select_xml_on_every_route() {
    let server = Server::start_with_resources();
    let r4b = &fhir_types::r4b::schema::SCHEMAS;
    // `_format=xml` on an operation.
    let (status, content_type, body) = server
        .get_text(
            &format!("/r4b/CodeSystem/$lookup?system={ANIMALS}&code=cat&_format=xml"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    let object = parsed(r4b, &body);
    assert_eq!(object["resourceType"], "Parameters");
    assert_eq!(
        parameter(&object, "display").expect("display")["valueString"],
        "Cat"
    );
    // `Accept` on the capability statement, which lists both formats.
    let (status, content_type, body) = server
        .get_text("/r5/metadata", Some("application/fhir+xml"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    let object = parsed(&fhir_types::r5::schema::SCHEMAS, &body);
    assert_eq!(object["resourceType"], "CapabilityStatement");
    assert_eq!(
        object["format"],
        json!([
            "application/fhir+json",
            "json",
            "application/fhir+xml",
            "xml"
        ])
    );
    // The full media type in `_format`, on the value set search and read.
    let (status, content_type, body) = server
        .get_text(
            &format!("/r4/ValueSet?url={VS_ALL}&_format=application/fhir%2Bxml"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    let bundle = parsed(&fhir_types::r4::schema::SCHEMAS, &body);
    assert_eq!(bundle["resourceType"], "Bundle");
    assert_eq!(bundle["entry"][0]["resource"]["resourceType"], "ValueSet");
    // `$versions` and `$expand` too.
    let (status, content_type, body) = server.get_text("/r6/$versions?_format=xml", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    assert_eq!(
        parameter(&parsed(&fhir_types::r6::schema::SCHEMAS, &body), "version").expect("version")["valueCode"],
        "6.0"
    );
    let (status, content_type, body) = server
        .get_text(
            &format!("/r5/ValueSet/$expand?url={VS_PETS}"),
            Some("text/xml"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    let expansion = parsed(&fhir_types::r5::schema::SCHEMAS, &body);
    assert_eq!(expansion["resourceType"], "ValueSet");
    assert!(
        expansion["expansion"]["contains"]
            .as_array()
            .is_some_and(|c| !c.is_empty())
    );
    // JSON stays the default, and `*/*` means JSON.
    let (status, content_type, _) = server
        .get_text(
            &format!("/r4b/CodeSystem/$lookup?system={ANIMALS}&code=cat"),
            Some("*/*"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, JSON);
}

#[tokio::test]
async fn failures_and_unknown_formats_answer_in_the_negotiated_format() {
    let server = Server::start_with_resources();
    // A failure in XML, when asked for.
    let (status, content_type, body) = server
        .get_text(
            "/r4b/CodeSystem/$lookup?system=http://example.org/nowhere&code=cat&_format=xml",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(content_type, XML);
    let outcome = parsed(&fhir_types::r4b::schema::SCHEMAS, &body);
    assert_eq!(outcome["resourceType"], "OperationOutcome");
    assert_eq!(outcome["issue"][0]["code"], "not-found");
    // An unknown route, by `Accept`.
    let (status, content_type, body) = server
        .get_text("/r4b/Nothing", Some("application/xml"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, XML, "{body}");
    // A `_format` the server does not speak.
    let (status, content_type, body) = server
        .get_text(
            &format!("/r4b/CodeSystem/$lookup?system={ANIMALS}&code=cat&_format=turtle"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_ACCEPTABLE, "{body}");
    assert_eq!(content_type, JSON);
}

#[tokio::test]
async fn an_xml_parameters_body_is_accepted_and_a_malformed_one_refused() {
    let server = Server::start_with_resources();
    let r4b = &fhir_types::r4b::schema::SCHEMAS;
    let request = json!({"resourceType": "Parameters", "parameter": [
        {"name": "url", "valueUri": VS_PETS},
        {"name": "coding", "valueCoding": {"system": ANIMALS, "code": "kitten", "display": "Kitten"}}
    ]});
    let xml = to_xml(r4b, request.as_object().expect("object")).expect("XML");
    // XML in, JSON out by default.
    let (status, content_type, body) = server
        .post_text(
            "/r4b/ValueSet/$validate-code",
            "application/fhir+xml",
            &xml,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, JSON);
    let answer: Value = serde_json::from_str(&body).expect("JSON");
    assert_eq!(
        crate::fixture::parameter(&answer, "result").expect("result")["valueBoolean"],
        true
    );
    // XML in, XML out when asked; the R5 endpoint reads it through its own schema.
    let (status, content_type, body) = server
        .post_text(
            "/r5/ValueSet/$validate-code?_format=xml",
            "application/fhir+xml; charset=utf-8",
            &xml,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(content_type, XML);
    let answer = parsed(&fhir_types::r5::schema::SCHEMAS, &body);
    assert_eq!(
        parameter(&answer, "result").expect("result")["valueBoolean"],
        true
    );
    // Malformed XML: a `400` in the format `Accept` names.
    let (status, content_type, body) = server
        .post_text(
            "/r4b/ValueSet/$validate-code",
            "application/fhir+xml",
            "<Parameters xmlns=\"http://hl7.org/fhir\"><parameter>",
            Some("application/fhir+xml"),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(content_type, XML);
    let outcome = parsed(r4b, &body);
    assert_eq!(outcome["issue"][0]["code"], "structure");
    // A body of another media type is still refused.
    let (status, _, body) = server
        .post_text("/r4b/ValueSet/$validate-code", "text/plain", "hello", None)
        .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{body}");
}
