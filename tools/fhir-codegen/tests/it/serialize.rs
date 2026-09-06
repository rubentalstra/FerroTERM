//! The two write paths agree byte for byte: the JSON document `Json::to_json`
//! builds, and the `Serialize` impl that writes the typed resource straight to
//! the serializer.
//!
//! FHIR JSON is normative (<https://hl7.org/fhir/R4B/json.html>), so the direct
//! path is correct only when it produces exactly the bytes the document path
//! produces, over every resource the operations answer with, per version.

use std::fs;
use std::path::Path;

use fhir_types::codec::{Json, Path as ElementPath, expect_object};
use proptest::prelude::*;
use serde_json::{Map, Value};

use crate::codec::root_set_files;

/// The two paths' bytes for one decoded resource.
fn both_paths<T: Json + serde::Serialize>(typed: &T) -> Result<(Vec<u8>, Vec<u8>), String> {
    let object = typed.to_json().map_err(|e| e.to_string())?;
    let document = serde_json::to_vec(&object).map_err(|e| e.to_string())?;
    let direct = serde_json::to_vec(typed).map_err(|e| e.to_string())?;
    Ok((document, direct))
}

/// Where two byte strings first differ, for a readable failure.
fn first_difference(left: &[u8], right: &[u8]) -> String {
    let at = left
        .iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| left.len().min(right.len()));
    let window = |bytes: &[u8]| {
        String::from_utf8_lossy(
            bytes
                .get(at.saturating_sub(40)..(at + 40).min(bytes.len()))
                .unwrap_or(b""),
        )
        .into_owned()
    };
    format!(
        "byte {at}: document `{}` versus direct `{}`",
        window(left),
        window(right)
    )
}

/// Both paths over one package's resources, skipping the files the codec
/// refuses (the package's own violations, pinned in `codec.rs`).
fn parity_over_package<T: Json + serde::Serialize>(package: &str) {
    let files = root_set_files(package);
    assert!(files.len() > 1000, "{package}: {} files", files.len());
    let mut compared = 0_usize;
    let mut failures = Vec::new();
    for file in &files {
        let Some(typed) = decode::<T>(file) else {
            continue;
        };
        match both_paths(&typed) {
            Ok((document, direct)) if document == direct => compared += 1,
            Ok((document, direct)) => failures.push(format!(
                "{}: {}",
                file.display(),
                first_difference(&document, &direct)
            )),
            Err(e) => failures.push(format!("{}: {e}", file.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{package}: {} of {} resources differ:\n{}",
        failures.len(),
        files.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        compared > 1000,
        "{package}: only {compared} resources compared"
    );
}

/// One resource file as the typed value, `None` when the strict codec refuses
/// it (those files are the package's own violations and have no typed form).
fn decode<T: Json>(file: &Path) -> Option<T> {
    let text = fs::read_to_string(file).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let mut path = ElementPath::root("Resource");
    let object = expect_object(&value, &path).ok()?;
    T::from_json(object, &mut path).ok()
}

#[test]
fn every_r4_root_set_resource_writes_the_same_bytes() {
    parity_over_package::<fhir_types::r4::resource::Resource>("hl7.fhir.r4.core");
}

#[test]
fn every_r4b_root_set_resource_writes_the_same_bytes() {
    parity_over_package::<fhir_types::r4b::resource::Resource>("hl7.fhir.r4b.core");
}

#[test]
fn every_r5_root_set_resource_writes_the_same_bytes() {
    parity_over_package::<fhir_types::r5::resource::Resource>("hl7.fhir.r5.core");
}

#[test]
fn every_r6_root_set_resource_writes_the_same_bytes() {
    parity_over_package::<fhir_types::r6::resource::Resource>("hl7.fhir.r6.core");
}

/// The two paths over one `Parameters` document, decoded in one version.
fn parity_of_parameters<T: Json + serde::Serialize>(
    object: &Map<String, Value>,
) -> Result<(), String> {
    let mut path = ElementPath::root("Parameters");
    let typed = T::from_json(object, &mut path).map_err(|e| e.to_string())?;
    let (document, direct) = both_paths(&typed)?;
    if document == direct {
        return Ok(());
    }
    Err(first_difference(&document, &direct))
}

/// The same document through every version's `Parameters`.
fn parity_of_every_version(object: &Map<String, Value>) -> Result<(), String> {
    parity_of_parameters::<fhir_types::r4::parameters::Parameters>(object)?;
    parity_of_parameters::<fhir_types::r4b::parameters::Parameters>(object)?;
    parity_of_parameters::<fhir_types::r5::parameters::Parameters>(object)?;
    parity_of_parameters::<fhir_types::r6::parameters::Parameters>(object)
}

/// A `parameter` entry: its name and one keyed value.
fn named(name: &str, key: &str, value: Value) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(name.into()));
    object.insert(key.into(), value);
    Value::Object(object)
}

/// The `Parameters` the property test writes, over the generated values.
fn parameters(
    entries: Vec<Value>,
    profiles: Vec<Value>,
    elements: Vec<Value>,
) -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("profile".into(), Value::Array(profiles));
    meta.insert("_profile".into(), Value::Array(elements));
    let mut object = Map::new();
    object.insert("resourceType".into(), Value::String("Parameters".into()));
    object.insert("meta".into(), Value::Object(meta));
    object.insert("parameter".into(), Value::Array(entries));
    object
}

proptest! {
    /// Every primitive kind, a primitive that carries only an extension, a
    /// repeating primitive with a hole, a nested resource, and a resource
    /// outside the root set write the same bytes both ways, in every version.
    #[test]
    fn the_two_write_paths_agree_on_a_generated_parameters(
        text in "[a-zA-Z0-9 <>&\"'\u{e9}]{0,24}",
        flag in any::<bool>(),
        number in any::<i32>(),
        decimal in prop_oneof![Just("1.50"), Just("0.001"), Just("-3"), Just("100"), Just("2.0e3")],
        code in "[a-z][a-z0-9-]{0,10}",
        identifier in "[a-z][a-z0-9-]{0,10}",
        extended in any::<bool>(),
    ) {
        let mut entries = vec![
            named("text", "valueString", Value::String(text)),
            named("flag", "valueBoolean", Value::Bool(flag)),
            named("number", "valueInteger", Value::from(number)),
            named("decimal", "valueDecimal", Value::Number(decimal.parse().expect("a number"))),
            named("instant", "valueInstant", Value::String("2026-09-06T00:00:00Z".into())),
        ];
        let mut coding = Map::new();
        coding.insert("system".into(), Value::String("http://example.org/s".into()));
        coding.insert("code".into(), Value::String(code.clone()));
        entries.push(named("coding", "valueCoding", Value::Object(coding)));
        // A primitive that carries an id and an extension, with and without a
        // value (https://hl7.org/fhir/R4B/json.html#primitive).
        let mut element = Map::new();
        element.insert("id".into(), Value::String(identifier.clone()));
        if extended {
            let mut extension = Map::new();
            extension.insert("url".into(), Value::String("http://example.org/x".into()));
            extension.insert("valueDecimal".into(), Value::Number("0.10".parse().expect("a number")));
            element.insert("extension".into(), Value::Array(vec![Value::Object(extension)]));
        }
        entries.push(named("element-only", "_valueString", Value::Object(element.clone())));
        let mut valued = Map::new();
        valued.insert("name".into(), Value::String("valued".into()));
        valued.insert("valueCode".into(), Value::String(code.clone()));
        valued.insert("_valueCode".into(), Value::Object(element));
        entries.push(Value::Object(valued));
        // A resource of the root set and one outside it, which the codec keeps
        // as an unknown resource.
        let mut system = Map::new();
        system.insert("resourceType".into(), Value::String("CodeSystem".into()));
        system.insert("status".into(), Value::String("active".into()));
        system.insert("content".into(), Value::String("not-present".into()));
        system.insert("url".into(), Value::String("http://example.org/cs".into()));
        entries.push(named("resource", "resource", Value::Object(system)));
        let mut outside = Map::new();
        outside.insert("resourceType".into(), Value::String("Practitioner".into()));
        outside.insert("id".into(), Value::String(identifier));
        entries.push(named("unknown", "resource", Value::Object(outside)));
        let profiles = vec![
            Value::String("http://example.org/a".into()),
            Value::String("http://example.org/b".into()),
        ];
        let mut hole = Map::new();
        hole.insert("id".into(), Value::String("p2".into()));
        let elements = vec![Value::Null, Value::Object(hole)];
        let object = parameters(entries, profiles, elements);
        prop_assert!(parity_of_every_version(&object).is_ok(), "{:?}", parity_of_every_version(&object));
    }
}

/// The document the ordering test pins, in the order a client writes it.
fn ordered_source() -> Map<String, Value> {
    let text = r#"{
        "resourceType": "Parameters",
        "meta": {
            "profile": ["http://example.org/a", "http://example.org/b"],
            "_profile": [null, {"id": "p2"}]
        },
        "parameter": [
            {
                "name": "text",
                "valueString": "hello",
                "_valueString": {
                    "id": "s1",
                    "extension": [{"url": "http://example.org/e", "valueBoolean": true}]
                }
            },
            {"name": "number", "valueDecimal": 2.50},
            {"name": "coded", "valueCoding": {"system": "http://example.org/s", "code": "c"}},
            {"name": "only-element", "_valueString": {"id": "e1"}}
        ]
    }"#;
    serde_json::from_str(text).expect("the source document parses")
}

/// The bytes both paths must write: keys in the order the JSON object holds
/// them, `_name` beside its value, a `null` only as a hole in a repeating
/// primitive, and an absent element with no key at all.
const ORDERED_BYTES: &str = concat!(
    r#"{"meta":{"_profile":[null,{"id":"p2"}],"#,
    r#""profile":["http://example.org/a","http://example.org/b"]},"#,
    r#""parameter":[{"_valueString":{"extension":[{"url":"http://example.org/e","valueBoolean":true}],"id":"s1"},"#,
    r#""name":"text","valueString":"hello"},"#,
    r#"{"name":"number","valueDecimal":2.50},"#,
    r#"{"name":"coded","valueCoding":{"code":"c","system":"http://example.org/s"}},"#,
    r#"{"_valueString":{"id":"e1"},"name":"only-element"}],"#,
    r#""resourceType":"Parameters"}"#
);

/// The direct path's bytes for `object`, decoded as `T`.
fn direct_bytes<T: Json + serde::Serialize>(object: &Map<String, Value>) -> String {
    let mut path = ElementPath::root("Parameters");
    let typed = T::from_json(object, &mut path).expect("the document decodes");
    serde_json::to_string(&typed).expect("the typed resource writes")
}

#[test]
fn the_direct_path_writes_the_ordered_bytes_in_every_version() {
    let object = ordered_source();
    assert_eq!(
        direct_bytes::<fhir_types::r4::parameters::Parameters>(&object),
        ORDERED_BYTES
    );
    assert_eq!(
        direct_bytes::<fhir_types::r4b::parameters::Parameters>(&object),
        ORDERED_BYTES
    );
    assert_eq!(
        direct_bytes::<fhir_types::r5::parameters::Parameters>(&object),
        ORDERED_BYTES
    );
    assert_eq!(
        direct_bytes::<fhir_types::r6::parameters::Parameters>(&object),
        ORDERED_BYTES
    );
}
