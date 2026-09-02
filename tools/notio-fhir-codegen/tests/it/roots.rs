use notio_fhir_codegen::fhir::ParameterUse;
use notio_fhir_codegen::roots::{OPERATION_RESOURCES, ROOT_RESOURCES, RootSet};

use crate::R4B;

#[test]
fn the_eight_root_resources_are_found() {
    let roots = RootSet::select(&R4B).expect("root set selects");
    assert_eq!(
        roots.resources.keys().copied().collect::<Vec<_>>(),
        ROOT_RESOURCES.to_vec()
    );
    for (name, definition) in &roots.resources {
        assert_eq!(&definition.name, name);
        assert_eq!(&definition.type_name, name);
    }
}

#[test]
fn exactly_the_terminology_operations_are_selected() {
    let roots = RootSet::select(&R4B).expect("root set selects");
    let ids = roots
        .operations
        .values()
        .map(|operation| format!("{}${}", operation.resource.join(","), operation.code))
        .collect::<Vec<_>>();
    // The R4B operations defined on CodeSystem, ValueSet, and ConceptMap
    // (https://hl7.org/fhir/R4B/terminology-module.html).
    assert_eq!(
        ids,
        vec![
            "CodeSystem$find-matches",
            "CodeSystem$lookup",
            "CodeSystem$subsumes",
            "CodeSystem$validate-code",
            "ConceptMap$closure",
            "ConceptMap$translate",
            "ValueSet$expand",
            "ValueSet$validate-code",
        ]
    );
    for operation in roots.operations.values() {
        for resource in &operation.resource {
            assert!(OPERATION_RESOURCES.contains(&resource.as_str()));
        }
    }
}

#[test]
fn operations_are_looked_up_by_resource_and_code() {
    let roots = RootSet::select(&R4B).expect("root set selects");
    let lookup = roots
        .operation("CodeSystem", "lookup")
        .expect("$lookup exists");
    assert_eq!(
        lookup.url,
        "http://hl7.org/fhir/OperationDefinition/CodeSystem-lookup"
    );
    let outputs = lookup
        .parameter
        .iter()
        .filter(|parameter| parameter.usage == ParameterUse::Out)
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    // The R4B $lookup out parameters
    // (https://hl7.org/fhir/R4B/codesystem-operation-lookup.html).
    assert_eq!(
        outputs,
        vec!["name", "version", "display", "designation", "property"]
    );
    let property = lookup
        .parameter
        .iter()
        .find(|parameter| parameter.name == "property" && parameter.usage == ParameterUse::Out)
        .expect("the property out parameter exists");
    assert_eq!(
        property
            .part
            .iter()
            .map(|part| part.name.as_str())
            .collect::<Vec<_>>(),
        vec!["code", "value", "description", "subproperty"]
    );
    assert!(roots.operation("ValueSet", "lookup").is_none());
    assert!(roots.operation("Resource", "validate").is_none());
}
