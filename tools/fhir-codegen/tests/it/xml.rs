//! The XML codec: every root-set resource round-trips JSON to XML to JSON,
//! refusals name their element, and a few examples are pinned as golden XML
//! (<https://hl7.org/fhir/R4B/xml.html>).

use std::fs;
use std::path::PathBuf;

use fhir_types::codec::{DecodeErrorKind, Json, expect_object};
use fhir_types::xml::{Schemas, from_xml, to_xml};
use proptest::prelude::*;
use serde_json::Value;

use crate::vendor_dir;

fn root_set_files(package: &str) -> Vec<PathBuf> {
    let dir = vendor_dir().join(package).join("package");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("package dir")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("");
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

/// The canonical JSON of a file the JSON codec accepts, `None` when it refuses it.
fn canonical<T: Json>(path: &std::path::Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    let original: Value = serde_json::from_str(&text).ok()?;
    let mut element_path = fhir_types::codec::Path::root("Resource");
    let object = expect_object(&original, &element_path).ok()?;
    let decoded = T::from_json(object, &mut element_path).ok()?;
    Some(Value::Object(decoded.to_json().ok()?))
}

/// JSON to XML to JSON, equal to the canonical JSON.
fn round_trip(schemas: &Schemas, canonical: &Value, name: &str) -> Result<(), String> {
    let object = canonical.as_object().ok_or("not an object")?;
    let xml = to_xml(schemas, object).map_err(|e| format!("{name}: to XML: {e}"))?;
    let back = from_xml(schemas, &xml).map_err(|e| format!("{name}: from XML: {e}"))?;
    if Value::Object(back) != *canonical {
        return Err(format!("{name}: the XML round trip differs"));
    }
    Ok(())
}

/// Every root-set file the JSON codec accepts round-trips through XML, except
/// a Bundle that carries a resource outside the root set, which has no schema
/// (the XML codec covers the same closure the typed model does).
fn round_trip_all<T: Json>(package: &str, schemas: &Schemas) {
    let files = root_set_files(package);
    assert!(files.len() > 1000, "{package}: {} files", files.len());
    let mut failures = Vec::new();
    let mut passed = 0;
    let mut unknown_resources = 0;
    for file in &files {
        let name = file
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("");
        let Some(canonical) = canonical::<T>(file) else {
            continue;
        };
        match round_trip(schemas, &canonical, name) {
            Ok(()) => passed += 1,
            Err(e) if e.contains("no schema for the resource type") => unknown_resources += 1,
            Err(e) => failures.push(e),
        }
    }
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
    assert!(passed > 1000, "{package}: {passed} round trips");
    assert!(
        unknown_resources < 20,
        "{package}: {unknown_resources} bundles carry resources outside the root set"
    );
}

#[test]
fn every_r4_root_set_resource_round_trips_through_xml() {
    round_trip_all::<fhir_types::r4::resource::Resource>(
        "hl7.fhir.r4.core",
        &fhir_types::r4::schema::SCHEMAS,
    );
}

#[test]
fn every_r4b_root_set_resource_round_trips_through_xml() {
    round_trip_all::<fhir_types::r4b::resource::Resource>(
        "hl7.fhir.r4b.core",
        &fhir_types::r4b::schema::SCHEMAS,
    );
}

#[test]
fn every_r5_root_set_resource_round_trips_through_xml() {
    round_trip_all::<fhir_types::r5::resource::Resource>(
        "hl7.fhir.r5.core",
        &fhir_types::r5::schema::SCHEMAS,
    );
}

#[test]
fn every_r6_root_set_resource_round_trips_through_xml() {
    round_trip_all::<fhir_types::r6::resource::Resource>(
        "hl7.fhir.r6.core",
        &fhir_types::r6::schema::SCHEMAS,
    );
}

#[test]
fn the_xml_of_examples_is_pinned() {
    let schemas = &fhir_types::r4b::schema::SCHEMAS;
    let first_concept_map = root_set_files("hl7.fhir.r4b.core")
        .into_iter()
        .find(|p| {
            p.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|n| n.starts_with("ConceptMap-"))
        })
        .and_then(|p| {
            p.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .map(str::to_owned)
        })
        .expect("a ConceptMap example");
    for file in [
        "ValueSet-administrative-gender.json",
        "CodeSystem-administrative-gender.json",
        first_concept_map.as_str(),
    ] {
        let path = vendor_dir()
            .join("hl7.fhir.r4b.core")
            .join("package")
            .join(file);
        let canonical =
            canonical::<fhir_types::r4b::resource::Resource>(&path).expect("the codec accepts it");
        let xml = to_xml(schemas, canonical.as_object().expect("object")).expect("XML");
        insta::assert_snapshot!(file, xml);
    }
}

#[test]
fn xml_refusals_name_the_element() {
    let schemas = &fhir_types::r5::schema::SCHEMAS;
    let refused = |xml: &str| from_xml(schemas, xml).expect_err("refused");
    let unknown =
        refused(r#"<Parameters xmlns="http://hl7.org/fhir"><foo value="1"/></Parameters>"#);
    assert_eq!(unknown.kind, DecodeErrorKind::UnknownElement);
    assert_eq!(unknown.path, "Resource.foo");
    let text = refused(
        r#"<Parameters xmlns="http://hl7.org/fhir"><parameter>text</parameter></Parameters>"#,
    );
    assert_eq!(text.kind, DecodeErrorKind::UnexpectedText);
    assert_eq!(text.path, "Resource.parameter");
    let root = refused(r#"<Coding xmlns="http://hl7.org/fhir"><code value="a"/></Coding>"#);
    assert_eq!(root.kind, DecodeErrorKind::WrongRoot);
    let boolean = refused(
        r#"<Parameters xmlns="http://hl7.org/fhir"><parameter><name value="x"/><valueBoolean value="yes"/></parameter></Parameters>"#,
    );
    assert_eq!(boolean.kind, DecodeErrorKind::BadValue);
    assert_eq!(boolean.path, "Resource.parameter.valueBoolean");
    let attribute = refused(r#"<Parameters xmlns="http://hl7.org/fhir" foo="1"/>"#);
    assert_eq!(attribute.kind, DecodeErrorKind::UnknownProperty);
    let malformed = refused(r#"<Parameters xmlns="http://hl7.org/fhir"><parameter>"#);
    assert!(matches!(
        malformed.kind,
        DecodeErrorKind::MalformedXml { .. }
    ));
    // Element ids, extensions on primitives, and a namespace prefix are read.
    let read = from_xml(
        schemas,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<f:Parameters xmlns:f="http://hl7.org/fhir"><!-- a comment -->
  <f:parameter f:id="p1"><f:name value="n"/><f:valueString id="s1" value="v"><f:extension url="http://example.org/x"><f:valueBoolean value="true"/></f:extension></f:valueString></f:parameter>
</f:Parameters>"#,
    )
    .expect("reads");
    assert_eq!(read["resourceType"], "Parameters");
    assert_eq!(read["parameter"][0]["id"], "p1");
    assert_eq!(read["parameter"][0]["valueString"], "v");
    assert_eq!(read["parameter"][0]["_valueString"]["id"], "s1");
    assert_eq!(
        read["parameter"][0]["_valueString"]["extension"][0]["url"],
        "http://example.org/x"
    );
}

fn coding(system: &str, code: &str, display: Option<String>) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("system".into(), Value::String(system.into()));
    object.insert("code".into(), Value::String(code.into()));
    if let Some(display) = display {
        object.insert("display".into(), Value::String(display));
    }
    Value::Object(object)
}

proptest! {
    /// A `Parameters` of every primitive kind, strings with markup, a nested
    /// coding, and a decimal's lexical form round-trips through XML.
    #[test]
    fn generated_parameters_round_trip_through_xml(
        text in "[a-zA-Z0-9 <>&\"'\u{e9}]{0,24}",
        flag in any::<bool>(),
        number in any::<i32>(),
        decimal in prop_oneof![Just("1.50"), Just("0.001"), Just("-3"), Just("100"), Just("2.0")],
        code in "[a-z][a-z0-9-]{0,10}",
        display in proptest::option::of("[A-Za-z ]{1,12}"),
    ) {
        let mut parameters = Vec::new();
        let named = |name: &str, key: &str, value: Value| {
            let mut object = serde_json::Map::new();
            object.insert("name".into(), Value::String(name.into()));
            object.insert(key.into(), value);
            Value::Object(object)
        };
        parameters.push(named("text", "valueString", Value::String(text)));
        parameters.push(named("flag", "valueBoolean", Value::Bool(flag)));
        parameters.push(named("number", "valueInteger", Value::from(number)));
        parameters.push(named(
            "decimal",
            "valueDecimal",
            Value::Number(decimal.parse().expect("a number")),
        ));
        parameters.push(named("coding", "valueCoding", coding("http://example.org/s", &code, display)));
        let mut object = serde_json::Map::new();
        object.insert("resourceType".into(), Value::String("Parameters".into()));
        object.insert("parameter".into(), Value::Array(parameters));
        let mut path = fhir_types::codec::Path::root("Parameters");
        let typed = fhir_types::r5::parameters::Parameters::from_json(&object, &mut path)
            .expect("the JSON codec accepts the generated resource");
        let canonical = Value::Object(typed.to_json().expect("encodes"));
        round_trip(&fhir_types::r5::schema::SCHEMAS, &canonical, "generated").expect("round trips");
    }
}
