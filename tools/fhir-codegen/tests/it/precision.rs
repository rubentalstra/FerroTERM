//! A FHIR decimal keeps its lexical form, and a crate that depends on
//! `fhir-types` keeps its own JSON.
//!
//! FHIR forbids reading a decimal through a binary float ("Do not use an IEEE
//! type floating point type, instead use something that works like a true
//! decimal", <https://hl7.org/fhir/R4B/datatypes.html#decimal>), so a trailing
//! zero and a decimal past `f64` precision survive both write paths in every
//! served version. The codec reaches that without `serde_json`'s
//! `arbitrary_precision`, which changes how every number in the build graph
//! deserializes, so the second half of this file pins what a dependent sees.

use fhir_types::codec::{Json, Path as ElementPath, Value, expect_object};
use serde::{Deserialize, Serialize};

/// A decimal with a trailing zero, one past `f64` precision, and an exponent.
const DECIMALS: [&str; 4] = ["1.10", "0.1234567890123456789012345678", "2.0e3", "-0.0100"];

/// A `Parameters` document carrying `decimal` as `valueDecimal`.
fn parameters(decimal: &str) -> String {
    format!(
        r#"{{"resourceType":"Parameters","parameter":[{{"name":"d","valueDecimal":{decimal}}}]}}"#
    )
}

/// Both write paths for one version: the document `Json::to_json` builds and
/// the bytes the typed resource writes through `Serialize`.
fn round_trip<T: Json + Serialize>(decimal: &str, version: &str) {
    let text = parameters(decimal);
    let carried: Value = serde_json::from_str(&text).expect("the document parses");
    let mut path = ElementPath::root("Parameters");
    let object = expect_object(&carried, &path).expect("an object");
    let typed = T::from_json(object, &mut path).expect("the document decodes");
    let document =
        serde_json::to_string(&Value::Object(typed.to_json().expect("encodes"))).expect("writes");
    let direct = serde_json::to_string(&typed).expect("the typed resource writes");
    let written = format!(r#""valueDecimal":{decimal}"#);
    assert_eq!(document, direct, "{version} the two write paths agree");
    assert!(document.contains(&written), "{version} keeps {decimal}");
    let reread: Value = serde_json::from_str(&document).expect("the written document parses");
    assert_eq!(reread, carried, "{version} round trip");
}

#[test]
fn a_decimal_keeps_its_lexical_form_in_every_served_version() {
    for decimal in DECIMALS {
        round_trip::<fhir_types::r4::parameters::Parameters>(decimal, "R4");
        round_trip::<fhir_types::r4b::parameters::Parameters>(decimal, "R4B");
        round_trip::<fhir_types::r5::parameters::Parameters>(decimal, "R5");
        round_trip::<fhir_types::r6::parameters::Parameters>(decimal, "R6");
    }
}

#[test]
fn a_decimal_reaches_the_typed_value_as_the_text_the_document_carried() {
    for decimal in DECIMALS {
        let carried: Value = serde_json::from_str(&parameters(decimal)).expect("parses");
        let mut path = ElementPath::root("Parameters");
        let object = expect_object(&carried, &path).expect("an object");
        let typed = fhir_types::r4b::parameters::Parameters::from_json(object, &mut path)
            .expect("the document decodes");
        let value = typed
            .parameter
            .first()
            .and_then(|p| p.value.as_ref())
            .expect("the parameter carries a value");
        let fhir_types::r4b::parameters::ParametersParameterValue::Decimal(held) = value else {
            panic!("a decimal value");
        };
        assert_eq!(held.value.as_deref(), Some(decimal));
    }
}

/// A dependent's own wire format, with nothing FHIR about it.
///
/// `serde` buffers an internally tagged enum to find its tag
/// (<https://serde.rs/enum-representations.html>), and buffering a number is
/// exactly what `serde_json`'s `arbitrary_precision` breaks, so this is the
/// shape that fails the moment `fhir-types` forces that feature on a dependent.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Answer {
    Ordinal { score: f64 },
}

#[test]
fn depending_on_the_crate_leaves_a_dependent_own_json_alone() {
    let answer = Answer::Ordinal { score: 1.5 };
    let text = serde_json::to_string(&answer).expect("the answer writes");
    assert_eq!(text, r#"{"kind":"ordinal","score":1.5}"#);
    let back: Answer = serde_json::from_str(&text).expect("the answer reads back");
    assert_eq!(back, answer);
    let plain: serde_json::Value = serde_json::from_str("1.5").expect("a number parses");
    assert!(
        plain.is_number(),
        "a serde_json number stays a number under this crate's feature set"
    );
}
