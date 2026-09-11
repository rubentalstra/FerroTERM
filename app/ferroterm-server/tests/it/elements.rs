//! `_elements` on the wire: the subset a client asked for, on every version.
//!
//! The unit tests in `ferroterm_server::elements` pin the projection itself.
//! These pin what a client sees: that the parameter is answered rather than
//! refused, that the answer says it is a subset, and that a search still
//! matches the same resources when it narrows what they carry.

use http::StatusCode;
use serde_json::Value;

use crate::fixture::Server;

/// The system of the tag a subsetted resource carries.
const TAG_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-ObservationValue";

/// The elements every entry of `body` carries, as one sorted list.
fn elements_of(body: &Value) -> Vec<String> {
    let Some(entries) = body["entry"].as_array() else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry["resource"].as_object())
        .flat_map(|resource| resource.keys().cloned())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Whether every entry of `body` is marked as a subset.
fn every_entry_is_subsetted(body: &Value) -> bool {
    let Some(entries) = body.get("entry").and_then(Value::as_array) else {
        return false;
    };
    !entries.is_empty()
        && entries.iter().all(|entry| {
            entry
                .pointer("/resource/meta/tag")
                .and_then(Value::as_array)
                .is_some_and(|tags| {
                    tags.iter().any(|tag| {
                        tag.get("system").and_then(Value::as_str) == Some(TAG_SYSTEM)
                            && tag.get("code").and_then(Value::as_str) == Some("SUBSETTED")
                    })
                })
        })
}

#[tokio::test]
async fn a_search_returns_the_elements_it_was_asked_for_on_every_served_version() {
    let server = Server::start_with_every_loader();
    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server
            .get(&format!("/{version}/CodeSystem?_elements=url,title"))
            .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{version} refuses a parameter its own release defines: {body}"
        );
        let names = elements_of(&body);
        assert!(
            !names.is_empty(),
            "{version} answered a search with no resources in it: {body}"
        );
        for name in &names {
            assert!(
                ["id", "meta", "resourceType", "title", "url"].contains(&name.as_str()),
                "{version} returned `{name}`, which the search did not ask for"
            );
        }
        assert!(
            names.contains(&"url".to_owned()),
            "{version} left out an element the search did ask for: {names:?}"
        );
    }
}

#[tokio::test]
async fn a_subsetted_resource_says_that_it_is_one() {
    let server = Server::start_with_every_loader();
    let (status, body) = server.get("/r5/CodeSystem?_elements=url").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        every_entry_is_subsetted(&body),
        "a client must not mistake a subset for the whole resource: {body}"
    );
}

#[tokio::test]
async fn the_mandatory_elements_come_back_whether_they_were_asked_for_or_not() {
    let server = Server::start_with_every_loader();
    let (status, body) = server.get("/r5/CodeSystem?_elements=title").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let names = elements_of(&body);
    for mandatory in ["resourceType", "id"] {
        assert!(
            names.contains(&mandatory.to_owned()),
            "the specification says a server returns mandatory elements whether they are requested or not, and `{mandatory}` is missing: {names:?}"
        );
    }
}

#[tokio::test]
async fn narrowing_what_a_resource_carries_does_not_change_what_the_search_matched() {
    let server = Server::start_with_every_loader();
    let (_, whole) = server.get("/r5/CodeSystem?url=http://loinc.org").await;
    let (status, subset) = server
        .get("/r5/CodeSystem?url=http://loinc.org&_elements=url")
        .await;
    assert_eq!(status, StatusCode::OK, "{subset}");
    assert_eq!(
        subset["total"], whole["total"],
        "_elements says what to return, not what to match"
    );
    assert_eq!(
        subset["entry"][0]["fullUrl"], whole["entry"][0]["fullUrl"],
        "the entry still names the resource it matched"
    );
}

#[tokio::test]
async fn an_element_no_resource_carries_is_not_an_error() {
    let server = Server::start_with_every_loader();
    let (status, body) = server
        .get("/r5/CodeSystem?_elements=url,neverAnElementOfCodeSystem")
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an element a resource does not have is absent, not a refusal: {body}"
    );
    let names = elements_of(&body);
    assert!(names.contains(&"url".to_owned()), "{names:?}");
    assert!(!names.contains(&"neverAnElementOfCodeSystem".to_owned()));
}

#[tokio::test]
async fn a_search_naming_no_elements_is_the_search_it_always_was() {
    let server = Server::start_with_every_loader();
    let (_, whole) = server.get("/r5/CodeSystem?url=http://loinc.org").await;
    let (status, empty) = server
        .get("/r5/CodeSystem?url=http://loinc.org&_elements=")
        .await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(
        empty, whole,
        "a client that named nothing asked for everything"
    );
}

#[tokio::test]
async fn the_value_set_and_concept_map_searches_answer_it_too() {
    let server = Server::start_with_every_loader();
    for resource in ["ValueSet", "ConceptMap"] {
        let (status, body) = server.get(&format!("/r5/{resource}?_elements=url")).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{resource} refuses a parameter every resource type defines: {body}"
        );
    }
}

#[tokio::test]
async fn the_capability_statement_declares_the_parameter_it_answers() {
    let server = Server::start_with_every_loader();
    for version in ["r4", "r4b", "r5", "r6"] {
        let (status, body) = server.get(&format!("/{version}/metadata")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let declared = body["rest"][0]["resource"]
            .as_array()
            .expect("a rest entry lists its resource types")
            .iter()
            .filter(|resource| resource["type"] == "CodeSystem")
            .flat_map(|resource| {
                resource["searchParam"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .any(|param| {
                param["name"] == "_elements"
                    && param["definition"]
                        == "http://hl7.org/fhir/SearchParameter/Resource-elements"
            });
        assert!(
            declared,
            "{version} answers _elements without declaring it, so a client cannot discover it"
        );
    }
}

#[tokio::test]
async fn a_parameter_no_version_defines_is_still_refused() {
    let server = Server::start_with_every_loader();
    let (status, body) = server.get("/r5/CodeSystem?_summaryish=true").await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "accepting _elements does not loosen the rest: {body}"
    );
    assert_eq!(body["issue"][0]["code"], "not-supported");
}
