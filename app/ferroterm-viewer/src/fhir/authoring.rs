//! The `CodeSystem` an editor reads back, in the elements it authors.
//!
//! The editor screen needs more of the resource than the detail screen shows:
//! the logical id and `meta.versionId` an update states in `If-Match`
//! (<https://hl7.org/fhir/R4B/http.html#concurrency>), the declared properties,
//! and every concept with its designations and property values
//! (<https://hl7.org/fhir/R4B/codesystem.html>). Those elements are read here,
//! outside every component, so the screen renders a value plain unit tests can
//! pin.
//!
//! This is still the minimum the screen draws. The viewer never mirrors a
//! whole FHIR resource.

use serde::Deserialize;
use serde_json::Value;

/// The resource type an entry must declare to be read as a code system.
const CODE_SYSTEM: &str = "CodeSystem";

/// What `GET [base]/CodeSystem?url=` answered, read for authoring.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct AuthoredSearch {
    /// `Bundle.entry`, one per match.
    #[serde(default)]
    entry: Vec<SearchEntry>,
}

/// One `Bundle.entry`.
///
/// The resource is kept as the server sent it. An update replaces the whole
/// resource (<https://hl7.org/fhir/R4B/http.html#update>), so every element
/// the editor does not draw still has to reach the server again, and an
/// element this viewer has never heard of cannot survive a typed read.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct SearchEntry {
    /// The resource the entry carries.
    resource: Option<Value>,
}

impl AuthoredSearch {
    /// The first `CodeSystem` the answer carries, in the server's own order.
    ///
    /// A `searchset` may also carry an `OperationOutcome` describing the search
    /// (<https://hl7.org/fhir/R4B/http.html#search>), so an entry declaring
    /// another type is passed over rather than read as a code system.
    pub(crate) fn first(&self) -> Option<Value> {
        self.entry
            .iter()
            .filter_map(|entry| entry.resource.as_ref())
            .find(|resource| {
                resource.get("resourceType").and_then(Value::as_str) == Some(CODE_SYSTEM)
            })
            .cloned()
    }
}

/// A `CodeSystem` as the editor reads it back off the server.
///
/// This is the elements the screen draws, and no more. The resource itself
/// travels beside it, so what the screen does not draw is still what is sent
/// back.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredCodeSystem {
    /// `id`, the logical id an update addresses.
    #[serde(default)]
    pub(crate) id: Option<String>,
    /// `meta`, which carries the version an update states.
    #[serde(default)]
    pub(crate) meta: Option<Meta>,
    /// `CodeSystem.url`, the canonical.
    #[serde(default)]
    pub(crate) url: Option<String>,
    /// `CodeSystem.version`, the business version.
    #[serde(default)]
    pub(crate) version: Option<String>,
    /// `CodeSystem.status`, a code of the publication status value set.
    #[serde(default)]
    pub(crate) status: Option<String>,
    /// `CodeSystem.content`, a code of the content mode value set.
    #[serde(default)]
    pub(crate) content: Option<String>,
    /// `CodeSystem.caseSensitive`.
    #[serde(default, rename = "caseSensitive")]
    pub(crate) case_sensitive: Option<bool>,
    /// `CodeSystem.property`, the properties this system declares.
    #[serde(default)]
    pub(crate) property: Vec<StoredProperty>,
    /// `CodeSystem.concept`, the concepts it defines.
    #[serde(default)]
    pub(crate) concept: Vec<StoredConcept>,
}

/// The `Resource.meta` elements the editor reads.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct Meta {
    /// `meta.versionId`, the version an `If-Match` states.
    #[serde(default, rename = "versionId")]
    pub(crate) version_id: Option<String>,
}

/// One `CodeSystem.property` declaration.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredProperty {
    /// `property.code`, the name a concept's value refers to.
    #[serde(default)]
    pub(crate) code: Option<String>,
    /// `property.uri`, the formal identifier of the property.
    #[serde(default)]
    pub(crate) uri: Option<String>,
    /// `property.type`, a code of the concept property type value set.
    #[serde(default, rename = "type")]
    pub(crate) kind: Option<String>,
}

/// One `CodeSystem.concept`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredConcept {
    /// `concept.code`.
    #[serde(default)]
    pub(crate) code: Option<String>,
    /// `concept.display`.
    #[serde(default)]
    pub(crate) display: Option<String>,
    /// `concept.definition`.
    #[serde(default)]
    pub(crate) definition: Option<String>,
    /// `concept.designation`.
    #[serde(default)]
    pub(crate) designation: Vec<StoredDesignation>,
    /// `concept.property`, the values this concept carries.
    #[serde(default)]
    pub(crate) property: Vec<StoredValue>,
}

/// One `concept.designation`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredDesignation {
    /// `designation.language`, a BCP 47 tag.
    #[serde(default)]
    pub(crate) language: Option<String>,
    /// `designation.use`, the coding that says what kind of term this is.
    #[serde(default, rename = "use")]
    pub(crate) usage: Option<StoredCoding>,
    /// `designation.value`, the term itself.
    #[serde(default)]
    pub(crate) value: Option<String>,
}

/// A `Coding`, in the two elements the editor authors.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredCoding {
    /// `Coding.system`.
    #[serde(default)]
    pub(crate) system: Option<String>,
    /// `Coding.code`.
    #[serde(default)]
    pub(crate) code: Option<String>,
}

/// One `concept.property`, read as the text the editor shows.
///
/// `property.value[x]` is a choice of seven types
/// (<https://hl7.org/fhir/R4B/codesystem-definitions.html#CodeSystem.concept.property.value_x_>),
/// and the editor draws one control per value whose shape the declared type
/// decides, so every arm is read into the text that control holds.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct StoredValue {
    /// `property.code`, naming the declared property.
    #[serde(default)]
    pub(crate) code: Option<String>,
    /// `valueCode`.
    #[serde(default, rename = "valueCode")]
    value_code: Option<String>,
    /// `valueCoding`.
    #[serde(default, rename = "valueCoding")]
    value_coding: Option<StoredCoding>,
    /// `valueString`.
    #[serde(default, rename = "valueString")]
    value_string: Option<String>,
    /// `valueInteger`, which FHIR defines as 32-bit signed
    /// (<https://hl7.org/fhir/R4B/datatypes.html#primitive>).
    #[serde(default, rename = "valueInteger")]
    value_integer: Option<i32>,
    /// `valueBoolean`.
    #[serde(default, rename = "valueBoolean")]
    value_boolean: Option<bool>,
    /// `valueDateTime`.
    #[serde(default, rename = "valueDateTime")]
    value_date_time: Option<String>,
    /// `valueDecimal`, read as the number's own text.
    #[serde(default, rename = "valueDecimal")]
    value_decimal: Option<serde_json::Number>,
}

impl StoredCodeSystem {
    /// The elements the editor draws, read out of `resource`.
    ///
    /// Every field is optional, so any JSON object reads; a document that is
    /// not an object reads as the empty resource, which the screen then shows
    /// as a code system it found nothing in.
    pub(crate) fn of(resource: &Value) -> Self {
        serde_json::from_value(resource.clone()).unwrap_or_default()
    }
}

/// The separator between a coding's system and its code, in one field.
///
/// FHIR search spells a coding `system|code`
/// (<https://hl7.org/fhir/R4B/search.html#token>), so the editor takes one
/// field in that syntax rather than two controls for one value.
pub(crate) const CODING_SEPARATOR: char = '|';

impl StoredValue {
    /// The value as the one text the editor's control holds.
    pub(crate) fn text(&self) -> String {
        if let Some(code) = &self.value_code {
            return code.clone();
        }
        if let Some(coding) = &self.value_coding {
            let system = coding.system.clone().unwrap_or_default();
            let code = coding.code.clone().unwrap_or_default();
            return format!("{system}{CODING_SEPARATOR}{code}");
        }
        if let Some(text) = &self.value_string {
            return text.clone();
        }
        if let Some(number) = self.value_integer {
            return number.to_string();
        }
        if let Some(flag) = self.value_boolean {
            return flag.to_string();
        }
        if let Some(moment) = &self.value_date_time {
            return moment.clone();
        }
        self.value_decimal
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_code_system_reads_the_elements_an_update_needs() {
        let answer: AuthoredSearch = serde_json::from_str(
            r#"{"resourceType":"Bundle","entry":[
                 {"resource":{"resourceType":"OperationOutcome"}},
                 {"resource":{"resourceType":"CodeSystem","id":"colours",
                   "meta":{"versionId":"4"},
                   "url":"https://terminology.example/colours","version":"1.0.0",
                   "status":"draft","content":"complete","caseSensitive":true,
                   "property":[{"code":"status","uri":"http://hl7.org/fhir/concept-properties#status","type":"code"}],
                   "concept":[{"code":"red","display":"Red","definition":"The colour red",
                     "designation":[{"language":"nl-NL","use":{"system":"https://terms.example/use","code":"syn"},"value":"Rood"}],
                     "property":[{"code":"status","valueCode":"active"}]}]}}]}"#,
        )
        .expect("the server's own answer parses");
        let stored =
            StoredCodeSystem::of(&answer.first().expect("the bundle carries a code system"));
        assert_eq!(stored.id.as_deref(), Some("colours"));
        assert_eq!(
            stored.meta.and_then(|meta| meta.version_id).as_deref(),
            Some("4"),
            "the version an If-Match states comes off the resource"
        );
        assert_eq!(stored.case_sensitive, Some(true));
        assert_eq!(stored.property.len(), 1);
        assert_eq!(stored.concept.len(), 1);
    }

    #[test]
    fn an_answer_carrying_no_code_system_reads_as_none() {
        let answer: AuthoredSearch = serde_json::from_str(
            r#"{"resourceType":"Bundle","entry":[{"resource":{"resourceType":"OperationOutcome"}}]}"#,
        )
        .expect("the server's own answer parses");
        assert_eq!(answer.first(), None);
        assert_eq!(AuthoredSearch::default().first(), None);
    }

    /// Every arm of `property.value[x]` reads back as the text its control
    /// holds, so a value the editor did not write is not dropped on a save.
    #[test]
    fn every_property_value_type_reads_as_the_text_its_control_holds() {
        for (json, expected) in [
            (r#"{"code":"p","valueCode":"active"}"#, "active"),
            (
                r#"{"code":"p","valueCoding":{"system":"https://x.example/s","code":"c"}}"#,
                "https://x.example/s|c",
            ),
            (r#"{"code":"p","valueString":"a phrase"}"#, "a phrase"),
            (r#"{"code":"p","valueInteger":-3}"#, "-3"),
            (r#"{"code":"p","valueBoolean":true}"#, "true"),
            (r#"{"code":"p","valueDateTime":"2026-09-24"}"#, "2026-09-24"),
            (r#"{"code":"p","valueDecimal":1.5}"#, "1.5"),
            (r#"{"code":"p"}"#, ""),
        ] {
            let value: StoredValue = serde_json::from_str(json).expect("the arm parses");
            assert_eq!(value.text(), expected, "{json}");
        }
    }
}
