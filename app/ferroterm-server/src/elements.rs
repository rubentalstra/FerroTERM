//! The `_elements` projection: the subset of a resource a client asked for.
//!
//! `_elements` is a search parameter every served version defines
//! (`http://hl7.org/fhir/SearchParameter/Resource-elements`, listed in the
//! base `CapabilityStatement` of each vendored package). A client names the
//! elements it wants and the server returns those, marking what it sends as a
//! subset (<https://hl7.org/fhir/R5/search.html#elements>).
//!
//! Three obligations come with honouring it, and each is a test below.
//!
//! The specification calls the list a hint: "the server SHOULD always return
//! mandatory elements whether they are requested or not". So `resourceType`
//! and `id` survive whatever was asked for, and so does `meta`, which is where
//! the tag goes.
//!
//! A resource returned as a subset carries the `SUBSETTED` tag, from
//! `http://terminology.hl7.org/CodeSystem/v3-ObservationValue`, so a client
//! cannot mistake what it received for the whole resource and store it back.
//!
//! The projection is over the wire representation rather than the typed model,
//! which is what lets one implementation serve all four FHIR versions: a
//! subset of a resource is not a resource, so it has no typed form to build.

use std::collections::BTreeMap;

use fhir_types::codec::Object;
use fhir_types::codec::Value;

/// The search parameter this module answers.
pub const PARAMETER: &str = "_elements";

/// The elements a resource keeps whatever a client asked for.
///
/// `resourceType` is what makes the JSON a resource at all, `id` is what makes
/// it addressable, and `meta` carries the tag saying it is a subset.
const MANDATORY: [&str; 3] = ["resourceType", "id", "meta"];

/// The system of the tag that marks a subset.
const TAG_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-ObservationValue";

/// The code of that tag.
const TAG_CODE: &str = "SUBSETTED";

/// Its display, as the code system states it.
const TAG_DISPLAY: &str = "subsetted";

/// The elements a client asked for, read from one `_elements` value.
///
/// The value is a comma-separated list. An empty name is dropped rather than
/// refused: a trailing comma is a client's slip, not a request for an element
/// with no name.
#[must_use]
pub fn requested(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `resource`, reduced to `wanted` plus the elements every resource keeps, and
/// marked as a subset.
///
/// Returns the resource untouched when `wanted` is empty, because a client
/// that named nothing asked for everything.
#[must_use]
pub fn project(resource: &Object, wanted: &[String]) -> Object {
    if wanted.is_empty() {
        return resource.clone();
    }
    let mut kept: Object = resource
        .iter()
        .filter(|(name, _)| {
            MANDATORY.contains(&name.as_str()) || wanted.iter().any(|asked| asked == *name)
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    kept.insert("meta".to_owned(), subsetted(resource.get("meta")));
    kept
}

/// `meta` with the `SUBSETTED` tag added, keeping whatever it already held.
fn subsetted(existing: Option<&Value>) -> Value {
    let mut meta = match existing {
        Some(Value::Object(held)) => held.clone(),
        _ => BTreeMap::new(),
    };
    let mut tags = match meta.get("tag") {
        Some(Value::Array(held)) => held.clone(),
        _ => Vec::new(),
    };
    if !tags.iter().any(is_subsetted) {
        tags.push(Value::Object(BTreeMap::from([
            ("system".to_owned(), Value::String(TAG_SYSTEM.to_owned())),
            ("code".to_owned(), Value::String(TAG_CODE.to_owned())),
            ("display".to_owned(), Value::String(TAG_DISPLAY.to_owned())),
        ])));
    }
    meta.insert("tag".to_owned(), Value::Array(tags));
    Value::Object(meta)
}

/// Whether one `meta.tag` entry is already the subsetted mark.
fn is_subsetted(tag: &Value) -> bool {
    let Value::Object(coding) = tag else {
        return false;
    };
    coding.get("system") == Some(&Value::String(TAG_SYSTEM.to_owned()))
        && coding.get("code") == Some(&Value::String(TAG_CODE.to_owned()))
}

/// Every `entry.resource` of a searchset `bundle`, projected onto `wanted`.
///
/// The bundle itself is not a resource a client asked elements of: the
/// parameter names elements of the resources a search matched, so the envelope
/// keeps every field it had.
pub fn project_bundle(bundle: &mut Object, wanted: &[String]) {
    if wanted.is_empty() {
        return;
    }
    let Some(Value::Array(entries)) = bundle.get_mut("entry") else {
        return;
    };
    for entry in entries {
        let Value::Object(entry) = entry else {
            continue;
        };
        if let Some(Value::Object(resource)) = entry.get("resource") {
            let projected = project(resource, wanted);
            entry.insert("resource".to_owned(), Value::Object(projected));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `CodeSystem` as a search would have rendered it.
    fn code_system() -> Object {
        BTreeMap::from([
            (
                "resourceType".to_owned(),
                Value::String("CodeSystem".to_owned()),
            ),
            ("id".to_owned(), Value::String("loinc".to_owned())),
            (
                "url".to_owned(),
                Value::String("http://loinc.org".to_owned()),
            ),
            ("title".to_owned(), Value::String("LOINC".to_owned())),
            ("version".to_owned(), Value::String("2.83".to_owned())),
            ("status".to_owned(), Value::String("active".to_owned())),
            ("concept".to_owned(), Value::Array(vec![Value::Null; 900])),
        ])
    }

    /// The tag entries a projected resource carries.
    fn tags(projected: &Object) -> Vec<String> {
        let Some(Value::Object(meta)) = projected.get("meta") else {
            return Vec::new();
        };
        let Some(Value::Array(tags)) = meta.get("tag") else {
            return Vec::new();
        };
        tags.iter()
            .filter_map(|tag| match tag {
                Value::Object(coding) => match coding.get("code") {
                    Some(Value::String(code)) => Some(code.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_comma_separated_value_is_the_list_of_elements() {
        assert_eq!(requested("url,name,title"), ["url", "name", "title"]);
        assert_eq!(
            requested(" url , title "),
            ["url", "title"],
            "space around a name is a client's formatting, not part of it"
        );
        assert_eq!(
            requested("url,,title,"),
            ["url", "title"],
            "an empty name is a slip, not a request for an element with no name"
        );
        assert!(requested("").is_empty());
    }

    #[test]
    fn naming_nothing_asks_for_everything() {
        let whole = code_system();
        assert_eq!(
            project(&whole, &[]),
            whole,
            "a search with no _elements is the search it always was"
        );
    }

    #[test]
    fn only_the_named_elements_and_the_mandatory_ones_survive() {
        let projected = project(&code_system(), &requested("url,title"));
        let mut names: Vec<&str> = projected.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["id", "meta", "resourceType", "title", "url"]);
        assert!(
            !projected.contains_key("concept"),
            "the element a client did not ask for is what this parameter exists to leave out"
        );
    }

    #[test]
    fn a_mandatory_element_survives_whether_it_was_asked_for_or_not() {
        let projected = project(&code_system(), &requested("title"));
        assert_eq!(
            projected.get("resourceType"),
            Some(&Value::String("CodeSystem".to_owned())),
            "the specification says a server returns mandatory elements whether they are requested or not"
        );
        assert_eq!(
            projected.get("id"),
            Some(&Value::String("loinc".to_owned())),
            "a resource with no id cannot be read back by the address the search gave"
        );
    }

    #[test]
    fn a_subset_says_it_is_one() {
        let projected = project(&code_system(), &requested("url"));
        assert_eq!(
            tags(&projected),
            ["SUBSETTED"],
            "a client must not mistake a subset for the whole resource and store it back"
        );
    }

    #[test]
    fn the_tag_is_added_once_however_often_a_resource_is_projected() {
        let once = project(&code_system(), &requested("url"));
        let twice = project(&once, &requested("url"));
        assert_eq!(tags(&twice), ["SUBSETTED"]);
    }

    #[test]
    fn a_tag_the_resource_already_carried_is_kept() {
        let mut whole = code_system();
        whole.insert(
            "meta".to_owned(),
            Value::Object(BTreeMap::from([(
                "tag".to_owned(),
                Value::Array(vec![Value::Object(BTreeMap::from([
                    (
                        "system".to_owned(),
                        Value::String("http://example.org/tags".to_owned()),
                    ),
                    ("code".to_owned(), Value::String("local".to_owned())),
                ]))]),
            )])),
        );
        let projected = project(&whole, &requested("url"));
        assert_eq!(
            tags(&projected),
            ["local", "SUBSETTED"],
            "the server adds its mark rather than replacing what the resource held"
        );
    }

    #[test]
    fn an_element_the_resource_does_not_have_is_not_an_error() {
        let projected = project(&code_system(), &requested("url,copyright"));
        assert!(
            !projected.contains_key("copyright"),
            "an absent element is absent, not null"
        );
        assert!(
            projected.contains_key("url"),
            "the elements it does have still come back"
        );
    }

    #[test]
    fn every_entry_of_a_searchset_is_projected_and_the_envelope_is_not() {
        let mut bundle: Object = BTreeMap::from([
            (
                "resourceType".to_owned(),
                Value::String("Bundle".to_owned()),
            ),
            ("type".to_owned(), Value::String("searchset".to_owned())),
            ("total".to_owned(), Value::String("2".to_owned())),
            (
                "entry".to_owned(),
                Value::Array(vec![
                    Value::Object(BTreeMap::from([(
                        "resource".to_owned(),
                        Value::Object(code_system()),
                    )])),
                    Value::Object(BTreeMap::from([(
                        "resource".to_owned(),
                        Value::Object(code_system()),
                    )])),
                ]),
            ),
        ]);
        project_bundle(&mut bundle, &requested("url"));
        assert_eq!(
            bundle.get("total"),
            Some(&Value::String("2".to_owned())),
            "the parameter names elements of the resources, not of the envelope"
        );
        let Some(Value::Array(entries)) = bundle.get("entry") else {
            panic!("the bundle keeps its entries");
        };
        for entry in entries {
            let Value::Object(entry) = entry else {
                panic!("an entry is an object");
            };
            let Some(Value::Object(resource)) = entry.get("resource") else {
                panic!("an entry carries its resource");
            };
            assert!(!resource.contains_key("concept"));
            assert_eq!(tags(resource), ["SUBSETTED"]);
        }
    }
}
