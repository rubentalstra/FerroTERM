//! The terminology ecosystem overlay on the wire: every served version accepts
//! the pre-adopted and ecosystem-defined parameters
//! (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>), refuses
//! the ones whose semantics are not implemented yet with `not-supported`, and
//! documents the overlay in its `CapabilityStatement`.

use ferroterm_testkit::fhir::{ANIMALS, CM_ANIMALS_COLOURS, VS_PETS};
use http::StatusCode;
use serde_json::Value;

use crate::fixture::{Server, parameter};

const VERSIONS: [&str; 3] = ["r4", "r4b", "r5"];

#[tokio::test]
async fn an_unimplemented_overlay_input_is_refused_as_not_supported_not_as_undeclared() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        for name in [
            "system-version",
            "check-system-version",
            "force-system-version",
            "default-valueset-version",
            "check-valueset-version",
            "force-valueset-version",
        ] {
            let (status, body) = server
                .get(&format!(
                    "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&{name}=http://example.org/x|1"
                ))
                .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{version} {name}: {body}");
            assert_eq!(
                body["issue"][0]["code"], "not-supported",
                "{version} {name}: {body}"
            );
            let text = body["issue"][0]["details"]["text"].as_str().unwrap();
            assert!(text.contains(name), "{version}: {text}");
        }
        for name in ["inferSystem", "lenient-display-validation"] {
            let (status, body) = server
                .get(&format!(
                    "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&{name}=true"
                ))
                .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{version} {name}: {body}");
            assert_eq!(
                body["issue"][0]["code"], "not-supported",
                "{version} {name}: {body}"
            );
            // `false` asks for the default behaviour and is accepted.
            let (status, body) = server
                .get(&format!(
                    "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&{name}=false"
                ))
                .await;
            assert_eq!(status, StatusCode::OK, "{version} {name}: {body}");
            assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
        }
    }
}

#[tokio::test]
async fn translate_accepts_the_r6_source_system_and_source_version_names() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        let code = if version == "r5" {
            "sourceCode"
        } else {
            "code"
        };
        let (status, body) = server
            .get(&format!(
                "/{version}/ConceptMap/$translate?url={CM_ANIMALS_COLOURS}&sourceSystem={ANIMALS}&{code}=cat"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            true,
            "{version}: {body}"
        );
    }
}

#[tokio::test]
async fn the_capability_statement_documents_the_overlay_per_operation() {
    let server = Server::start();
    for version in VERSIONS {
        let (status, body) = server.get(&format!("/{version}/metadata")).await;
        assert_eq!(status, StatusCode::OK);
        let operations: Vec<&Value> = body["rest"][0]["resource"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r["operation"].as_array().unwrap())
            .collect();
        let documentation = |name: &str| -> String {
            operations
                .iter()
                .find(|o| o["name"] == name)
                .and_then(|o| o["documentation"].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let validate = documentation("validate-code");
        assert!(
            validate.contains("`x-caused-by-unknown-system`"),
            "{version}: {validate}"
        );
        assert!(
            validate.contains("https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html"),
            "{version}: {validate}"
        );
        let lookup = documentation("lookup");
        assert!(lookup.contains("`abstract`"), "{version}: {lookup}");
        assert!(
            documentation("subsumes").is_empty(),
            "{version}: the overlay adds nothing to $subsumes"
        );
        let value_set_validate = operations
            .iter()
            .filter(|o| o["name"] == "validate-code")
            .nth(1)
            .and_then(|o| o["documentation"].as_str())
            .unwrap_or_default();
        if version == "r5" {
            assert!(
                !value_set_validate.contains("`issues`"),
                "{version} declares issues itself: {value_set_validate}"
            );
        } else {
            assert!(
                value_set_validate.contains("`issues`"),
                "{version}: {value_set_validate}"
            );
        }
        assert!(
            value_set_validate.contains("`check-system-version`"),
            "{version}: {value_set_validate}"
        );
    }
}

#[tokio::test]
async fn every_version_answers_the_validated_code_system_version_and_issues() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], true);
        assert_eq!(
            parameter(&body, "code").unwrap()["valueCode"],
            "kitten",
            "{version}"
        );
        assert_eq!(
            parameter(&body, "system").unwrap()["valueUri"],
            ANIMALS,
            "{version}"
        );
        assert_eq!(
            parameter(&body, "version").unwrap()["valueString"],
            "2.0",
            "{version}"
        );
        assert!(
            parameter(&body, "issues").is_none(),
            "{version}: no issues on a clean pass"
        );
        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$validate-code?url={ANIMALS}&code=unicorn"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(parameter(&body, "result").unwrap()["valueBoolean"], false);
        assert_eq!(
            parameter(&body, "code").unwrap()["valueCode"],
            "unicorn",
            "{version}"
        );
        let issues = parameter(&body, "issues").expect("issues");
        assert_eq!(issues["resource"]["resourceType"], "OperationOutcome");
        assert_eq!(
            issues["resource"]["issue"][0]["details"]["coding"][0]["code"], "invalid-code",
            "{version}"
        );
    }
}

#[tokio::test]
async fn an_unknown_system_is_a_false_result_naming_the_system_on_every_version() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        for path in [
            format!("/{version}/CodeSystem/$validate-code?url=http://example.org/none&code=x"),
            format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system=http://example.org/none&code=x"
            ),
        ] {
            let (status, body) = server.get(&path).await;
            assert_eq!(status, StatusCode::OK, "{path}: {body}");
            assert_eq!(
                parameter(&body, "result").unwrap()["valueBoolean"],
                false,
                "{path}"
            );
            assert_eq!(
                parameter(&body, "x-caused-by-unknown-system").unwrap()["valueCanonical"],
                "http://example.org/none",
                "{path}: {body}"
            );
            assert_eq!(
                parameter(&body, "code").unwrap()["valueCode"],
                "x",
                "{path}"
            );
            let issue = &parameter(&body, "issues").expect("issues")["resource"]["issue"][0];
            assert_eq!(issue["code"], "not-found", "{path}");
            assert_eq!(issue["details"]["coding"][0]["code"], "not-found", "{path}");
        }
        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$validate-code?url={ANIMALS}&version=9.9&code=cat"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "x-caused-by-unknown-system").unwrap()["valueCanonical"],
            format!("{ANIMALS}|9.9"),
            "{version}: an unknown version names url|version"
        );
    }
}
