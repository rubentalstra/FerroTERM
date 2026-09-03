use fhir_codegen::closure::TypeClosure;
use fhir_codegen::fhir::StructureKind;
use fhir_codegen::lower::{Cardinality, Target, TypeKind, VersionModule};
use fhir_codegen::roots::RootSet;

use crate::{R4B, R5};

fn closure() -> TypeClosure {
    let roots = RootSet::select(&R4B).expect("root set selects");
    TypeClosure::compute(&R4B, &roots).expect("closure computes")
}

fn model() -> VersionModule {
    VersionModule::lower(&closure(), "r4b", "hl7.fhir.r4b.core", "4.3.0").expect("model lowers")
}

#[test]
fn the_closure_holds_every_primitive_and_the_open_datatypes() {
    let closure = closure();
    let primitives: Vec<&str> = closure
        .of_kind(StructureKind::PrimitiveType)
        .map(|s| s.name.as_str())
        .collect();
    // The R4B primitive types (https://hl7.org/fhir/R4B/datatypes.html#primitive), plus xhtml.
    assert_eq!(
        primitives,
        vec![
            "base64Binary",
            "boolean",
            "canonical",
            "code",
            "date",
            "dateTime",
            "decimal",
            "id",
            "instant",
            "integer",
            "markdown",
            "oid",
            "positiveInt",
            "string",
            "time",
            "unsignedInt",
            "uri",
            "url",
            "uuid",
            "xhtml"
        ]
    );
    for name in [
        "Coding",
        "CodeableConcept",
        "Extension",
        "Meta",
        "Narrative",
        "Reference",
        "Identifier",
        "Dosage",
        "Timing",
        "Duration",
    ] {
        assert!(
            closure.structures().contains_key(name),
            "{name} is in the closure"
        );
    }
    assert_eq!(closure.roots().len(), 8);
}

#[test]
fn the_closure_stops_at_the_root_set() {
    let closure = closure();
    for name in [
        "Patient",
        "Observation",
        "ElementDefinition",
        "Resource",
        "DomainResource",
        "Element",
        "BackboneElement",
    ] {
        assert!(
            !closure.structures().contains_key(name),
            "{name} is outside the closure"
        );
    }
    assert_eq!(closure.structures().len(), 63);
}

#[test]
fn cardinality_maps_to_option_vec_and_direct() {
    let model = model();
    let TypeKind::Struct { fields } = &model.types.get("Coding").expect("Coding is lowered").kind
    else {
        panic!("Coding is a struct");
    };
    let code = fields
        .iter()
        .find(|f| f.name == "code")
        .expect("Coding.code");
    assert_eq!(code.ty.card, Cardinality::Optional);
    assert_eq!(code.ty.target, Target::Named("Code".to_owned()));
    let TypeKind::Struct { fields } = &model
        .types
        .get("ValueSetComposeInclude")
        .expect("include is lowered")
        .kind
    else {
        panic!("include is a struct");
    };
    let concept = fields
        .iter()
        .find(|f| f.name == "concept")
        .expect("include.concept");
    assert_eq!(concept.ty.card, Cardinality::Many);
    let TypeKind::Struct { fields } = &model
        .types
        .get("ValueSetComposeIncludeFilter")
        .expect("filter is lowered")
        .kind
    else {
        panic!("filter is a struct");
    };
    let op = fields.iter().find(|f| f.name == "op").expect("filter.op");
    assert_eq!(op.ty.card, Cardinality::One);
}

#[test]
fn choice_elements_become_enums() {
    let model = model();
    let value = model
        .types
        .get("ParametersParameterValue")
        .expect("choice enum exists");
    let TypeKind::Choice {
        variants,
        element_path,
    } = &value.kind
    else {
        panic!("expected a choice");
    };
    assert_eq!(element_path, "Parameters.parameter.value[x]");
    assert_eq!(variants.len(), 50);
    assert!(
        variants
            .iter()
            .any(|v| v.name == "Coding" && v.code == "Coding")
    );
    let TypeKind::Struct { fields } = &model
        .types
        .get("ParametersParameter")
        .expect("parameter")
        .kind
    else {
        panic!("parameter is a struct");
    };
    let field = fields
        .iter()
        .find(|f| f.name == "value")
        .expect("value field");
    assert_eq!(field.fhir_name, "value[x]");
    assert_eq!(
        field.ty.target,
        Target::Named("ParametersParameterValue".to_owned())
    );
}

#[test]
fn keywords_become_raw_identifiers_and_content_references_reuse_structs() {
    let model = model();
    let TypeKind::Struct { fields } = &model.types.get("ValueSetCompose").expect("compose").kind
    else {
        panic!("compose is a struct");
    };
    let exclude = fields
        .iter()
        .find(|f| f.name == "exclude")
        .expect("exclude");
    assert_eq!(
        exclude.ty.target,
        Target::Named("ValueSetComposeInclude".to_owned())
    );
    let TypeKind::Struct { fields } = &model.types.get("Identifier").expect("Identifier").kind
    else {
        panic!("Identifier is a struct");
    };
    assert!(fields.iter().any(|f| f.name == "r#use"));
    assert!(fields.iter().any(|f| f.name == "r#type"));
}

#[test]
fn the_identifier_reference_cycle_is_boxed() {
    let model = model();
    for (owner, field_name) in [("Identifier", "assigner"), ("Reference", "identifier")] {
        let TypeKind::Struct { fields } = &model.types.get(owner).expect("type exists").kind else {
            panic!("{owner} is a struct");
        };
        let field = fields
            .iter()
            .find(|f| f.name == field_name)
            .expect("field exists");
        assert!(field.ty.boxed, "{owner}.{field_name} is boxed");
    }
    let TypeKind::Struct { fields } = &model.types.get("Coding").expect("Coding").kind else {
        panic!("Coding is a struct");
    };
    assert!(fields.iter().all(|f| !f.ty.boxed));
}

#[test]
fn primitives_keep_element_id_and_extension_and_a_scalar_value() {
    let model = model();
    let boolean = model.types.get("Boolean").expect("Boolean");
    assert!(boolean.is_primitive);
    assert_eq!(boolean.module, "primitives");
    let TypeKind::Struct { fields } = &boolean.kind else {
        panic!("Boolean is a struct");
    };
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["id", "extension", "value"]);
    assert_eq!(
        fields.get(2).map(|f| &f.ty.target),
        Some(&Target::Inline(fhir_codegen::lower::Scalar::Bool))
    );
}

#[test]
fn the_resource_enum_covers_the_root_set() {
    let model = model();
    let TypeKind::ResourceEnum { resources } =
        &model.types.get("Resource").expect("Resource enum").kind
    else {
        panic!("Resource is the enum");
    };
    assert_eq!(
        resources,
        &vec![
            "Bundle",
            "CapabilityStatement",
            "CodeSystem",
            "ConceptMap",
            "OperationOutcome",
            "Parameters",
            "TerminologyCapabilities",
            "ValueSet"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>()
    );
    let TypeKind::Struct { fields } = &model.types.get("BundleEntry").expect("BundleEntry").kind
    else {
        panic!("BundleEntry is a struct");
    };
    let resource = fields
        .iter()
        .find(|f| f.name == "resource")
        .expect("entry.resource");
    assert_eq!(resource.ty.target, Target::Named("Resource".to_owned()));
}

fn r5_model() -> VersionModule {
    let roots = RootSet::select(&R5).expect("R5 root set selects");
    let closure = TypeClosure::compute(&R5, &roots).expect("R5 closure computes");
    VersionModule::lower(&closure, "r5", "hl7.fhir.r5.core", "5.0.0").expect("R5 model lowers")
}

#[test]
fn r5_closes_over_its_own_datatypes() {
    let roots = RootSet::select(&R5).expect("R5 root set selects");
    let closure = TypeClosure::compute(&R5, &roots).expect("R5 closure computes");
    assert_eq!(closure.roots().len(), 8);
    // R5 adds integer64 and new datatypes to the open type set
    // (https://hl7.org/fhir/R5/datatypes.html#open).
    for name in ["integer64", "Availability", "ExtendedContactDetail"] {
        assert!(
            closure.structures().contains_key(name),
            "{name} is in the R5 closure"
        );
    }
    for name in ["Contributor", "MonetaryComponent", "VirtualServiceDetail"] {
        assert!(
            !closure.structures().contains_key(name),
            "{name} is outside the R5 open types and the root set"
        );
    }
    assert_eq!(closure.structures().len(), 65);
}

#[test]
fn r5_integer64_is_an_i64_and_expansion_carries_properties() {
    let model = r5_model();
    let TypeKind::Struct { fields } = &model.types.get("Integer64").expect("Integer64").kind else {
        panic!("Integer64 is a struct");
    };
    assert_eq!(
        fields
            .iter()
            .find(|f| f.name == "value")
            .map(|f| &f.ty.target),
        Some(&Target::Inline(fhir_codegen::lower::Scalar::I64))
    );
    // ValueSet.expansion.property and contains.property are new in R5
    // (https://hl7.org/fhir/R5/valueset.html).
    assert!(model.types.contains_key("ValueSetExpansionProperty"));
    assert!(
        model
            .types
            .contains_key("ValueSetExpansionContainsProperty")
    );
    let TypeKind::Struct { fields } = &model
        .types
        .get("ValueSetExpansionContainsProperty")
        .expect("property")
        .kind
    else {
        panic!("property is a struct");
    };
    let value = fields.iter().find(|f| f.name == "value").expect("value[x]");
    assert_eq!(value.ty.card, Cardinality::One);
}
