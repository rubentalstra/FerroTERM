//! Synthetic FHIR `CodeSystem` resources (R5 JSON) for the generic provider:
//! invented systems under `http://example.org/`, no published content.

use std::path::Path;

/// The URL of the hierarchical animal system.
pub const ANIMALS: &str = "http://example.org/fhir/CodeSystem/animals";
/// The URL of the case-insensitive colour system.
pub const COLOURS: &str = "http://example.org/fhir/CodeSystem/colours";
/// The URL of the `example`-content system.
pub const SKETCH: &str = "http://example.org/fhir/CodeSystem/sketch";
/// The URL of the supplement to the animal system.
pub const ANIMALS_NL: &str = "http://example.org/fhir/CodeSystem/animals-nl";

/// The animal system: nesting for the hierarchy, `subsumedBy` for one extra
/// parent, `status`, `notSelectable`, and `inactive` properties, designations
/// in English and German, one declared property and filter.
fn animals() -> serde_json::Value {
    serde_json::json!({
      "resourceType": "CodeSystem",
      "url": ANIMALS, "version": "2.0", "name": "Animals", "title": "Animals (synthetic)",
      "status": "active", "content": "complete", "caseSensitive": true,
      "hierarchyMeaning": "is-a", "compositional": false, "versionNeeded": false,
      "filter": [{"code": "legs", "description": "By leg count", "operator": ["=", "in"], "value": "an integer"}],
      "property": [
        {"code": "legs", "uri": "http://example.org/legs", "description": "Leg count", "type": "integer"},
        {"code": "status", "uri": "http://hl7.org/fhir/concept-properties#status", "type": "code"},
        {"code": "notSelectable", "uri": "http://hl7.org/fhir/concept-properties#notSelectable", "type": "boolean"},
        {"code": "inactive", "uri": "http://hl7.org/fhir/concept-properties#inactive", "type": "boolean"},
        {"code": "subsumedBy", "type": "code"}
      ],
      "concept": [
        {"code": "living", "display": "Living thing", "property": [{"code": "notSelectable", "valueBoolean": true}], "concept": [
          {"code": "animal", "display": "Animal", "definition": "A living thing that is not a plant.",
           "designation": [{"language": "de", "value": "Tier"}],
           "concept": [
             {"code": "cat", "display": "Cat", "designation": [
                {"language": "en", "use": {"system": "http://snomed.info/sct", "code": "900000000000013009", "display": "Synonym"}, "value": "Domestic cat"},
                {"language": "de", "value": "Katze"}],
              "property": [{"code": "legs", "valueInteger": 4}]},
             {"code": "dog", "display": "Dog", "property": [{"code": "legs", "valueInteger": 4}]},
             {"code": "fish", "display": "Fish", "property": [{"code": "legs", "valueInteger": 0}, {"code": "status", "valueCode": "retired"}]}
           ]},
          {"code": "plant", "display": "Plant"}
        ]},
        {"code": "pet", "display": "Pet", "property": [{"code": "notSelectable", "valueBoolean": true}]},
        {"code": "kitten", "display": "Kitten", "property": [{"code": "subsumedBy", "valueCode": "cat"}, {"code": "subsumedBy", "valueCode": "pet"}, {"code": "legs", "valueInteger": 4}]},
        {"code": "dodo", "display": "Dodo", "property": [{"code": "inactive", "valueBoolean": true}]}
      ]
    })
}

fn colours() -> serde_json::Value {
    serde_json::json!({
      "resourceType": "CodeSystem",
      "url": COLOURS, "version": "1", "name": "Colours", "status": "active",
      "content": "complete", "caseSensitive": false,
      "concept": [
        {"code": "RED", "display": "Red"},
        {"code": "Green", "display": "Green"},
        {"code": "blue", "display": "Blue", "designation": [{"language": "nl", "value": "Blauw"}]}
      ]
    })
}

fn sketch() -> serde_json::Value {
    serde_json::json!({
      "resourceType": "CodeSystem",
      "url": SKETCH, "version": "0.1", "name": "Sketch", "status": "draft",
      "content": "example",
      "concept": [{"code": "a", "display": "An example"}]
    })
}

fn animals_nl() -> serde_json::Value {
    serde_json::json!({
      "resourceType": "CodeSystem",
      "url": ANIMALS_NL, "version": "1", "name": "AnimalsNl", "status": "active",
      "content": "supplement", "supplements": ANIMALS,
      "property": [{"code": "colour", "type": "string"}],
      "concept": [
        {"code": "cat", "designation": [{"language": "nl", "value": "Kat"}], "property": [{"code": "colour", "valueString": "any"}]},
        {"code": "dog", "designation": [{"language": "nl", "value": "Hond"}]}
      ]
    })
}

/// Writes the four resources (and a non-resource JSON file to be skipped)
/// into `dir`, as a FHIR package's `package/` directory would hold them.
///
/// # Errors
///
/// Returns the I/O error when a file cannot be written.
pub fn write_code_systems(dir: &Path) -> std::io::Result<()> {
    for (name, value) in [
        ("CodeSystem-animals.json", animals()),
        ("CodeSystem-colours.json", colours()),
        ("CodeSystem-sketch.json", sketch()),
        ("CodeSystem-animals-nl.json", animals_nl()),
        (
            "ValueSet-unrelated.json",
            serde_json::json!({"resourceType": "ValueSet", "status": "active"}),
        ),
        (
            "package.json",
            serde_json::json!({"name": "example.fixture", "version": "1.0.0", "fhirVersions": ["5.0.0"]}),
        ),
    ] {
        std::fs::write(dir.join(name), serde_json::to_string_pretty(&value)?)?;
    }
    Ok(())
}
