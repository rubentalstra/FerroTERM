use ferroterm_fhir_codegen::closure::TypeClosure;
use ferroterm_fhir_codegen::fhir::ParameterUse;
use ferroterm_fhir_codegen::lower::VersionModule;
use ferroterm_fhir_codegen::operations::OperationContract;
use ferroterm_fhir_codegen::package::Package;
use ferroterm_fhir_codegen::roots::RootSet;

use crate::{R4B, R5};

fn contracts(package: &Package, module: &str) -> Vec<OperationContract> {
    let roots = RootSet::select(package).expect("root set selects");
    let closure = TypeClosure::compute(package, &roots).expect("closure computes");
    let model = VersionModule::lower(&closure, module, "pkg", "0").expect("model lowers");
    let mut contracts: Vec<OperationContract> = roots
        .operations
        .values()
        .flat_map(|operation| {
            operation.resource.iter().map(|resource| {
                OperationContract::lower(operation, resource, &model).expect("contract lowers")
            })
        })
        .collect();
    contracts.sort_by(|a, b| a.module.cmp(&b.module));
    contracts
}

fn find<'a>(
    contracts: &'a [OperationContract],
    resource: &str,
    code: &str,
) -> &'a OperationContract {
    contracts
        .iter()
        .find(|c| c.resource == resource && c.code == code)
        .expect("operation exists")
}

#[test]
fn every_terminology_operation_gets_a_contract_in_both_versions() {
    for (package, module) in [(&*R4B, "r4b"), (&*R5, "r5")] {
        let contracts = contracts(package, module);
        let modules: Vec<&str> = contracts.iter().map(|c| c.module.as_str()).collect();
        assert_eq!(
            modules,
            vec![
                "code_system_find_matches",
                "code_system_lookup",
                "code_system_subsumes",
                "code_system_validate_code",
                "concept_map_closure",
                "concept_map_translate",
                "value_set_expand",
                "value_set_validate_code",
            ],
            "{module}"
        );
        let lookup = find(&contracts, "CodeSystem", "lookup");
        assert_eq!(lookup.request, "CodeSystemLookupRequest");
        assert_eq!(lookup.response, "CodeSystemLookupResponse");
        assert_eq!(lookup.descriptor, "CODE_SYSTEM_LOOKUP");
        assert!(lookup.type_level && !lookup.system);
    }
}

#[test]
fn the_expand_request_is_exactly_what_each_version_declares() {
    // R4B $expand in parameters (https://hl7.org/fhir/R4B/valueset-operation-expand.html).
    let r4b = contracts(&R4B, "r4b");
    let expand = find(&r4b, "ValueSet", "expand");
    let inputs: Vec<&str> = expand.inputs.iter().map(|f| f.fhir_name.as_str()).collect();
    assert_eq!(
        inputs,
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
        ]
    );
    assert!(
        inputs
            .iter()
            .all(|name| *name != "useSupplement" && *name != "property")
    );
    assert_eq!(
        expand
            .outputs
            .iter()
            .map(|f| f.fhir_name.as_str())
            .collect::<Vec<_>>(),
        vec!["return"]
    );
    let exclude = expand
        .inputs
        .iter()
        .find(|f| f.fhir_name == "exclude-system")
        .expect("exclude-system");
    assert_eq!(exclude.name, "exclude_system");
    assert_eq!(exclude.rust_type, "super::super::primitives::Canonical");

    // R5 adds useSupplement and property (https://hl7.org/fhir/R5/valueset-operation-expand.html).
    let r5 = contracts(&R5, "r5");
    let expand = find(&r5, "ValueSet", "expand");
    let inputs: Vec<&str> = expand.inputs.iter().map(|f| f.fhir_name.as_str()).collect();
    assert!(inputs.contains(&"useSupplement"));
    assert!(inputs.contains(&"property"));
    let url = expand
        .inputs
        .iter()
        .find(|f| f.fhir_name == "url")
        .expect("url");
    assert_eq!(url.scope, vec!["type".to_owned()]);
}

#[test]
fn multi_part_parameters_nest_and_element_maps_to_the_open_type() {
    let r4b = contracts(&R4B, "r4b");
    let lookup = find(&r4b, "CodeSystem", "lookup");
    let property = lookup
        .outputs
        .iter()
        .find(|f| f.fhir_name == "property" && f.usage == ParameterUse::Out)
        .expect("property out");
    assert_eq!(
        property.part_struct.as_deref(),
        Some("CodeSystemLookupResponseProperty")
    );
    assert_eq!(property.rust_type, "CodeSystemLookupResponseProperty");
    let names: Vec<&str> = property
        .parts
        .iter()
        .map(|p| p.fhir_name.as_str())
        .collect();
    assert_eq!(names, vec!["code", "value", "description", "subproperty"]);
    let value = property
        .parts
        .iter()
        .find(|p| p.fhir_name == "value")
        .expect("value");
    assert_eq!(value.type_code.as_deref(), Some("Element"));
    assert_eq!(
        value.rust_type,
        "super::super::parameters::ParametersParameterValue"
    );
    let subproperty = property
        .parts
        .iter()
        .find(|p| p.fhir_name == "subproperty")
        .expect("subproperty");
    assert_eq!(
        subproperty.part_struct.as_deref(),
        Some("CodeSystemLookupResponsePropertySubproperty")
    );
    assert_eq!(subproperty.parts.len(), 3);

    let r5 = contracts(&R5, "r5");
    let lookup = find(&r5, "CodeSystem", "lookup");
    let property = lookup
        .outputs
        .iter()
        .find(|f| f.fhir_name == "property" && f.usage == ParameterUse::Out)
        .expect("property out");
    let names: Vec<&str> = property
        .parts
        .iter()
        .map(|p| p.fhir_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["code", "value", "description", "source", "subproperty"]
    );
}

#[test]
fn resource_typed_parameters_use_the_root_set_types() {
    let r4b = contracts(&R4B, "r4b");
    let validate = find(&r4b, "ValueSet", "validate-code");
    let value_set = validate
        .inputs
        .iter()
        .find(|f| f.fhir_name == "valueSet")
        .expect("valueSet");
    assert_eq!(value_set.rust_type, "super::super::value_set::ValueSet");
    assert_eq!(validate.request, "ValueSetValidateCodeRequest");
    let result = validate
        .outputs
        .iter()
        .find(|f| f.fhir_name == "result")
        .expect("result");
    assert_eq!(result.min, 1);
    let r5 = contracts(&R5, "r5");
    let validate = find(&r5, "ValueSet", "validate-code");
    let issues = validate
        .outputs
        .iter()
        .find(|f| f.fhir_name == "issues")
        .expect("R5 issues");
    assert_eq!(
        issues.rust_type,
        "super::super::operation_outcome::OperationOutcome"
    );
}
