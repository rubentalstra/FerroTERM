//! The generic `CodeSystem` provider over the synthetic R5 resources.

use std::sync::Arc;

use concept_graph::subsumption::Outcome;
use ferroterm_testkit::fhir::{ANIMALS, ANIMALS_NL, COLOURS, SKETCH, write_code_systems};
use fhir_terminology::fhir_codesystem::load::{FhirVersion, load_dir, package_version};
use fhir_terminology::fhir_codesystem::provider::FhirCodeSystem;
use fhir_terminology::filter::{Filter, FilterOperator};
use fhir_terminology::operations::{Invocation, OperationError, lookup, validate_code};
use fhir_terminology::provider::{
    Capability, CodeSystemProvider, ContentMode, PropertyValue, ProviderError,
};
use fhir_terminology::registry::Registry;
use fhir_terminology::supplement::{Additions, Supplement, Supplemented};

fn load_all() -> (tempfile::TempDir, Vec<FhirCodeSystem>) {
    let dir = tempfile::tempdir().expect("tempdir");
    write_code_systems(dir.path()).expect("writes");
    assert_eq!(
        package_version(dir.path()).expect("reads"),
        Some(FhirVersion::R5)
    );
    let models = load_dir(dir.path(), FhirVersion::R5).expect("loads");
    let providers = models
        .into_iter()
        .map(|m| FhirCodeSystem::new(m).expect("builds"))
        .collect();
    (dir, providers)
}

fn find<'a>(providers: &'a [FhirCodeSystem], url: &str) -> &'a FhirCodeSystem {
    providers
        .iter()
        .find(|p| p.identity().url == url)
        .expect("system")
}

fn codes(p: &dyn CodeSystemProvider, set: &roaring::RoaringBitmap) -> Vec<String> {
    let mut out: Vec<String> = set
        .iter()
        .filter_map(|i| {
            p.code(fhir_terminology::provider::Concept::new(i))
                .expect("reads")
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_directory_loads_every_code_system_and_skips_other_resources() {
    let (_dir, providers) = load_all();
    let mut urls: Vec<&str> = providers
        .iter()
        .map(|p| p.identity().url.as_str())
        .collect();
    urls.sort_unstable();
    assert_eq!(urls, [ANIMALS, ANIMALS_NL, COLOURS, SKETCH]);
    let animals = find(&providers, ANIMALS);
    assert_eq!(animals.identity().version, "2.0");
    assert_eq!(
        animals.identity().title.as_deref(),
        Some("Animals (synthetic)")
    );
    let d = animals.declaration();
    assert_eq!(d.content, ContentMode::Complete);
    assert!(d.capabilities.contains(&Capability::Subsumption));
    assert!(d.capabilities.contains(&Capability::Enumeration));
    assert_eq!(d.languages, ["de", "en"]);
    assert_eq!(d.filters[0].code, "legs");
    assert_eq!(
        d.filters[0].operators,
        [FilterOperator::Equal, FilterOperator::In]
    );
    let names: Vec<&str> = d.properties.iter().map(|p| p.code.as_str()).collect();
    assert!(names.contains(&"legs") && names.contains(&"parent") && names.contains(&"child"));
}

#[test]
fn locate_display_designations_and_status_follow_the_resource() {
    let (_dir, providers) = load_all();
    let animals = find(&providers, ANIMALS);
    let cat = animals.locate("cat").expect("reads").expect("cat").concept;
    assert!(
        animals.locate("CAT").expect("reads").is_none(),
        "case-sensitive system"
    );
    assert_eq!(
        animals.display(cat, None).expect("reads").as_deref(),
        Some("Cat")
    );
    assert_eq!(
        animals
            .display(cat, Some("de-DE"))
            .expect("reads")
            .as_deref(),
        Some("Katze")
    );
    assert_eq!(
        animals.display(cat, Some("fr")).expect("reads").as_deref(),
        Some("Cat")
    );
    assert_eq!(
        animals.designations(cat, Some("en")).expect("reads")[0].value,
        "Domestic cat"
    );
    assert_eq!(
        animals
            .definition(
                animals
                    .locate("animal")
                    .expect("reads")
                    .expect("animal")
                    .concept
            )
            .expect("reads")
            .as_deref(),
        Some("A living thing that is not a plant.")
    );
    let fish = animals
        .locate("fish")
        .expect("reads")
        .expect("fish")
        .concept;
    let dodo = animals
        .locate("dodo")
        .expect("reads")
        .expect("dodo")
        .concept;
    let pet = animals.locate("pet").expect("reads").expect("pet").concept;
    assert!(
        !animals.status(fish).expect("reads").active,
        "status retired is inactive"
    );
    assert!(
        !animals.status(dodo).expect("reads").active,
        "inactive = true"
    );
    assert!(
        animals.status(pet).expect("reads").abstract_concept,
        "notSelectable is abstract"
    );
    assert!(animals.status(cat).expect("reads").active);

    let colours = find(&providers, COLOURS);
    let red = colours
        .locate("red")
        .expect("reads")
        .expect("case-insensitive match");
    assert_eq!(red.code, "RED", "the system's own spelling is returned");
    assert!(colours.locate("BLUE").expect("reads").is_some());
}

#[test]
fn properties_and_hierarchy_come_from_nesting_and_subsumed_by() {
    let (_dir, providers) = load_all();
    let animals = find(&providers, ANIMALS);
    let kitten = animals
        .locate("kitten")
        .expect("reads")
        .expect("kitten")
        .concept;
    let props = animals.properties(kitten).expect("reads");
    let parents: Vec<&PropertyValue> = props
        .iter()
        .filter(|p| p.code == "parent")
        .map(|p| &p.value)
        .collect();
    assert_eq!(
        parents,
        [
            &PropertyValue::Code(String::from("cat")),
            &PropertyValue::Code(String::from("pet"))
        ]
    );
    assert!(
        props
            .iter()
            .any(|p| p.code == "legs" && p.value == PropertyValue::Integer(4))
    );
    assert!(
        props
            .iter()
            .any(|p| p.code == "inactive" && p.value == PropertyValue::Boolean(false))
    );
    let hierarchy = animals.hierarchy().expect("is-a hierarchy");
    let living = animals
        .locate("living")
        .expect("reads")
        .expect("living")
        .concept;
    let cat = animals.locate("cat").expect("reads").expect("cat").concept;
    assert_eq!(hierarchy.subsumes(living, kitten), Outcome::Subsumes);
    assert_eq!(hierarchy.subsumes(kitten, cat), Outcome::SubsumedBy);
    let leaves = animals
        .filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::DescendentLeaf,
            value: String::from("animal"),
        })
        .expect("filters");
    assert_eq!(codes(animals, &leaves), ["dog", "fish", "kitten"]);
    let four_legs = animals
        .filter(&Filter {
            property: String::from("legs"),
            op: FilterOperator::Equal,
            value: String::from("4"),
        })
        .expect("filters");
    assert_eq!(codes(animals, &four_legs), ["cat", "dog", "kitten"]);
    let children_of_living = animals
        .properties(living)
        .expect("reads")
        .iter()
        .filter(|p| p.code == "child")
        .count();
    assert_eq!(children_of_living, 2);
    let colours = find(&providers, COLOURS);
    assert!(
        colours.hierarchy().is_none(),
        "no hierarchyMeaning, no subsumption"
    );
    assert!(matches!(
        colours.filter(&Filter {
            property: String::from("concept"),
            op: FilterOperator::IsA,
            value: String::from("RED")
        }),
        Err(ProviderError::UnsupportedFilter { .. })
    ));
}

#[test]
fn search_folds_case_and_diacritics_and_respects_language() {
    let (_dir, providers) = load_all();
    let animals = find(&providers, ANIMALS);
    assert_eq!(
        codes(
            animals,
            &animals.search("kat", Some("de")).expect("searches")
        ),
        ["cat"]
    );
    assert_eq!(
        codes(animals, &animals.search("dom cat", None).expect("searches")),
        ["cat"]
    );
    assert_eq!(
        codes(animals, &animals.search("PLA", None).expect("searches")),
        ["plant"]
    );
    assert!(animals.search("zebra", None).expect("searches").is_empty());
}

#[test]
fn example_content_refuses_validation_and_enumeration() {
    let (_dir, providers) = load_all();
    let sketch = find(&providers, SKETCH);
    assert!(matches!(
        sketch.locate("a"),
        Err(ProviderError::IncompleteContent { .. })
    ));
    assert!(matches!(sketch.all(), Err(ProviderError::NotEnumerable)));
    let mut registry = Registry::new();
    let (_dir2, providers2) = load_all();
    for p in providers2 {
        registry.register(Arc::new(p)).expect("registers");
    }
    let request = validate_code::ValidateCodeInput {
        url: Some(SKETCH.to_owned()),
        code: Some(String::from("a")),
        ..validate_code::ValidateCodeInput::default()
    };
    let error =
        validate_code::validate_code(&registry, &Invocation::Type, &request).expect_err("refused");
    assert!(matches!(error, OperationError::NotSupported(_)));
    assert_eq!(error.issue_code(), "not-supported");
}

#[test]
fn a_supplement_layers_designations_and_properties_over_the_system() {
    let (_dir, providers) = load_all();
    let supplement_model = find(&providers, ANIMALS_NL).model().clone();
    assert_eq!(supplement_model.content, ContentMode::Supplement);
    assert_eq!(supplement_model.supplements.as_deref(), Some(ANIMALS));
    let mut concepts = std::collections::BTreeMap::new();
    for entry in &supplement_model.concepts {
        concepts.insert(
            entry.code.clone(),
            Additions {
                designations: entry.designations.clone(),
                properties: entry.properties.clone(),
            },
        );
    }
    let supplement = Supplement {
        url: supplement_model.url.clone(),
        version: Some(supplement_model.version.clone()),
        concepts,
    };
    let (_dir3, mut base) = load_all();
    let animals = base.remove(
        base.iter()
            .position(|p| p.identity().url == ANIMALS)
            .expect("animals"),
    );
    let supplemented = Supplemented::new(Arc::new(animals), vec![supplement]);
    let cat = supplemented
        .locate("cat")
        .expect("reads")
        .expect("cat")
        .concept;
    assert_eq!(
        supplemented
            .display(cat, Some("nl"))
            .expect("reads")
            .as_deref(),
        Some("Kat")
    );
    assert!(
        supplemented
            .properties(cat)
            .expect("reads")
            .iter()
            .any(|p| p.code == "colour")
    );
    assert_eq!(supplemented.declaration().languages, ["de", "en", "nl"]);

    let mut registry = Registry::new();
    registry
        .register(Arc::new(supplemented))
        .expect("registers");
    let input = lookup::LookupInput {
        system: Some(ANIMALS.to_owned()),
        code: Some(String::from("cat")),
        display_language: Some(String::from("nl")),
        ..lookup::LookupInput::default()
    };
    let outcome = lookup::lookup(&registry, &Invocation::Type, &input).expect("looks up");
    assert_eq!(outcome.display, "Kat");
    assert_eq!(outcome.version.as_deref(), Some("2.0"));
}

fn take(providers: Vec<FhirCodeSystem>, url: &str) -> FhirCodeSystem {
    providers
        .into_iter()
        .find(|p| p.identity().url == url)
        .expect("system")
}

fn registry_of(provider: FhirCodeSystem) -> Registry {
    let mut registry = Registry::new();
    registry.register(Arc::new(provider)).expect("registers");
    registry
}

fn lookup_cat(registry: &Registry, properties: &[&str]) -> lookup::LookupOutcome {
    let input = lookup::LookupInput {
        system: Some(ANIMALS.to_owned()),
        code: Some(String::from("cat")),
        properties: properties.iter().map(|p| (*p).to_owned()).collect(),
        ..lookup::LookupInput::default()
    };
    lookup::lookup(registry, &Invocation::Type, &input).expect("looks up")
}

#[test]
fn lookup_property_star_answers_parent_child_inactive_and_the_declared_properties() {
    let (_dir, providers) = load_all();
    let registry = registry_of(take(providers, ANIMALS));
    let outcome = lookup_cat(&registry, &["*"]);
    let properties: Vec<(&str, String)> = outcome
        .properties
        .iter()
        .map(|p| (p.code.as_str(), p.value.as_text()))
        .collect();
    assert_eq!(
        properties,
        [
            ("inactive", String::from("false")),
            ("legs", String::from("4")),
            ("parent", String::from("animal")),
            ("child", String::from("kitten")),
        ]
    );
    let named = lookup_cat(&registry, &["parent", "inactive"]);
    let codes: Vec<&str> = named.properties.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(codes, ["inactive", "parent"]);
    assert!(
        named.designations.is_empty(),
        "designations were not asked for"
    );
}

#[test]
fn lookup_answers_the_display_as_a_designation_in_the_system_language() {
    let (_dir, providers) = load_all();
    let registry = registry_of(take(providers, ANIMALS));
    let outcome = lookup_cat(&registry, &[]);
    assert_eq!(outcome.display, "Cat");
    let display = outcome
        .designations
        .iter()
        .find(|d| d.value == "Cat")
        .expect("the display is a designation");
    assert_eq!(display.language.as_deref(), Some("en"));
    let use_ = display.use_.as_ref().expect("has a use");
    assert_eq!(
        (use_.system.as_str(), use_.code.as_str()),
        (
            "http://terminology.hl7.org/CodeSystem/hl7TermMaintInfra",
            "preferredForLanguage"
        )
    );
    let values: Vec<&str> = outcome
        .designations
        .iter()
        .map(|d| d.value.as_str())
        .collect();
    assert_eq!(values, ["Domestic cat", "Katze", "Cat"]);
    let german = lookup_cat(&registry, &["lang.de"]);
    let values: Vec<&str> = german
        .designations
        .iter()
        .map(|d| d.value.as_str())
        .collect();
    assert_eq!(
        values,
        ["Katze"],
        "lang.de keeps the German designations only"
    );
}

#[test]
fn lookup_name_is_the_code_system_name_then_the_title_then_the_url() {
    let (_dir, providers) = load_all();
    let animals = take(providers, ANIMALS);
    let mut model = animals.model().clone();
    assert_eq!(lookup_cat(&registry_of(animals), &[]).name, "Animals");
    model.name = None;
    let titled = FhirCodeSystem::new(model.clone()).expect("builds");
    assert_eq!(
        lookup_cat(&registry_of(titled), &[]).name,
        "Animals (synthetic)"
    );
    model.title = None;
    let bare = FhirCodeSystem::new(model).expect("builds");
    assert_eq!(lookup_cat(&registry_of(bare), &[]).name, ANIMALS);
}

#[test]
fn the_vendored_hl7_terminology_package_loads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/fhir-codegen/vendor/hl7.terminology/package");
    let version = package_version(&dir)
        .expect("reads")
        .expect("declares a version");
    let models = load_dir(&dir, version).expect("loads");
    assert!(models.len() > 900, "{} code systems", models.len());
    let mut built = 0;
    for model in models {
        FhirCodeSystem::new(model).expect("builds");
        built += 1;
    }
    assert!(built > 900);
}

// NOTE: R5 `$lookup` declares `definition` as its own output
// (<https://hl7.org/fhir/R5/codesystem-operation-lookup.html>).
#[test]
fn lookup_carries_the_definition_when_the_system_states_one() {
    let mut registry = Registry::new();
    let (_dir, providers) = load_all();
    for p in providers {
        registry.register(Arc::new(p)).expect("registers");
    }
    let animal = lookup::lookup(
        &registry,
        &Invocation::Type,
        &lookup::LookupInput {
            system: Some(ANIMALS.to_owned()),
            code: Some(String::from("animal")),
            ..lookup::LookupInput::default()
        },
    )
    .expect("looks up");
    assert_eq!(
        animal.definition.as_deref(),
        Some("A living thing that is not a plant.")
    );
    let cat = lookup::lookup(
        &registry,
        &Invocation::Type,
        &lookup::LookupInput {
            system: Some(ANIMALS.to_owned()),
            code: Some(String::from("cat")),
            ..lookup::LookupInput::default()
        },
    )
    .expect("looks up");
    assert_eq!(cat.definition, None);
}
