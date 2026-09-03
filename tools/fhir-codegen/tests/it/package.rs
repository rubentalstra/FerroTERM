use std::path::PathBuf;

use fhir_codegen::fhir::{Derivation, StructureKind};
use fhir_codegen::package::{LoadError, Package};

use crate::{R4B, r4b_dir};

#[test]
fn manifest_names_the_pinned_package() {
    let manifest = R4B.manifest();
    assert_eq!(manifest.name, "hl7.fhir.r4b.core");
    assert_eq!(manifest.version, "4.3.0");
    assert_eq!(manifest.fhir_versions, vec!["4.3.0".to_owned()]);
    assert_eq!(R4B.root(), r4b_dir());
}

#[test]
fn loads_every_conformance_resource_of_the_four_kinds() {
    // The counts of R4B core 4.3.0, one file per resource.
    assert_eq!(R4B.structure_definitions().len(), 651);
    assert_eq!(R4B.operation_definitions().len(), 47);
    assert_eq!(R4B.value_sets().len(), 721);
    assert_eq!(R4B.code_systems().len(), 540);
}

#[test]
fn structure_definitions_are_keyed_by_canonical_url() {
    let value_set = R4B
        .structure_definitions()
        .get("http://hl7.org/fhir/StructureDefinition/ValueSet")
        .expect("ValueSet is defined");
    assert_eq!(value_set.name, "ValueSet");
    assert_eq!(value_set.type_name, "ValueSet");
    assert_eq!(value_set.kind, StructureKind::Resource);
    assert_eq!(value_set.derivation, Some(Derivation::Specialization));
    assert!(!value_set.is_abstract);
    assert_eq!(
        value_set.base_definition.as_deref(),
        Some("http://hl7.org/fhir/StructureDefinition/DomainResource")
    );
    assert_eq!(value_set.version.as_deref(), Some("4.3.0"));
    assert_eq!(
        R4B.structure_definition_named("ValueSet")
            .map(|d| d.url.as_str()),
        Some("http://hl7.org/fhir/StructureDefinition/ValueSet")
    );
}

#[test]
fn only_two_example_profiles_lack_a_snapshot() {
    // Every type definition in 4.3.0 ships a snapshot; two example profiles do not.
    let without = R4B
        .structure_definitions()
        .values()
        .filter(|definition| definition.snapshot.is_none())
        .map(|definition| (definition.url.as_str(), definition.derivation))
        .collect::<Vec<_>>();
    assert_eq!(
        without,
        vec![
            (
                "http://hl7.org/fhir/StructureDefinition/example-composition",
                Some(Derivation::Constraint)
            ),
            (
                "http://hl7.org/fhir/StructureDefinition/example-section-library",
                Some(Derivation::Constraint)
            ),
        ]
    );
    let specializations_without = R4B
        .structure_definitions()
        .values()
        .filter(|d| d.derivation != Some(Derivation::Constraint) && d.snapshot.is_none())
        .count();
    assert_eq!(specializations_without, 0);
}

#[test]
fn primitive_and_complex_kinds_are_read() {
    let string = R4B
        .structure_definition_named("string")
        .expect("string is defined");
    assert_eq!(string.kind, StructureKind::PrimitiveType);
    let coding = R4B
        .structure_definition_named("Coding")
        .expect("Coding is defined");
    assert_eq!(coding.kind, StructureKind::ComplexType);
    let element = R4B
        .structure_definition_named("Element")
        .expect("Element is defined");
    assert!(element.is_abstract);
}

#[test]
fn operation_definition_shape_is_read() {
    let expand = R4B
        .operation_definitions()
        .get("http://hl7.org/fhir/OperationDefinition/ValueSet-expand")
        .expect("$expand is defined");
    assert_eq!(expand.code, "expand");
    assert_eq!(expand.kind, "operation");
    assert_eq!(expand.resource, vec!["ValueSet".to_owned()]);
    assert!(!expand.system);
    assert!(expand.type_level);
    assert!(expand.instance);
    let names = expand
        .parameter
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    // The R4B $expand parameter list
    // (https://hl7.org/fhir/R4B/valueset-operation-expand.html).
    assert_eq!(
        names,
        vec![
            "url",
            "valueSet",
            "valueSetVersion",
            "context",
            "contextDirection",
            "filter",
            "date",
            "offset",
            "count",
            "includeDesignations",
            "designation",
            "includeDefinition",
            "activeOnly",
            "excludeNested",
            "excludeNotForUI",
            "excludePostCoordinated",
            "displayLanguage",
            "exclude-system",
            "system-version",
            "check-system-version",
            "force-system-version",
            "return",
        ]
    );
}

#[test]
fn value_set_and_code_system_shapes_are_read() {
    let value_set = R4B
        .value_sets()
        .get("http://hl7.org/fhir/ValueSet/filter-operator")
        .expect("filter-operator value set is defined");
    assert_eq!(value_set.status.as_deref(), Some("active"));
    let compose = value_set.compose.as_ref().expect("has compose");
    assert_eq!(
        compose
            .include
            .iter()
            .map(|include| include.system.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("http://hl7.org/fhir/filter-operator")]
    );

    let code_system = R4B
        .code_systems()
        .get("http://hl7.org/fhir/filter-operator")
        .expect("filter-operator code system is defined");
    assert_eq!(code_system.content, "complete");
    let codes = code_system
        .concept
        .iter()
        .map(|concept| concept.code.as_str())
        .collect::<Vec<_>>();
    // The R4B FilterOperator codes (https://hl7.org/fhir/R4B/codesystem-filter-operator.html).
    assert_eq!(
        codes,
        vec![
            "=",
            "is-a",
            "descendent-of",
            "is-not-a",
            "regex",
            "in",
            "not-in",
            "generalizes",
            "exists"
        ]
    );
}

#[test]
fn the_published_package_omits_status_on_exactly_two_resources() {
    // CodeSystem.status and ValueSet.status are 1..1 in R4B, yet the published
    // 4.3.0 package leaves both catalogType resources without one.
    let code_systems = R4B
        .code_systems()
        .values()
        .filter(|cs| cs.status.is_none())
        .map(|cs| cs.url.as_str())
        .collect::<Vec<_>>();
    assert_eq!(code_systems, vec!["http://hl7.org/fhir/catalogType"]);
    let value_sets = R4B
        .value_sets()
        .values()
        .filter(|vs| vs.status.is_none())
        .map(|vs| vs.url.as_str())
        .collect::<Vec<_>>();
    assert_eq!(value_sets, vec!["http://hl7.org/fhir/ValueSet/catalogType"]);
}

#[test]
fn source_files_are_recorded() {
    let source = R4B
        .source_of(
            "StructureDefinition",
            "http://hl7.org/fhir/StructureDefinition/ValueSet",
        )
        .expect("source is recorded");
    assert_eq!(
        source.file_name().and_then(|name| name.to_str()),
        Some("StructureDefinition-ValueSet.json")
    );
    assert!(R4B.source_of("ValueSet", "urn:nothing").is_none());
}

#[test]
fn a_directory_without_a_manifest_is_not_a_package() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    match Package::open(dir.path()) {
        Err(LoadError::NotAPackage { path }) => assert_eq!(path, dir.path()),
        other => panic!("expected NotAPackage, got {other:?}"),
    }
    Ok(())
}

#[test]
fn a_duplicate_canonical_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let content = dir.path().join("package");
    std::fs::create_dir(&content)?;
    std::fs::write(
        content.join("package.json"),
        r#"{"name":"test.pkg","version":"0.1.0"}"#,
    )?;
    let value_set =
        r#"{"resourceType":"ValueSet","url":"http://example.org/vs","status":"active"}"#;
    std::fs::write(content.join("ValueSet-a.json"), value_set)?;
    std::fs::write(content.join("ValueSet-b.json"), value_set)?;
    match Package::open(dir.path()) {
        Err(LoadError::DuplicateCanonical {
            resource_type,
            url,
            first,
            second,
        }) => {
            assert_eq!(resource_type, "ValueSet");
            assert_eq!(url, "http://example.org/vs");
            assert_eq!(first, content.join("ValueSet-a.json"));
            assert_eq!(second, content.join("ValueSet-b.json"));
        }
        other => panic!("expected DuplicateCanonical, got {other:?}"),
    }
    Ok(())
}

#[test]
fn a_malformed_resource_names_its_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let content = dir.path().join("package");
    std::fs::create_dir(&content)?;
    std::fs::write(
        content.join("package.json"),
        r#"{"name":"test.pkg","version":"0.1.0"}"#,
    )?;
    std::fs::write(
        content.join("CodeSystem-broken.json"),
        r#"{"resourceType":"CodeSystem"}"#,
    )?;
    match Package::open(dir.path()) {
        Err(LoadError::Json { path, source }) => {
            assert_eq!(path, content.join("CodeSystem-broken.json"));
            assert!(source.to_string().contains("missing field"));
        }
        other => panic!("expected Json, got {other:?}"),
    }
    Ok(())
}

#[test]
fn the_loader_reads_an_r5_package_unchanged() {
    // hl7.terminology 7.3.0 is published on R5 (5.0.0); the same loader reads it unchanged.
    let tho =
        Package::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/hl7.terminology"))
            .expect("the vendored hl7.terminology package should load");
    assert_eq!(tho.manifest().name, "hl7.terminology");
    assert_eq!(tho.manifest().version, "7.3.0");
    assert_eq!(tho.manifest().fhir_versions, vec!["5.0.0".to_owned()]);
    assert!(tho.code_systems().len() > 500);
    assert!(tho.value_sets().len() > 500);
    assert!(tho.operation_definitions().is_empty());
}
