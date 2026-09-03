//! The terminology ecosystem overlay on the wire: every served version accepts
//! the pre-adopted and ecosystem-defined parameters
//! (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>), refuses
//! the ones whose semantics are not implemented yet with `not-supported`, and
//! documents the overlay in its `CapabilityStatement`.

use ferroterm_testkit::fhir::{
    ANIMALS, CM_ANIMALS_COLOURS, COLOURS, VS_ALL, VS_ENUMERATED, VS_PETS, VS_PETS_REF,
};
use http::StatusCode;
use serde_json::Value;

use crate::fixture::{Server, parameter};

const VERSIONS: [&str; 3] = ["r4", "r4b", "r5"];

#[tokio::test]
async fn the_version_negotiation_parameters_are_accepted_on_every_version() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        // A default for a system the value set does not pin, and a matching check.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&system-version={ANIMALS}|2.0&check-system-version={ANIMALS}|2.0"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            true,
            "{version}"
        );
        assert_eq!(
            parameter(&body, "version").unwrap()["valueString"],
            "2.0",
            "{version}"
        );
        // A forced version the server does not serve is an unknown version, named.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&force-system-version={ANIMALS}|9.9"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            false,
            "{version}"
        );
        assert_eq!(
            parameter(&body, "x-caused-by-unknown-system").unwrap()["valueCanonical"],
            format!("{ANIMALS}|9.9"),
            "{version}"
        );
        // A check that disagrees with the reference is refused.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_ALL}&valueSetVersion=2.0&system={ANIMALS}&code=cat&check-valueset-version={VS_ALL}|1.0"
            ))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{version}: {body}");
        assert_eq!(body["issue"][0]["code"], "invalid", "{version}: {body}");
        // $expand pins an imported value set by default-valueset-version and says so.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$expand?url={VS_PETS_REF}&default-valueset-version={VS_PETS}|1.0"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        let names: Vec<(String, String)> = body["expansion"]["parameter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["name"].as_str().unwrap().to_owned(),
                    p["valueUri"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        let used = (String::from("used-valueset"), format!("{VS_PETS}|1.0"));
        let echoed = (
            String::from("default-valueset-version"),
            format!("{VS_PETS}|1.0"),
        );
        assert!(names.contains(&used), "{version}: {names:?}");
        assert!(names.contains(&echoed), "{version}: {names:?}");
    }
}

#[tokio::test]
async fn infer_system_and_lenient_display_validation_apply_on_every_version() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        // A bare code finds its system among the two the enumerated value set draws on.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_ENUMERATED}&code=RED&inferSystem=true"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            true,
            "{version}"
        );
        assert_eq!(
            parameter(&body, "system").unwrap()["valueUri"],
            COLOURS,
            "{version}"
        );
        // A bare code in neither system cannot be inferred: not in the value set, and the
        // system undetermined.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_ENUMERATED}&code=dodo&inferSystem=true"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            false,
            "{version}"
        );
        let kinds: Vec<&str> = parameter(&body, "issues").unwrap()["resource"]["issue"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["details"]["coding"][0]["code"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["not-in-vs", "cannot-infer"], "{version}");
        // A wrong display is a warning under lenient-display-validation.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&display=Puppy&lenient-display-validation=true"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            true,
            "{version}"
        );
        let issue = &parameter(&body, "issues").unwrap()["resource"]["issue"][0];
        assert_eq!(issue["severity"], "warning", "{version}");
        assert_eq!(
            issue["details"]["coding"][0]["code"], "invalid-display",
            "{version}"
        );
        // Without it the same display fails the validation.
        let (status, body) = server
            .get(&format!(
                "/{version}/ValueSet/$validate-code?url={VS_PETS}&system={ANIMALS}&code=kitten&display=Puppy"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "result").unwrap()["valueBoolean"],
            false,
            "{version}"
        );
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

#[tokio::test]
async fn lookup_answers_code_system_and_abstract_on_every_version() {
    let server = Server::start_with_resources();
    for version in VERSIONS {
        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$lookup?system={ANIMALS}&code=living"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "code").unwrap()["valueCode"],
            "living",
            "{version}"
        );
        assert_eq!(
            parameter(&body, "system").unwrap()["valueUri"],
            ANIMALS,
            "{version}"
        );
        assert_eq!(
            parameter(&body, "abstract").unwrap()["valueBoolean"],
            true,
            "{version}"
        );
        let (status, body) = server
            .get(&format!(
                "/{version}/CodeSystem/$lookup?system={ANIMALS}&code=cat"
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{version}: {body}");
        assert_eq!(
            parameter(&body, "abstract").unwrap()["valueBoolean"],
            false,
            "{version}"
        );
    }
}
