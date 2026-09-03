//! Supplements add designations and properties without touching the system.

use std::collections::BTreeMap;
use std::sync::Arc;

use fhir_terminology::provider::{CodeSystemProvider, Designation, Property, PropertyValue};
use fhir_terminology::supplement::{Additions, Supplement, Supplemented};

use crate::fixture::Fixture;

#[test]
fn a_supplement_layers_designations_and_properties_over_the_system() {
    let inner: Arc<dyn CodeSystemProvider> = Arc::new(Fixture::hierarchical("2025"));
    let supplement = Supplement {
        url: String::from("http://example.org/fixture-de"),
        version: Some(String::from("1")),
        concepts: BTreeMap::from([(
            String::from("cat"),
            Additions {
                designations: vec![Designation {
                    language: Some(String::from("de")),
                    use_: None,
                    value: String::from("Katze"),
                }],
                properties: vec![Property {
                    code: String::from("colour"),
                    value: PropertyValue::String(String::from("any")),
                    ..Property::default()
                }],
            },
        )]),
    };
    let supplemented = Supplemented::new(inner, vec![supplement]);
    let cat = supplemented
        .locate("cat")
        .expect("reads")
        .expect("cat")
        .concept;
    let dog = supplemented
        .locate("dog")
        .expect("reads")
        .expect("dog")
        .concept;
    assert_eq!(
        supplemented
            .designations(cat, Some("de"))
            .expect("reads")
            .len(),
        1
    );
    assert_eq!(
        supplemented.designations(cat, None).expect("reads").len(),
        3
    );
    assert_eq!(
        supplemented.designations(dog, None).expect("reads").len(),
        2
    );
    assert_eq!(
        supplemented
            .display(cat, Some("de"))
            .expect("reads")
            .as_deref(),
        Some("Katze"),
        "a supplement designation in the requested language is the display \
         (<https://hl7.org/fhir/R4B/codesystem.html#supplements>, \
         <https://hl7.org/fhir/R4B/valueset-operation-expand.html> displayLanguage)"
    );
    assert_eq!(
        supplemented.display(cat, None).expect("reads").as_deref(),
        Some("Cat"),
        "without a language the system's display wins"
    );
    assert!(
        supplemented
            .properties(cat)
            .expect("reads")
            .iter()
            .any(|p| p.code == "colour")
    );
    assert!(
        !supplemented
            .properties(dog)
            .expect("reads")
            .iter()
            .any(|p| p.code == "colour")
    );
    assert_eq!(supplemented.declaration().languages, ["de", "en", "nl"]);
    assert_eq!(supplemented.identity().version, "2025");
    assert_eq!(supplemented.supplements().len(), 1);
}
