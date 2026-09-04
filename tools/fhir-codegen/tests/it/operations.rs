use fhir_codegen::closure::TypeClosure;
use fhir_codegen::fhir::ParameterUse;
use fhir_codegen::lower::VersionModule;
use fhir_codegen::operations::OperationContract;
use fhir_codegen::package::Package;
use fhir_codegen::roots::RootSet;

use crate::{R4, R4B, R5, R6};

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
    for (package, module) in [(&*R4, "r4"), (&*R4B, "r4b"), (&*R5, "r5"), (&*R6, "r6")] {
        let contracts = contracts(package, module);
        let modules: Vec<&str> = contracts.iter().map(|c| c.module.as_str()).collect();
        // The R6 ballot5 core package no longer publishes CodeSystem/$find-matches
        // and ConceptMap/$closure; every earlier version does.
        let expected: Vec<&str> = if module == "r6" {
            vec![
                "code_system_lookup",
                "code_system_subsumes",
                "code_system_validate_code",
                "concept_map_translate",
                "value_set_expand",
                "value_set_validate_code",
            ]
        } else {
            vec![
                "code_system_find_matches",
                "code_system_lookup",
                "code_system_subsumes",
                "code_system_validate_code",
                "concept_map_closure",
                "concept_map_translate",
                "value_set_expand",
                "value_set_validate_code",
            ]
        };
        assert_eq!(modules, expected, "{module}");
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
fn a_primitive_parameter_reads_the_primitives_that_specialize_it() {
    for (package, module) in [(&*R4B, "r4b"), (&*R5, "r5")] {
        let contracts = contracts(package, module);
        let expand = find(&contracts, "ValueSet", "expand");
        let accepts = |name: &str| -> Vec<String> {
            let mut list = expand
                .inputs
                .iter()
                .find(|f| f.fhir_name == name)
                .expect(name)
                .accepts
                .clone();
            list.sort();
            list
        };
        // `code`, `id`, and `markdown` specialize `string`; `canonical`, `oid`,
        // `url`, and `uuid` specialize `uri`
        // (<https://hl7.org/fhir/R5/datatypes.html#primitive>).
        assert_eq!(accepts("filter"), ["Code", "Id", "Markdown"], "{module}");
        assert_eq!(
            accepts("url"),
            ["Canonical", "Oid", "Url", "Uuid"],
            "{module}"
        );
        // An integer parameter takes nothing: `positiveInt` has another scalar.
        assert!(accepts("count").is_empty(), "{module}");
        assert!(
            accepts("includeDesignations").is_empty(),
            "{module}: boolean"
        );
    }
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

mod conversions {
    //! The generated `Parameters` conversions: exact to the declared set.

    use fhir_types::operation::ParametersError;
    use fhir_types::r4b::coding::Coding;
    use fhir_types::r4b::operations::code_system_lookup::{
        CodeSystemLookupResponse, CodeSystemLookupResponseDesignation,
        CodeSystemLookupResponseProperty,
    };
    use fhir_types::r4b::operations::code_system_validate_code::CodeSystemValidateCodeRequest;
    use fhir_types::r4b::parameters::{Parameters, ParametersParameter, ParametersParameterValue};

    fn param(name: &str, value: ParametersParameterValue) -> ParametersParameter {
        ParametersParameter {
            name: name.into(),
            value: Some(value),
            ..Default::default()
        }
    }

    #[test]
    fn a_lookup_response_round_trips_through_parameters() {
        let response = CodeSystemLookupResponse {
            name: "SNOMED CT".into(),
            version: Some("http://snomed.info/sct/900000000000207008/version/20260101".into()),
            display: "Heart".into(),
            designation: vec![CodeSystemLookupResponseDesignation {
                language: Some("nl".into()),
                r#use: Some(Coding {
                    system: Some("http://snomed.info/sct".into()),
                    code: Some("900000000000013009".into()),
                    ..Default::default()
                }),
                value: "Hart".into(),
            }],
            property: vec![CodeSystemLookupResponseProperty {
                code: "inactive".into(),
                value: Some(ParametersParameterValue::Boolean(false.into())),
                description: None,
                subproperty: Vec::new(),
            }],
            code: None,
            system: None,
            r#abstract: None,
        };
        let parameters = response.to_parameters();
        let names: Vec<&str> = parameters
            .parameter
            .iter()
            .filter_map(|p| p.name.value.as_deref())
            .collect();
        assert_eq!(
            names,
            ["name", "version", "display", "designation", "property"]
        );
        assert_eq!(
            parameters.parameter[3].part.len(),
            3,
            "language, use, value"
        );
        let back = CodeSystemLookupResponse::from_parameters(&parameters).expect("reads back");
        assert_eq!(back, response);
    }

    #[test]
    fn r4b_validate_code_refuses_the_undeclared_system_parameter() {
        // The R4B definition declares `url`, not `system`, although its prose
        // says "code+system": strict means exactly the declared set.
        let parameters = Parameters {
            parameter: vec![
                param(
                    "system",
                    ParametersParameterValue::Uri("http://snomed.info/sct".into()),
                ),
                param("code", ParametersParameterValue::Code("80146002".into())),
            ],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemValidateCodeRequest::from_parameters(&parameters),
            Err(ParametersError::Undeclared {
                operation: "CodeSystem/$validate-code",
                name: String::from("system"),
            })
        );
    }

    #[test]
    fn repeated_missing_unnamed_and_wrongly_typed_parameters_are_refused() {
        let twice = Parameters {
            parameter: vec![
                param("code", ParametersParameterValue::Code("1".into())),
                param("code", ParametersParameterValue::Code("2".into())),
            ],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemValidateCodeRequest::from_parameters(&twice),
            Err(ParametersError::Repeated {
                operation: "CodeSystem/$validate-code",
                name: "code",
            })
        );
        let missing = Parameters {
            parameter: vec![param(
                "display",
                ParametersParameterValue::String("Heart".into()),
            )],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemLookupResponse::from_parameters(&missing),
            Err(ParametersError::Missing {
                operation: "CodeSystem/$lookup",
                name: "name",
            })
        );
        let unnamed = Parameters {
            parameter: vec![ParametersParameter::default()],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemValidateCodeRequest::from_parameters(&unnamed),
            Err(ParametersError::Unnamed {
                operation: "CodeSystem/$validate-code",
            })
        );
        let wrong = Parameters {
            parameter: vec![param(
                "code",
                ParametersParameterValue::Boolean(true.into()),
            )],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemValidateCodeRequest::from_parameters(&wrong),
            Err(ParametersError::WrongType {
                operation: "CodeSystem/$validate-code",
                name: "code",
                expected: "Code",
            })
        );
        let valueless = Parameters {
            parameter: vec![ParametersParameter {
                name: "code".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemValidateCodeRequest::from_parameters(&valueless),
            Err(ParametersError::MissingValue {
                operation: "CodeSystem/$validate-code",
                name: "code",
            })
        );
    }

    #[test]
    fn a_part_error_names_the_dotted_path() {
        let parameters = Parameters {
            parameter: vec![
                param("name", ParametersParameterValue::String("x".into())),
                param("display", ParametersParameterValue::String("y".into())),
                ParametersParameter {
                    name: "designation".into(),
                    part: vec![param(
                        "colour",
                        ParametersParameterValue::String("red".into()),
                    )],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            CodeSystemLookupResponse::from_parameters(&parameters),
            Err(ParametersError::Undeclared {
                operation: "CodeSystem/$lookup",
                name: String::from("designation.colour"),
            })
        );
    }
}

/// The contracts of `package` with the terminology ecosystem overlay applied,
/// the R6 package as the source of the pre-adopted parameters.
fn overlaid(package: &Package, module: &str) -> Vec<OperationContract> {
    let roots = RootSet::select(package).expect("root set selects");
    let r6_roots = RootSet::select(&R6).expect("the R6 root set selects");
    let closure = TypeClosure::compute(package, &roots).expect("closure computes");
    let model = VersionModule::lower(&closure, module, "pkg", "0").expect("model lowers");
    let mut contracts = Vec::new();
    for (url, operation) in &roots.operations {
        let source = r6_roots.operations.get(url).copied();
        for resource in &operation.resource {
            contracts.push(
                OperationContract::lower_overlaid(operation, resource, &model, source)
                    .expect("contract lowers"),
            );
        }
    }
    contracts
}

#[test]
fn the_overlay_pre_adopts_the_r6_parameters_every_earlier_version_lacks() {
    use fhir_codegen::ecosystem::ParameterSource;
    for (package, module) in [(&*R4, "r4"), (&*R4B, "r4b"), (&*R5, "r5")] {
        let contracts = overlaid(package, module);
        let validate = find(&contracts, "ValueSet", "validate-code");
        let pre_adopted: Vec<&str> = validate
            .inputs
            .iter()
            .filter(|f| f.source == ParameterSource::PreAdopted)
            .map(|f| f.fhir_name.as_str())
            .collect();
        // R5 declares `useSupplement` itself; the R4 family pre-adopts it.
        let mut expected_inputs = vec![
            "lenient-display-validation",
            "valueset-membership-only",
            "inferSystem",
            "system-version",
            "check-system-version",
            "force-system-version",
            "default-valueset-version",
            "check-valueset-version",
            "force-valueset-version",
        ];
        if module != "r5" {
            expected_inputs.insert(0, "useSupplement");
        }
        assert_eq!(pre_adopted, expected_inputs, "{module}: the R6 order");
        let outputs: Vec<(&str, ParameterSource)> = validate
            .outputs
            .iter()
            .map(|f| (f.fhir_name.as_str(), f.source))
            .collect();
        let expected_source = if module == "r5" {
            ParameterSource::Version
        } else {
            ParameterSource::PreAdopted
        };
        for name in ["code", "system", "version", "issues"] {
            assert!(
                outputs.contains(&(name, expected_source)),
                "{module}: {name} {outputs:?}"
            );
        }
        assert!(outputs.contains(&("x-caused-by-unknown-system", ParameterSource::Ecosystem)));
        let documented = validate
            .inputs
            .iter()
            .find(|f| f.fhir_name == "inferSystem")
            .expect("inferSystem");
        assert!(
            documented
                .documentation
                .as_deref()
                .unwrap()
                .starts_with("Pre-adopted from the FHIR R6 ballot"),
            "{module}: {:?}",
            documented.documentation
        );
        // $expand pre-adopts the value set version trio, and `property` where the
        // version lacks it (the ecosystem requires it on every version); $subsumes
        // gets nothing.
        let expand = find(&contracts, "ValueSet", "expand");
        let adopted: Vec<&str> = expand
            .inputs
            .iter()
            .chain(&expand.outputs)
            .filter(|f| f.source != ParameterSource::Version)
            .map(|f| f.fhir_name.as_str())
            .collect();
        let mut expected_expand = vec![
            "default-valueset-version",
            "check-valueset-version",
            "force-valueset-version",
        ];
        if module != "r5" {
            expected_expand.insert(0, "useSupplement");
            expected_expand.insert(1, "property");
        }
        assert_eq!(adopted, expected_expand, "{module}");
        let subsumes = find(&contracts, "CodeSystem", "subsumes");
        assert!(
            subsumes
                .inputs
                .iter()
                .chain(&subsumes.outputs)
                .all(|f| f.source == ParameterSource::Version),
            "{module}: CodeSystem/$subsumes"
        );
    }
}

#[test]
fn the_overlay_pre_adopts_the_r6_translate_inputs() {
    use fhir_codegen::ecosystem::ParameterSource;
    for (package, module) in [(&*R4, "r4"), (&*R4B, "r4b"), (&*R5, "r5")] {
        let contracts = overlaid(package, module);
        let translate = find(&contracts, "ConceptMap", "translate");
        let adopted: Vec<&str> = translate
            .inputs
            .iter()
            .filter(|f| f.source == ParameterSource::PreAdopted)
            .map(|f| f.fhir_name.as_str())
            .collect();
        // R5 declares the source*/target* names itself; the R4 family pre-adopts
        // them all from R6, in the R6 order.
        // R5 also takes the R6 types for `targetCode`, `targetCoding`, and
        // `targetCodeableConcept`, which it declares as `uri`.
        let expected_translate = if module == "r5" {
            vec![
                "targetCode",
                "targetCoding",
                "targetCodeableConcept",
                "sourceSystem",
                "sourceVersion",
            ]
        } else {
            vec![
                "sourceCode",
                "sourceSystem",
                "sourceVersion",
                "sourceScope",
                "sourceCoding",
                "sourceCodeableConcept",
                "targetCode",
                "targetCoding",
                "targetCodeableConcept",
                "targetScope",
                "targetSystem",
            ]
        };
        assert_eq!(adopted, expected_translate, "{module}");
    }
}

#[test]
fn the_translate_match_carries_the_ecosystem_parts_and_pre_adopts_origin_map() {
    use fhir_codegen::ecosystem::ParameterSource;
    for (package, module) in [(&*R4, "r4"), (&*R4B, "r4b"), (&*R5, "r5"), (&*R6, "r6")] {
        let contracts = overlaid(package, module);
        let translate = find(&contracts, "ConceptMap", "translate");
        let matched = translate
            .outputs
            .iter()
            .find(|f| f.fhir_name == "match")
            .expect("match");
        let part = |name: &str| {
            matched
                .parts
                .iter()
                .find(|p| p.fhir_name == name)
                .map(|p| (p.type_code.as_deref(), p.source))
        };
        // R5 declares `originMap` as a uri and takes R6's canonical; the R4
        // family pre-adopts the R6 part, and answers its own `source` as a
        // canonical, the ecosystem's type.
        let origin_source = if module == "r6" {
            ParameterSource::Version
        } else {
            ParameterSource::PreAdopted
        };
        assert_eq!(
            part("originMap"),
            Some((Some("canonical"), origin_source)),
            "{module}"
        );
        if module == "r4" || module == "r4b" {
            assert_eq!(
                part("source"),
                Some((Some("canonical"), ParameterSource::Ecosystem)),
                "{module}"
            );
        }
        for (name, type_code) in [
            ("sourceConcept", "Coding"),
            ("sourceComment", "string"),
            ("targetComment", "string"),
            ("noMap", "boolean"),
        ] {
            assert_eq!(
                part(name),
                Some((Some(type_code), ParameterSource::Ecosystem)),
                "{module}: {name}"
            );
        }
        for name in ["used-conceptmap", "used-system"] {
            let field = translate
                .outputs
                .iter()
                .find(|f| f.fhir_name == name)
                .expect(name);
            assert_eq!(
                (field.type_code.as_deref(), field.source),
                (Some("uri"), ParameterSource::Ecosystem),
                "{module}: {name}"
            );
        }
    }
}

#[test]
fn the_ecosystem_defined_outputs_join_every_version_and_r6_pre_adopts_nothing() {
    use fhir_codegen::ecosystem::ParameterSource;
    for (package, module) in [(&*R4, "r4"), (&*R4B, "r4b"), (&*R5, "r5"), (&*R6, "r6")] {
        let contracts = overlaid(package, module);
        let lookup = find(&contracts, "CodeSystem", "lookup");
        let ecosystem: Vec<(&str, Option<&str>)> = lookup
            .outputs
            .iter()
            .filter(|f| f.source == ParameterSource::Ecosystem)
            .map(|f| (f.fhir_name.as_str(), f.type_code.as_deref()))
            .collect();
        assert_eq!(
            ecosystem,
            [
                ("code", Some("code")),
                ("system", Some("uri")),
                ("abstract", Some("boolean"))
            ],
            "{module}"
        );
        for (resource, code) in [
            ("CodeSystem", "validate-code"),
            ("ValueSet", "validate-code"),
        ] {
            let contract = find(&contracts, resource, code);
            let unknown = contract
                .outputs
                .iter()
                .find(|f| f.fhir_name == "x-caused-by-unknown-system")
                .expect("x-caused-by-unknown-system");
            assert_eq!(unknown.source, ParameterSource::Ecosystem);
            assert_eq!(unknown.type_code.as_deref(), Some("canonical"));
            assert!(
                unknown
                    .documentation
                    .as_deref()
                    .unwrap()
                    .starts_with("Defined by the terminology ecosystem"),
                "{module}"
            );
        }
        if module == "r6" {
            let pre_adopted = contracts
                .iter()
                .flat_map(|c| c.inputs.iter().chain(&c.outputs))
                .filter(|f| f.source == ParameterSource::PreAdopted)
                .count();
            assert_eq!(pre_adopted, 0, "R6 is the source, it pre-adopts nothing");
        }
    }
}
