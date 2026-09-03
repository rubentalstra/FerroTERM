//! The terminology ecosystem overlay: the parameters every served version
//! carries beyond its own `OperationDefinition`.
//!
//! The HL7 terminology ecosystem requires servers to accept and answer
//! parameters the earlier FHIR versions never declared
//! (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>). Where the
//! R6 ballot declares one, its definition is pre-adopted verbatim from the
//! vendored R6 package; the rest the ecosystem alone defines, and this module
//! declares them. The overlay is applied to a version's definition before
//! lowering, so the generated contract, its descriptor, and its documentation
//! carry the parameters like any declared one, each marked with its source.

use std::fmt;

use crate::fhir::{OperationDefinition, OperationParameter, ParameterUse};

/// The requirements page the overlay rests on.
pub const IG_REQUIREMENTS: &str = "https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html";

/// Where a contract parameter comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParameterSource {
    /// The version's own `OperationDefinition`.
    #[default]
    Version,
    /// Pre-adopted from the FHIR R6 ballot for the terminology ecosystem.
    PreAdopted,
    /// Defined by the terminology ecosystem alone.
    Ecosystem,
}

impl ParameterSource {
    /// The variant name, as the generated descriptor spells it.
    #[must_use]
    pub const fn variant(self) -> &'static str {
        match self {
            Self::Version => "Version",
            Self::PreAdopted => "PreAdopted",
            Self::Ecosystem => "Ecosystem",
        }
    }
}

impl fmt::Display for ParameterSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.variant())
    }
}

const CODE_SYSTEM_LOOKUP: &str = "http://hl7.org/fhir/OperationDefinition/CodeSystem-lookup";
const CODE_SYSTEM_VALIDATE_CODE: &str =
    "http://hl7.org/fhir/OperationDefinition/CodeSystem-validate-code";
const VALUE_SET_VALIDATE_CODE: &str =
    "http://hl7.org/fhir/OperationDefinition/ValueSet-validate-code";
const CONCEPT_MAP_TRANSLATE: &str = "http://hl7.org/fhir/OperationDefinition/ConceptMap-translate";
const VALUE_SET_EXPAND: &str = "http://hl7.org/fhir/OperationDefinition/ValueSet-expand";

/// The R6 parameters pre-adopted into every version that lacks them, by
/// operation URL, direction, and name.
const PRE_ADOPTED: &[(&str, ParameterUse, &str)] = &[
    (VALUE_SET_VALIDATE_CODE, ParameterUse::In, "system-version"),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "check-system-version",
    ),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "force-system-version",
    ),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "default-valueset-version",
    ),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "check-valueset-version",
    ),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "force-valueset-version",
    ),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::In, "inferSystem"),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::In,
        "lenient-display-validation",
    ),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::Out, "code"),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::Out, "system"),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::Out, "version"),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::Out, "issues"),
    (CODE_SYSTEM_VALIDATE_CODE, ParameterUse::Out, "code"),
    (CODE_SYSTEM_VALIDATE_CODE, ParameterUse::Out, "system"),
    (CODE_SYSTEM_VALIDATE_CODE, ParameterUse::Out, "version"),
    (CODE_SYSTEM_VALIDATE_CODE, ParameterUse::Out, "issues"),
    (
        CODE_SYSTEM_VALIDATE_CODE,
        ParameterUse::Out,
        "codeableConcept",
    ),
    (
        VALUE_SET_VALIDATE_CODE,
        ParameterUse::Out,
        "codeableConcept",
    ),
    (CONCEPT_MAP_TRANSLATE, ParameterUse::In, "sourceSystem"),
    (CONCEPT_MAP_TRANSLATE, ParameterUse::In, "sourceVersion"),
    (CODE_SYSTEM_LOOKUP, ParameterUse::In, "useSupplement"),
    (VALUE_SET_VALIDATE_CODE, ParameterUse::In, "useSupplement"),
    (VALUE_SET_EXPAND, ParameterUse::In, "useSupplement"),
    (
        VALUE_SET_EXPAND,
        ParameterUse::In,
        "default-valueset-version",
    ),
    (VALUE_SET_EXPAND, ParameterUse::In, "check-valueset-version"),
    (VALUE_SET_EXPAND, ParameterUse::In, "force-valueset-version"),
];

/// One parameter the overlay added: its direction, name, and source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// `in` or `out`.
    pub usage: ParameterUse,
    /// The parameter name.
    pub name: String,
    /// Where it comes from.
    pub source: ParameterSource,
}

/// A parameter the ecosystem alone defines.
fn ecosystem_parameter(
    name: &str,
    usage: ParameterUse,
    max: &str,
    type_name: &str,
    documentation: &str,
) -> OperationParameter {
    OperationParameter {
        name: name.to_owned(),
        usage,
        min: 0,
        max: max.to_owned(),
        documentation: Some(format!(
            "Defined by the terminology ecosystem (<{IG_REQUIREMENTS}>), declared by no FHIR version. {documentation}"
        )),
        type_name: Some(type_name.to_owned()),
        target_profile: Vec::new(),
        search_type: None,
        binding: None,
        scope: Vec::new(),
        part: Vec::new(),
    }
}

/// The parameters the ecosystem alone defines, by operation URL.
fn ecosystem_parameters(url: &str) -> Vec<OperationParameter> {
    let unknown_system = || {
        ecosystem_parameter(
            "x-caused-by-unknown-system",
            ParameterUse::Out,
            "*",
            "canonical",
            "A code system the server does not serve, one parameter per system, so a validator can tell the user which resources are missing.",
        )
    };
    let unknown_system_of_concept = || {
        ecosystem_parameter(
            "x-unknown-system",
            ParameterUse::Out,
            "*",
            "canonical",
            "A code system a coding of the `codeableConcept` names that the server does not serve, one parameter per system (the ecosystem's twin of `x-caused-by-unknown-system` for that input).",
        )
    };
    match url {
        CODE_SYSTEM_VALIDATE_CODE | VALUE_SET_VALIDATE_CODE => {
            vec![unknown_system(), unknown_system_of_concept()]
        }
        CODE_SYSTEM_LOOKUP => vec![
            ecosystem_parameter(
                "code",
                ParameterUse::Out,
                "1",
                "code",
                "The code that was looked up, as the code system spells it.",
            ),
            ecosystem_parameter(
                "system",
                ParameterUse::Out,
                "1",
                "uri",
                "The code system the code was looked up in.",
            ),
            ecosystem_parameter(
                "abstract",
                ParameterUse::Out,
                "1",
                "boolean",
                "Whether the concept is abstract (`notSelectable`), a grouper that is not itself selected as a value.",
            ),
        ],
        _ => Vec::new(),
    }
}

fn declares(definition: &OperationDefinition, usage: ParameterUse, name: &str) -> bool {
    definition
        .parameter
        .iter()
        .any(|p| p.usage == usage && p.name == name)
}

/// Applies the overlay to `definition`.
///
/// The pre-adopted R6 parameters it lacks come first (taken from `r6`, the R6
/// definition of the same operation), then the ecosystem's own. Returns the
/// overlaid definition and what was added, in order.
#[must_use]
pub fn overlay(
    definition: &OperationDefinition,
    r6: Option<&OperationDefinition>,
) -> (OperationDefinition, Vec<Added>) {
    let mut overlaid = definition.clone();
    let mut added = Vec::new();
    if let Some(r6) = r6 {
        for parameter in &r6.parameter {
            let wanted = PRE_ADOPTED.iter().any(|(url, usage, name)| {
                *url == definition.url && *usage == parameter.usage && *name == parameter.name
            });
            if !wanted || declares(definition, parameter.usage, &parameter.name) {
                continue;
            }
            let mut parameter = parameter.clone();
            parameter.documentation = Some(format!(
                "Pre-adopted from the FHIR R6 ballot for the terminology ecosystem (<{IG_REQUIREMENTS}>). {}",
                parameter.documentation.as_deref().unwrap_or_default()
            ));
            added.push(Added {
                usage: parameter.usage,
                name: parameter.name.clone(),
                source: ParameterSource::PreAdopted,
            });
            overlaid.parameter.push(parameter);
        }
    }
    for parameter in ecosystem_parameters(&definition.url) {
        if declares(definition, parameter.usage, &parameter.name) {
            continue;
        }
        added.push(Added {
            usage: parameter.usage,
            name: parameter.name.clone(),
            source: ParameterSource::Ecosystem,
        });
        overlaid.parameter.push(parameter);
    }
    (overlaid, added)
}
