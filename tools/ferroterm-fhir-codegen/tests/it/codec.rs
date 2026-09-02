use std::fs;
use std::path::{Path, PathBuf};

use ferroterm_fhir::codec::{DecodeErrorKind, Json, Path as ElementPath, expect_object};
use serde_json::Value;

use crate::vendor_dir;

/// The resource files of a package whose type is in the root set.
fn root_set_files(package: &str) -> Vec<PathBuf> {
    let dir = vendor_dir().join(package).join("package");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("package dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                && [
                    "ValueSet-",
                    "CodeSystem-",
                    "ConceptMap-",
                    "CapabilityStatement-",
                    "TerminologyCapabilities-",
                    "OperationOutcome-",
                    "Parameters-",
                    "Bundle-",
                ]
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .collect();
    files.sort();
    files
}

fn round_trip<T: Json>(path: &Path, root: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let original: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut element_path = ElementPath::root(root);
    let object = expect_object(&original, &element_path).map_err(|e| e.to_string())?;
    let decoded =
        T::from_json(object, &mut element_path).map_err(|e| format!("{}: {e}", path.display()))?;
    let encoded = Value::Object(decoded.to_json().map_err(|e| e.to_string())?);
    if encoded != original {
        return Err(format!("round trip differs for {}", path.display()));
    }
    Ok(())
}

/// Files the package publishes in violation of its own definitions, each with the
/// error the strict codec must report. CodeSystem.status and ValueSet.status are
/// 1..1 (<https://hl7.org/fhir/R4B/codesystem.html>, <https://hl7.org/fhir/R4B/valueset.html>),
/// and the 4.3.0 package omits both on catalogType.
fn expected_rejections(package: &str) -> Vec<(&'static str, &'static str)> {
    match package {
        "hl7.fhir.r4b.core" => vec![
            (
                "CodeSystem-catalogType.json",
                "Resource.status: required property is missing",
            ),
            (
                "ValueSet-catalogType.json",
                "Resource.status: required property is missing",
            ),
        ],
        _ => Vec::new(),
    }
}

fn round_trip_all<T: Json>(package: &str) {
    let files = root_set_files(package);
    assert!(files.len() > 1300, "{package}: {} files", files.len());
    let rejections = expected_rejections(package);
    let mut failures = Vec::new();
    let mut rejected = 0;
    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match (
            round_trip::<T>(file, "Resource"),
            rejections.iter().find(|(f, _)| *f == name),
        ) {
            (Ok(()), None) => {}
            (Ok(()), Some(_)) => {
                failures.push(format!("{name}: expected a rejection, round-tripped"));
            }
            (Err(e), Some((_, expected))) if e.ends_with(expected) => rejected += 1,
            (Err(e), _) => failures.push(e),
        }
    }
    assert_eq!(
        rejected,
        rejections.len(),
        "{package}: every expected rejection happened"
    );
    assert!(
        failures.is_empty(),
        "{package}: {} of {} files failed:\n{}",
        failures.len(),
        files.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_r4_root_set_resource_round_trips() {
    round_trip_all::<ferroterm_fhir::r4::resource::Resource>("hl7.fhir.r4.core");
}

#[test]
fn every_r4b_root_set_resource_round_trips() {
    round_trip_all::<ferroterm_fhir::r4b::resource::Resource>("hl7.fhir.r4b.core");
}

#[test]
fn every_r5_root_set_resource_round_trips() {
    round_trip_all::<ferroterm_fhir::r5::resource::Resource>("hl7.fhir.r5.core");
}

fn decode_coding(
    text: &str,
) -> Result<ferroterm_fhir::r4b::coding::Coding, ferroterm_fhir::codec::DecodeError> {
    let value: Value = serde_json::from_str(text).expect("json");
    let mut path = ElementPath::root("Coding");
    let object = expect_object(&value, &path).expect("object");
    ferroterm_fhir::r4b::coding::Coding::from_json(object, &mut path)
}

#[test]
fn unknown_properties_and_wrong_shapes_are_refused() {
    let unknown = decode_coding(r#"{"code":"a","bogus":1}"#).expect_err("unknown property");
    assert_eq!(unknown.kind, DecodeErrorKind::UnknownProperty);
    assert_eq!(unknown.path, "Coding.bogus");
    let wrong = decode_coding(r#"{"code":["a"]}"#).expect_err("array for a single element");
    assert_eq!(wrong.kind, DecodeErrorKind::WrongCardinality);
    let typed = decode_coding(r#"{"userSelected":"yes"}"#).expect_err("string for a boolean");
    assert_eq!(
        typed.kind,
        DecodeErrorKind::WrongType {
            expected: "a boolean"
        }
    );
    let empty = decode_coding(r#"{"code":"a","extension":[]}"#).expect_err("empty array");
    assert_eq!(empty.kind, DecodeErrorKind::Empty);
}

#[test]
fn primitive_extensions_and_choice_types_round_trip() {
    // A primitive extension pair and choice elements
    // (https://hl7.org/fhir/R4B/json.html#primitive).
    let text = r#"{"resourceType":"Parameters","parameter":[{"name":"x","valueString":"hello","_valueString":{"id":"v1","extension":[{"url":"http://example.org/e","valueBoolean":true}]}},{"name":"y","valueDecimal":2.50},{"name":"z","valueCoding":{"system":"http://s","code":"c"}}]}"#;
    let original: Value = serde_json::from_str(text).expect("json");
    let mut path = ElementPath::root("Parameters");
    let object = expect_object(&original, &path).expect("object");
    let decoded =
        ferroterm_fhir::r4b::parameters::Parameters::from_json(object, &mut path).expect("decodes");
    let encoded = Value::Object(decoded.to_json().expect("encodes"));
    assert_eq!(encoded, original);
    assert!(
        encoded.to_string().contains("2.50"),
        "decimal precision survives"
    );
    let duplicate = r#"{"resourceType":"Parameters","parameter":[{"name":"x","valueString":"a","valueCode":"b"}]}"#;
    let value: Value = serde_json::from_str(duplicate).expect("json");
    let mut path = ElementPath::root("Parameters");
    let err = ferroterm_fhir::r4b::parameters::Parameters::from_json(
        expect_object(&value, &path).expect("object"),
        &mut path,
    )
    .expect_err("two choice forms");
    assert_eq!(err.kind, DecodeErrorKind::DuplicateChoice);
}

#[test]
fn serde_bridges_agree_with_the_codec() {
    let text = r#"{"resourceType":"ValueSet","status":"active","compose":{"include":[{"system":"http://snomed.info/sct","filter":[{"property":"concept","op":"is-a","value":"123"}]}]}}"#;
    let value_set: ferroterm_fhir::r4b::value_set::ValueSet =
        serde_json::from_str(text).expect("deserializes");
    let back = serde_json::to_value(&value_set).expect("serializes");
    let original: Value = serde_json::from_str(text).expect("json");
    assert_eq!(back, original);
    let unknown: Result<ferroterm_fhir::r4b::value_set::ValueSet, _> =
        serde_json::from_str(r#"{"resourceType":"ValueSet","status":"active","nope":1}"#);
    assert!(unknown.is_err());
}
