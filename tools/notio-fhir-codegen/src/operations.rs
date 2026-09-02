//! Operation contracts: the terminology `OperationDefinition`s as typed
//! request and response shapes plus a runtime descriptor.
//!
//! Each terminology operation of a version yields a request struct (its `in`
//! parameters), a response struct (its `out` parameters), one struct per
//! multi-part parameter, and a `const` describing the exact parameter set the
//! version declares (<https://hl7.org/fhir/R4B/operationdefinition.html>), so
//! a server can refuse what the version does not define
//! (`spec-adherence.md`).

use std::fmt::{self, Write};

use crate::fhir::{OperationDefinition, OperationParameter, ParameterUse};
use crate::lower::{Cardinality, VersionModule};
use crate::naming::{field_name, module_name, type_name};
use crate::snapshot::Max;

/// The module holding the shared descriptor types.
pub const DESCRIPTOR_MODULE: &str = "operation";
/// The choice enum every `Element`-typed parameter maps to.
pub const OPEN_TYPE_ENUM: &str = "ParametersParameterValue";

/// A failure while lowering an operation.
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    /// A parameter has neither a type nor parts.
    #[error("{operation}: parameter {name} has neither a type nor parts")]
    Untyped {
        /// The operation code.
        operation: String,
        /// The parameter name.
        name: String,
    },
    /// A parameter names a type the version module does not emit.
    #[error("{operation}: parameter {name} has type {code}, which the module does not emit")]
    UnknownType {
        /// The operation code.
        operation: String,
        /// The parameter name.
        name: String,
        /// The type code.
        code: String,
    },
    /// A parameter's `max` is neither a number nor `*`.
    #[error("{operation}: parameter {name} has an invalid max {max:?}")]
    InvalidMax {
        /// The operation code.
        operation: String,
        /// The parameter name.
        name: String,
        /// The offending value.
        max: String,
    },
    /// Rendering to a string failed.
    #[error("rendering failed")]
    Render(#[from] fmt::Error),
}

/// One typed parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractField {
    /// The Rust field name.
    pub name: String,
    /// The FHIR parameter name.
    pub fhir_name: String,
    /// The parameter documentation.
    pub documentation: Option<String>,
    /// `in` or `out`.
    pub usage: ParameterUse,
    /// The minimum cardinality.
    pub min: u32,
    /// The maximum cardinality.
    pub max: Max,
    /// The FHIR type code, absent for a multi-part parameter.
    pub type_code: Option<String>,
    /// The Rust type path the field holds (without `Option`/`Vec`).
    pub rust_type: String,
    /// The invocation levels the parameter applies to (R5 `scope`).
    pub scope: Vec<String>,
    /// The nested parts, lowered.
    pub parts: Vec<ContractField>,
    /// The struct name of the nested parts, when there are any.
    pub part_struct: Option<String>,
}

/// One operation's contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationContract {
    /// The resource the operation applies to, for example `CodeSystem`.
    pub resource: String,
    /// The operation code, for example `lookup`.
    pub code: String,
    /// The canonical URL of the `OperationDefinition`.
    pub url: String,
    /// The operation's own documentation.
    pub description: Option<String>,
    /// The request struct name.
    pub request: String,
    /// The response struct name.
    pub response: String,
    /// The descriptor constant name.
    pub descriptor: String,
    /// The module (file) name.
    pub module: String,
    /// Whether the operation is invoked at the system level.
    pub system: bool,
    /// Whether the operation is invoked at the type level.
    pub type_level: bool,
    /// Whether the operation is invoked on an instance.
    pub instance: bool,
    /// The `in` parameters.
    pub inputs: Vec<ContractField>,
    /// The `out` parameters.
    pub outputs: Vec<ContractField>,
}

impl OperationContract {
    /// Lowers `definition` against the types `module` emits.
    ///
    /// # Errors
    ///
    /// Returns [`OperationError`] for a parameter without type or parts, a
    /// type the module does not emit, or an invalid `max`.
    pub fn lower(
        definition: &OperationDefinition,
        resource: &str,
        module: &VersionModule,
    ) -> Result<Self, OperationError> {
        let stem = format!("{}{}", type_name(resource), pascal(&definition.code));
        let request = format!("{stem}Request");
        let response = format!("{stem}Response");
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for parameter in &definition.parameter {
            let owner = match parameter.usage {
                ParameterUse::In => &request,
                ParameterUse::Out => &response,
            };
            let field = lower_field(parameter, owner, &definition.code, module)?;
            match parameter.usage {
                ParameterUse::In => inputs.push(field),
                ParameterUse::Out => outputs.push(field),
            }
        }
        Ok(Self {
            resource: resource.to_owned(),
            code: definition.code.clone(),
            url: definition.url.clone(),
            description: definition.description.clone(),
            request,
            response,
            descriptor: module_name(&stem).to_uppercase(),
            module: module_name(&stem),
            system: definition.system,
            type_level: definition.type_level,
            instance: definition.instance,
            inputs,
            outputs,
        })
    }
}

fn lower_field(
    parameter: &OperationParameter,
    owner: &str,
    operation: &str,
    module: &VersionModule,
) -> Result<ContractField, OperationError> {
    let max = parse_max(&parameter.max).ok_or_else(|| OperationError::InvalidMax {
        operation: operation.to_owned(),
        name: parameter.name.clone(),
        max: parameter.max.clone(),
    })?;
    let part_struct =
        (!parameter.part.is_empty()).then(|| format!("{owner}{}", pascal(&parameter.name)));
    let rust_type = match (&parameter.type_name, &part_struct) {
        (_, Some(nested)) => nested.clone(),
        (Some(code), None) if code == "Element" => {
            format!("super::super::parameters::{OPEN_TYPE_ENUM}")
        }
        (Some(code), None) => {
            let name = type_name(code);
            let ty = module
                .types
                .get(&name)
                .ok_or_else(|| OperationError::UnknownType {
                    operation: operation.to_owned(),
                    name: parameter.name.clone(),
                    code: code.clone(),
                })?;
            format!("super::super::{}::{name}", ty.module)
        }
        (None, None) => {
            return Err(OperationError::Untyped {
                operation: operation.to_owned(),
                name: parameter.name.clone(),
            });
        }
    };
    let mut parts = Vec::new();
    if let Some(nested) = &part_struct {
        for part in &parameter.part {
            parts.push(lower_field(part, nested, operation, module)?);
        }
    }
    Ok(ContractField {
        name: field_name(&parameter.name),
        fhir_name: parameter.name.clone(),
        documentation: parameter.documentation.clone(),
        usage: parameter.usage,
        min: parameter.min,
        max,
        type_code: parameter.type_name.clone(),
        rust_type,
        scope: parameter.scope.clone(),
        parts,
        part_struct,
    })
}

fn parse_max(max: &str) -> Option<Max> {
    if max == "*" {
        Some(Max::Unbounded)
    } else {
        max.parse().ok().map(Max::Bounded)
    }
}

/// `validate-code` to `ValidateCode`, `find-matches` to `FindMatches`.
fn pascal(code: &str) -> String {
    code.split(['-', '_']).map(type_name).collect()
}

fn cardinality(min: u32, max: Max) -> Cardinality {
    match (min, max) {
        (_, Max::Unbounded) => Cardinality::Many,
        (_, Max::Bounded(n)) if n > 1 => Cardinality::Many,
        (0, _) => Cardinality::Optional,
        (_, _) => Cardinality::One,
    }
}

/// The version-neutral descriptor module, `src/operation.rs`.
#[must_use]
pub fn render_descriptor_module(banner: &str) -> String {
    let mut out = banner.to_owned();
    out.push_str(
        r"//! Runtime descriptors of the terminology operations.
//!
//! The exact parameter set each FHIR version declares, so a server accepts
//! nothing more and nothing less
//! (<https://hl7.org/fhir/R4B/operationdefinition.html>).

/// The direction of an operation parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterUse {
    /// An input parameter.
    In,
    /// An output parameter.
    Out,
}

/// How many values a parameter takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cardinality {
    /// The minimum number of values.
    pub min: u32,
    /// The maximum number of values; `None` is unbounded (`*`).
    pub max: Option<u32>,
}

impl Cardinality {
    /// Whether `count` values satisfy this cardinality.
    #[must_use]
    pub const fn admits(self, count: u32) -> bool {
        count >= self.min
            && match self.max {
                Some(max) => count <= max,
                None => true,
            }
    }
}

/// One declared parameter, with its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    /// The parameter name as it appears in `Parameters.parameter.name`.
    pub name: &'static str,
    /// The direction.
    pub usage: ParameterUse,
    /// The cardinality.
    pub cardinality: Cardinality,
    /// The FHIR type code, `None` for a multi-part parameter.
    pub type_code: Option<&'static str>,
    /// The invocation levels the parameter applies to; empty means every level.
    pub scope: &'static [&'static str],
    /// The nested parts.
    pub parts: &'static [Parameter],
}

impl Parameter {
    /// The part named `name`, if any.
    #[must_use]
    pub fn part(&self, name: &str) -> Option<&'static Parameter> {
        self.parts.iter().find(|part| part.name == name)
    }
}

/// One operation as its version declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation {
    /// The canonical URL of the `OperationDefinition`.
    pub url: &'static str,
    /// The resource type the operation applies to.
    pub resource: &'static str,
    /// The code used to invoke the operation, without the `$`.
    pub code: &'static str,
    /// Whether the operation is invoked at the system level.
    pub system: bool,
    /// Whether the operation is invoked at the resource type level.
    pub type_level: bool,
    /// Whether the operation is invoked on a resource instance.
    pub instance: bool,
    /// Every declared parameter, `in` and `out`, in declaration order.
    pub parameters: &'static [Parameter],
}

impl Operation {
    /// The parameter named `name` with the given direction, if declared.
    #[must_use]
    pub fn parameter(&self, usage: ParameterUse, name: &str) -> Option<&'static Parameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.usage == usage && parameter.name == name)
    }

    /// The parameters of one direction, in declaration order.
    pub fn parameters_of(&self, usage: ParameterUse) -> impl Iterator<Item = &'static Parameter> {
        self.parameters.iter().filter(move |parameter| parameter.usage == usage)
    }
}
",
    );
    out
}

/// The `operations/mod.rs` of a version module.
///
/// # Errors
///
/// Returns [`fmt::Error`] only if writing to the string fails, which `String` never does.
pub fn render_operations_mod(
    banner: &str,
    version: &str,
    contracts: &[OperationContract],
) -> Result<String, fmt::Error> {
    let mut out = banner.to_owned();
    writeln!(
        out,
        "//! The FHIR {version} terminology operations: typed request and response\n//! shapes and the descriptor of each operation's declared parameter set.\n"
    )?;
    for contract in contracts {
        writeln!(out, "pub mod {};", contract.module)?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "/// Every terminology operation this version declares, in module order."
    )?;
    writeln!(
        out,
        "pub const OPERATIONS: [&super::super::{DESCRIPTOR_MODULE}::Operation; {}] = [",
        contracts.len()
    )?;
    for contract in contracts {
        writeln!(out, "    &{}::{},", contract.module, contract.descriptor)?;
    }
    writeln!(out, "];")?;
    Ok(out)
}

/// One operation's file.
///
/// # Errors
///
/// Returns [`fmt::Error`] only if writing to the string fails, which `String` never does.
pub fn render_operation(banner: &str, contract: &OperationContract) -> Result<String, fmt::Error> {
    let mut out = banner.to_owned();
    writeln!(out, "//! `{}/${}`.", contract.resource, contract.code)?;
    writeln!(out, "//!")?;
    let description = contract.description.as_deref().map_or_else(
        || String::from("(undocumented in the package)"),
        |d| crate::render::escape_doc(&collapse_whitespace(d)),
    );
    for line in wrap(&description) {
        writeln!(out, "//! {line}")?;
    }
    writeln!(out)?;
    render_struct(
        &mut out,
        &contract.request,
        &format!(
            "The `in` parameters of `{}/${}`.",
            contract.resource, contract.code
        ),
        &contract.inputs,
    )?;
    render_struct(
        &mut out,
        &contract.response,
        &format!(
            "The `out` parameters of `{}/${}`.",
            contract.resource, contract.code
        ),
        &contract.outputs,
    )?;
    render_descriptor(&mut out, contract)?;
    Ok(out)
}

fn render_struct(out: &mut String, name: &str, doc: &str, fields: &[ContractField]) -> fmt::Result {
    writeln!(out, "/// {doc}")?;
    out.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    writeln!(out, "pub struct {name} {{")?;
    for field in fields {
        let doc = field.documentation.as_deref().map_or_else(
            || format!("The `{}` parameter.", field.fhir_name),
            crate::render::escape_doc,
        );
        for line in wrap(&doc) {
            writeln!(out, "    /// {line}")?;
        }
        let ty = match cardinality(field.min, field.max) {
            Cardinality::One => field.rust_type.clone(),
            Cardinality::Optional => format!("Option<{}>", field.rust_type),
            Cardinality::Many => format!("Vec<{}>", field.rust_type),
        };
        writeln!(out, "    pub {}: {ty},", field.name)?;
    }
    out.push_str("}\n\n");
    for field in fields {
        if let Some(nested) = &field.part_struct {
            render_struct(
                out,
                nested,
                &format!("The parts of the `{}` parameter.", field.fhir_name),
                &field.parts,
            )?;
        }
    }
    Ok(())
}

fn render_descriptor(out: &mut String, contract: &OperationContract) -> fmt::Result {
    let d = format!("super::super::super::{DESCRIPTOR_MODULE}");
    writeln!(
        out,
        "/// The declared parameter set of `{}/${}`.",
        contract.resource, contract.code
    )?;
    writeln!(
        out,
        "pub const {}: {d}::Operation = {d}::Operation {{",
        contract.descriptor
    )?;
    writeln!(out, "    url: {:?},", contract.url)?;
    writeln!(out, "    resource: {:?},", contract.resource)?;
    writeln!(out, "    code: {:?},", contract.code)?;
    writeln!(out, "    system: {},", contract.system)?;
    writeln!(out, "    type_level: {},", contract.type_level)?;
    writeln!(out, "    instance: {},", contract.instance)?;
    out.push_str("    parameters: &[\n");
    for field in contract.inputs.iter().chain(&contract.outputs) {
        render_parameter(out, field, &d, 2)?;
    }
    out.push_str("    ],\n};\n");
    Ok(())
}

fn render_parameter(out: &mut String, field: &ContractField, d: &str, depth: usize) -> fmt::Result {
    let pad = "    ".repeat(depth);
    writeln!(out, "{pad}{d}::Parameter {{")?;
    writeln!(out, "{pad}    name: {:?},", field.fhir_name)?;
    let usage = match field.usage {
        ParameterUse::In => "In",
        ParameterUse::Out => "Out",
    };
    writeln!(out, "{pad}    usage: {d}::ParameterUse::{usage},")?;
    let max = match field.max {
        Max::Unbounded => String::from("None"),
        Max::Bounded(n) => format!("Some({n})"),
    };
    writeln!(
        out,
        "{pad}    cardinality: {d}::Cardinality {{ min: {}, max: {max} }},",
        field.min
    )?;
    match &field.type_code {
        Some(code) => writeln!(out, "{pad}    type_code: Some({code:?}),")?,
        None => writeln!(out, "{pad}    type_code: None,")?,
    }
    let scope: Vec<String> = field.scope.iter().map(|s| format!("{s:?}")).collect();
    writeln!(out, "{pad}    scope: &[{}],", scope.join(", "))?;
    if field.parts.is_empty() {
        writeln!(out, "{pad}    parts: &[],")?;
    } else {
        writeln!(out, "{pad}    parts: &[")?;
        for part in &field.parts {
            render_parameter(out, part, d, depth + 2)?;
        }
        writeln!(out, "{pad}    ],")?;
    }
    writeln!(out, "{pad}}},")
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn wrap(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > 72 {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::pascal;

    #[test]
    fn codes_become_pascal_case() {
        assert_eq!(pascal("lookup"), "Lookup");
        assert_eq!(pascal("validate-code"), "ValidateCode");
        assert_eq!(pascal("find-matches"), "FindMatches");
    }
}
