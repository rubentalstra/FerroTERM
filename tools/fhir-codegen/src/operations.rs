//! Operation contracts: the terminology `OperationDefinition`s as typed
//! request and response shapes plus a runtime descriptor.
//!
//! Each terminology operation of a version yields a request struct (its `in`
//! parameters), a response struct (its `out` parameters), one struct per
//! multi-part parameter, and a `const` describing the exact parameter set the
//! version declares (<https://hl7.org/fhir/R4B/operationdefinition.html>), so
//! a server can refuse what the version does not define
//! (`spec-adherence.md`). The terminology ecosystem overlay
//! (`crate::ecosystem`) adds its parameters before lowering, each marked
//! with its source.

use std::fmt::{self, Write};

use crate::ecosystem::{self, ParameterSource};
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
/// How a parameter's value travels in `Parameters.parameter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// A data type: one variant of the open-type enum, named like the type.
    Value(String),
    /// A resource: `parameter.resource`, one variant of the resource enum.
    Resource(String),
    /// `Resource` itself: `parameter.resource`, any variant.
    AnyResource,
    /// `Element`: any value of the open-type enum.
    OpenType,
    /// A multi-part parameter: `parameter.part`.
    Parts,
}

/// One operation parameter as the module renders it: a struct field plus
/// the wire facts the conversions and the descriptor need.
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
    /// How the value travels on the wire.
    pub kind: FieldKind,
    /// Whether the field's type derives `Default` (so a required field of it
    /// does not stop the owning struct from deriving `Default`).
    pub defaultable: bool,
    /// Where the parameter comes from: the version, or the ecosystem overlay.
    pub source: ParameterSource,
    /// The other primitive variants a value of this field is read from: the
    /// primitives that specialize the declared one and share its scalar
    /// (`code` for a `string` parameter, `canonical` for a `uri` one), and the
    /// ones it specializes on the same terms (`uri` for a `canonical`
    /// parameter).
    pub accepts: Vec<String>,
    /// Whether the open-type variant this field travels in holds its value
    /// behind a `Box`, which every complex type does so the enum stays as
    /// narrow as its primitives.
    pub boxed: bool,
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
        let defaultable = crate::render::defaultable(module);
        for parameter in &definition.parameter {
            let owner = match parameter.usage {
                ParameterUse::In => &request,
                ParameterUse::Out => &response,
            };
            let field = lower_field(parameter, owner, &definition.code, module, &defaultable)?;
            match parameter.usage {
                ParameterUse::In => inputs.push(field),
                ParameterUse::Out => outputs.push(field),
            }
        }
        disambiguate(&mut inputs);
        disambiguate(&mut outputs);
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

    /// Lowers `definition` with the terminology ecosystem overlay applied
    /// (`r6` is the R6 definition of the same operation, the source of the
    /// pre-adopted parameters), marking each added field with its source.
    ///
    /// # Errors
    ///
    /// Returns [`OperationError`] as [`Self::lower`] does.
    pub fn lower_overlaid(
        definition: &OperationDefinition,
        resource: &str,
        module: &VersionModule,
        r6: Option<&OperationDefinition>,
    ) -> Result<Self, OperationError> {
        let (overlaid, added) = ecosystem::overlay(definition, r6);
        let mut contract = Self::lower(&overlaid, resource, module)?;
        for field in contract.inputs.iter_mut().chain(&mut contract.outputs) {
            if let Some(entry) = added
                .iter()
                .find(|a| a.usage == field.usage && a.name == field.fhir_name)
            {
                field.source = entry.source;
            }
            for part in &mut field.parts {
                let dotted = format!("{}.{}", field.fhir_name, part.fhir_name);
                if let Some(entry) = added
                    .iter()
                    .find(|a| a.usage == field.usage && a.name == dotted)
                {
                    part.source = entry.source;
                }
            }
        }
        Ok(contract)
    }
}

fn lower_field(
    parameter: &OperationParameter,
    owner: &str,
    operation: &str,
    module: &VersionModule,
    defaultable_types: &std::collections::BTreeSet<String>,
) -> Result<ContractField, OperationError> {
    let max = parse_max(&parameter.max).ok_or_else(|| OperationError::InvalidMax {
        operation: operation.to_owned(),
        name: parameter.name.clone(),
        max: parameter.max.clone(),
    })?;
    let part_struct =
        (!parameter.part.is_empty()).then(|| format!("{owner}{}", pascal(&parameter.name)));
    let mut kind = FieldKind::Parts;
    let rust_type = match (&parameter.type_name, &part_struct) {
        (_, Some(nested)) => nested.clone(),
        (Some(code), None) if code == "Element" => {
            kind = FieldKind::OpenType;
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
            kind = if ty.is_resource {
                FieldKind::Resource(name.clone())
            } else if code == "Resource" {
                FieldKind::AnyResource
            } else {
                FieldKind::Value(name.clone())
            };
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
            parts.push(lower_field(
                part,
                nested,
                operation,
                module,
                defaultable_types,
            )?);
        }
    }
    let defaultable = match &kind {
        FieldKind::Value(name) | FieldKind::Resource(name) => defaultable_types.contains(name),
        FieldKind::OpenType | FieldKind::AnyResource => false,
        FieldKind::Parts => derives_default(&parts),
    };
    let accepts = match &kind {
        FieldKind::Value(name) => {
            let mut both = specializations(module, name);
            for wider in generalizations(module, name) {
                if !both.contains(&wider) {
                    both.push(wider);
                }
            }
            both.sort();
            both
        }
        _ => Vec::new(),
    };
    let boxed = match &kind {
        FieldKind::Value(name) => !module.types.get(name).is_some_and(|ty| ty.is_primitive),
        _ => false,
    };
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
        kind,
        defaultable,
        source: ParameterSource::Version,
        accepts,
        boxed,
    })
}

/// The primitives of `module` that specialize `name` (through their
/// `baseDefinition` chain) and carry the same scalar, so a value sent as one
/// of them reads as `name`: a `code` is a `string`, a `canonical` is a `uri`
/// (<https://hl7.org/fhir/R5/datatypes.html#primitive>).
fn specializations(module: &VersionModule, name: &str) -> Vec<String> {
    let Some(base) = module.types.get(name).filter(|t| t.is_primitive) else {
        return Vec::new();
    };
    let code = |rust: &str| {
        let mut chars = rust.chars();
        chars
            .next()
            .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
            .unwrap_or_default()
    };
    let scalar = crate::lower::scalar_for(&code(&base.name));
    module
        .types
        .values()
        .filter(|t| t.is_primitive && t.name != name)
        .filter(|t| {
            let mut current = t.base.as_deref();
            let mut hops = 0;
            while let Some(step) = current {
                if step == name {
                    return true;
                }
                hops += 1;
                if hops > 8 {
                    break;
                }
                current = module.types.get(step).and_then(|b| b.base.as_deref());
            }
            false
        })
        .filter(|t| crate::lower::scalar_for(&code(&t.name)) == scalar)
        .map(|t| t.name.clone())
        .collect()
}

/// The primitives `name` specializes (through its own `baseDefinition` chain)
/// that carry the same scalar, so a value sent as one of them reads as `name`:
/// a `uri` reads as a `canonical`
/// (<https://hl7.org/fhir/R5/datatypes.html#primitive>).
///
/// `canonical` and `uri` are distinct types that "are never substituted for
/// each other" (<https://hl7.org/fhir/R4B/datatypes.html#uri>), so this is a
/// decision rather than a reading of the type system: their value spaces are
/// identical, no clause requires a server to refuse the wider spelling, and
/// the GET form of an operation delivers the value with no type marker at all,
/// so the server already applies the declared type by parsing
/// (<https://hl7.org/fhir/R4B/operations.html>). Recorded on #352.
fn generalizations(module: &VersionModule, name: &str) -> Vec<String> {
    let Some(declared) = module.types.get(name).filter(|t| t.is_primitive) else {
        return Vec::new();
    };
    let code = |rust: &str| {
        let mut chars = rust.chars();
        chars
            .next()
            .map(|c| c.to_ascii_lowercase().to_string() + chars.as_str())
            .unwrap_or_default()
    };
    let scalar = crate::lower::scalar_for(&code(&declared.name));
    let mut out = Vec::new();
    let mut current = declared.base.as_deref();
    let mut hops = 0;
    while let Some(step) = current {
        let Some(base) = module.types.get(step) else {
            break;
        };
        if base.is_primitive && crate::lower::scalar_for(&code(&base.name)) == scalar {
            out.push(base.name.clone());
        }
        hops += 1;
        if hops > 8 {
            break;
        }
        current = base.base.as_deref();
    }
    out
}

/// Whether a struct of `fields` derives `Default`: every required field's
/// type does (the rule `render::defaultable` applies to the model types).
fn derives_default(fields: &[ContractField]) -> bool {
    fields
        .iter()
        .all(|field| cardinality(field.min, field.max) != Cardinality::One || field.defaultable)
}

/// Keeps field names unique within one struct: when two parameters reduce to
/// the same Rust name (R6's `ValueSet/$validate-code` declares both
/// `systemVersion` and `system-version`), the hyphenated one gets its type
/// code as a suffix (`system_version_canonical`). No spec governs the Rust
/// names: our own rule.
fn disambiguate(fields: &mut [ContractField]) {
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for field in fields.iter() {
        *seen.entry(field.name.clone()).or_insert(0) += 1;
    }
    for field in fields.iter_mut() {
        if seen.get(&field.name).copied().unwrap_or(0) > 1
            && field.fhir_name.contains('-')
            && let Some(code) = &field.type_code
        {
            field.name = format!("{}_{}", field.name, field_name(code));
        }
        disambiguate(&mut field.parts);
    }
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
//! The exact parameter set each FHIR version declares
//! (<https://hl7.org/fhir/R4B/operationdefinition.html>) plus the terminology
//! ecosystem overlay (<https://hl7.org/fhir/uv/tx-ecosystem/requirements.html>),
//! each parameter marked with its source, so a server accepts nothing more
//! and nothing less.
//!
//! NOTE: a parameter is read from the primitive its definition declares and
//! from the primitives that share that one's scalar in either direction, so a
//! `canonical` parameter reads a `valueUri` too. No clause requires refusing
//! the wider spelling, and the GET form carries no type marker at all
//! (<https://hl7.org/fhir/R4B/operations.html>); the decision is recorded on
//! #352.

/// The direction of an operation parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterUse {
    /// An input parameter.
    In,
    /// An output parameter.
    Out,
}

/// Where a declared parameter comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSource {
    /// The version's own `OperationDefinition`.
    Version,
    /// Pre-adopted from the FHIR R6 ballot for the terminology ecosystem.
    PreAdopted,
    /// Defined by the terminology ecosystem alone.
    Ecosystem,
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
    /// Where the parameter comes from.
    pub source: ParameterSource,
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
    out.push_str(PARAMETERS_ERROR);
    out
}

/// The error the generated `from_parameters` conversions return.
const PARAMETERS_ERROR: &str = r#"
/// Why a `Parameters` resource does not fit an operation's declared
/// parameter set. Every variant names the operation and the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParametersError {
    /// A parameter has no name.
    Unnamed {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
    },
    /// A parameter the operation does not declare.
    Undeclared {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
        /// The parameter name (dotted for a part).
        name: std::string::String,
    },
    /// A parameter with a maximum of one given more than once.
    Repeated {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
        /// The parameter name (dotted for a part).
        name: &'static str,
    },
    /// A required parameter is absent.
    Missing {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
        /// The parameter name (dotted for a part).
        name: &'static str,
    },
    /// A parameter carries neither a value nor a resource.
    MissingValue {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
        /// The parameter name (dotted for a part).
        name: &'static str,
    },
    /// A parameter's value is not of the declared type.
    WrongType {
        /// The operation, as `Resource/$code`.
        operation: &'static str,
        /// The parameter name (dotted for a part).
        name: &'static str,
        /// The declared type.
        expected: &'static str,
    },
}

impl std::fmt::Display for ParametersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unnamed { operation } => {
                write!(f, "{operation}: a parameter has no name")
            }
            Self::Undeclared { operation, name } => {
                write!(f, "{operation}: parameter `{name}` is not declared")
            }
            Self::Repeated { operation, name } => {
                write!(f, "{operation}: parameter `{name}` is given more than once")
            }
            Self::Missing { operation, name } => {
                write!(f, "{operation}: parameter `{name}` is required")
            }
            Self::MissingValue { operation, name } => {
                write!(f, "{operation}: parameter `{name}` has no value")
            }
            Self::WrongType {
                operation,
                name,
                expected,
            } => write!(f, "{operation}: parameter `{name}` is not a {expected}"),
        }
    }
}

impl std::error::Error for ParametersError {}
"#;

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
    let operation = format!("{}/${}", contract.resource, contract.code);
    render_conversions(
        &mut out,
        &contract.request,
        &operation,
        &contract.inputs,
        "",
    )?;
    render_conversions(
        &mut out,
        &contract.response,
        &operation,
        &contract.outputs,
        "",
    )?;
    render_descriptor(&mut out, contract)?;
    Ok(out)
}

/// `from_parameters`/`into_parameters`/`to_parameters` on a request or response
/// struct, and the parameter-list pair on it and every part struct.
///
/// The moving form is the answer path; the borrowing one clones into it, so the
/// values are copied only where a caller genuinely keeps its own.
fn render_conversions(
    out: &mut String,
    name: &str,
    operation: &str,
    fields: &[ContractField],
    prefix: &str,
) -> fmt::Result {
    writeln!(out, "impl {name} {{")?;
    if prefix.is_empty() {
        writeln!(
            out,
            "    /// Reads the parameters from a `Parameters` resource."
        )?;
        writeln!(out, "    ///")?;
        writeln!(out, "    /// # Errors")?;
        writeln!(out, "    ///")?;
        writeln!(
            out,
            "    /// Returns the error for an undeclared, repeated, missing, or wrongly typed parameter."
        )?;
        writeln!(
            out,
            "    pub fn from_parameters(parameters: &{P}::Parameters) -> Result<Self, {E}> {{"
        )?;
        writeln!(
            out,
            "        Self::from_parameter_list(&parameters.parameter)"
        )?;
        writeln!(out, "    }}")?;
        writeln!(
            out,
            "    /// Writes the parameters as a `Parameters` resource, by move."
        )?;
        writeln!(out, "    #[must_use]")?;
        writeln!(
            out,
            "    pub fn into_parameters(self) -> {P}::Parameters {{"
        )?;
        writeln!(out, "        {P}::Parameters {{")?;
        writeln!(out, "            parameter: self.into_parameter_list(),")?;
        writeln!(out, "            ..Default::default()")?;
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(
            out,
            "    /// Writes the parameters as a `Parameters` resource, from a reference."
        )?;
        writeln!(out, "    #[must_use]")?;
        writeln!(out, "    pub fn to_parameters(&self) -> {P}::Parameters {{")?;
        writeln!(out, "        self.clone().into_parameters()")?;
        writeln!(out, "    }}")?;
    }
    render_from_list(out, operation, fields, prefix)?;
    render_to_list(out, fields)?;
    writeln!(out, "}}\n")?;
    for field in fields {
        if let Some(nested) = &field.part_struct {
            render_conversions(
                out,
                nested,
                operation,
                &field.parts,
                &format!("{prefix}{}.", field.fhir_name),
            )?;
        }
    }
    Ok(())
}

const P: &str = "super::super::parameters";
const E: &str = "super::super::super::operation::ParametersError";

/// `from_parameter_list`: every field collected by name, cardinality enforced.
///
/// Locals carry a `field_` prefix so a parameter named `name` or `value`
/// cannot shadow the loop's bindings.
fn render_from_list(
    out: &mut String,
    operation: &str,
    fields: &[ContractField],
    prefix: &str,
) -> fmt::Result {
    writeln!(out, "    /// Reads the fields from a parameter list.")?;
    writeln!(out, "    ///")?;
    writeln!(out, "    /// # Errors")?;
    writeln!(out, "    ///")?;
    writeln!(
        out,
        "    /// Returns the error for an undeclared, repeated, missing, or wrongly typed parameter."
    )?;
    writeln!(
        out,
        "    pub fn from_parameter_list(list: &[{P}::ParametersParameter]) -> Result<Self, {E}> {{"
    )?;
    writeln!(out, "        const OPERATION: &str = {operation:?};")?;
    writeln!(out, "        const PREFIX: &str = {prefix:?};")?;
    for field in fields {
        if cardinality(field.min, field.max) == Cardinality::Many {
            writeln!(
                out,
                "        let mut {}: Vec<{}> = Vec::new();",
                local(field),
                field.rust_type
            )?;
        } else {
            writeln!(
                out,
                "        let mut {}: Option<{}> = None;",
                local(field),
                field.rust_type
            )?;
        }
    }
    writeln!(out, "        for parameter in list {{")?;
    writeln!(
        out,
        "            let parameter_name = parameter.name.value.as_deref().ok_or({E}::Unnamed {{ operation: OPERATION }})?;"
    )?;
    writeln!(out, "            match parameter_name {{")?;
    for field in fields {
        let path = format!("{prefix}{}", field.fhir_name);
        let extract = render_extract(field, &path);
        writeln!(out, "                {:?} => {{", field.fhir_name)?;
        if cardinality(field.min, field.max) == Cardinality::Many {
            writeln!(out, "                    {}.push({extract});", local(field))?;
        } else {
            writeln!(out, "                    if {}.is_some() {{", local(field))?;
            writeln!(
                out,
                "                        return Err({E}::Repeated {{ operation: OPERATION, name: {path:?} }});"
            )?;
            writeln!(out, "                    }}")?;
            writeln!(
                out,
                "                    {} = Some({extract});",
                local(field)
            )?;
        }
        writeln!(out, "                }}")?;
    }
    writeln!(out, "                other => {{")?;
    writeln!(
        out,
        "                    return Err({E}::Undeclared {{ operation: OPERATION, name: [PREFIX, other].concat() }});"
    )?;
    writeln!(out, "                }}")?;
    writeln!(out, "            }}")?;
    writeln!(out, "        }}")?;
    writeln!(out, "        Ok(Self {{")?;
    for field in fields {
        let path = format!("{prefix}{}", field.fhir_name);
        if cardinality(field.min, field.max) == Cardinality::One {
            writeln!(
                out,
                "            {}: {}.ok_or({E}::Missing {{ operation: OPERATION, name: {path:?} }})?,",
                field.name,
                local(field)
            )?;
        } else {
            writeln!(out, "            {}: {},", field.name, local(field))?;
        }
    }
    writeln!(out, "        }})")?;
    writeln!(out, "    }}")
}

/// The local that accumulates a field inside `from_parameter_list`: the
/// field name with a `field_` prefix and without a raw-identifier marker.
fn local(field: &ContractField) -> String {
    format!("field_{}", field.name.trim_start_matches("r#"))
}

/// `to_parameter_list`: one `ParametersParameter` per present value.
fn render_to_list(out: &mut String, fields: &[ContractField]) -> fmt::Result {
    writeln!(
        out,
        "    /// Writes the fields as a parameter list, by move."
    )?;
    writeln!(out, "    #[must_use]")?;
    writeln!(
        out,
        "    pub fn into_parameter_list(self) -> Vec<{P}::ParametersParameter> {{"
    )?;
    writeln!(
        out,
        "        let mut out = Vec::with_capacity({});",
        fields.len()
    )?;
    for field in fields {
        let build = render_build(field, "value");
        match cardinality(field.min, field.max) {
            Cardinality::One => {
                writeln!(out, "        {{")?;
                writeln!(out, "            let value = self.{};", field.name)?;
                writeln!(out, "            out.push({build});")?;
                writeln!(out, "        }}")?;
            }
            Cardinality::Optional => {
                writeln!(out, "        if let Some(value) = self.{} {{", field.name)?;
                writeln!(out, "            out.push({build});")?;
                writeln!(out, "        }}")?;
            }
            Cardinality::Many => {
                writeln!(out, "        for value in self.{} {{", field.name)?;
                writeln!(out, "            out.push({build});")?;
                writeln!(out, "        }}")?;
            }
        }
    }
    writeln!(out, "        out")?;
    writeln!(out, "    }}")?;
    writeln!(
        out,
        "    /// Writes the fields as a parameter list, from a reference."
    )?;
    writeln!(out, "    #[must_use]")?;
    writeln!(
        out,
        "    pub fn to_parameter_list(&self) -> Vec<{P}::ParametersParameter> {{"
    )?;
    writeln!(out, "        self.clone().into_parameter_list()")?;
    writeln!(out, "    }}")
}

/// The expression reading one field's value out of `parameter`.
fn render_extract(field: &ContractField, path: &str) -> String {
    let (p, e) = (P, E);
    match &field.kind {
        FieldKind::Value(variant) => {
            // NOTE: a value sent as a specialization of the declared primitive (a
            // `code` for a `string`, a `canonical` for a `uri`) reads as the
            // declared one (<https://hl7.org/fhir/R5/datatypes.html#primitive>).
            let accepted: Vec<String> = field
                .accepts
                .iter()
                .map(|accepted| {
                    format!(
                        "Some({p}::{OPEN_TYPE_ENUM}::{accepted}(value)) => {} {{ id: value.id.clone(), extension: value.extension.clone(), value: value.value.clone() }}, ",
                        field.rust_type
                    )
                })
                .collect();
            let own = if field.boxed {
                "(**value).clone()"
            } else {
                "value.clone()"
            };
            let arms = format!(
                "Some({p}::{OPEN_TYPE_ENUM}::{variant}(value)) => {own}, {}",
                accepted.join("")
            );
            format!(
                "match &parameter.value {{ {arms}Some(_) => return Err({e}::WrongType {{ operation: OPERATION, name: {path:?}, expected: {variant:?} }}), None => return Err({e}::MissingValue {{ operation: OPERATION, name: {path:?} }}) }}"
            )
        }
        FieldKind::Resource(resource) => format!(
            "match &parameter.resource {{ Some(super::super::resource::Resource::{resource}(value)) => (**value).clone(), Some(_) => return Err({e}::WrongType {{ operation: OPERATION, name: {path:?}, expected: {resource:?} }}), None => return Err({e}::MissingValue {{ operation: OPERATION, name: {path:?} }}) }}"
        ),
        FieldKind::OpenType => format!(
            "parameter.value.clone().ok_or({e}::MissingValue {{ operation: OPERATION, name: {path:?} }})?"
        ),
        FieldKind::AnyResource => format!(
            "parameter.resource.clone().ok_or({e}::MissingValue {{ operation: OPERATION, name: {path:?} }})?"
        ),
        FieldKind::Parts => format!("{}::from_parameter_list(&parameter.part)?", field.rust_type),
    }
}

/// The expression building one `ParametersParameter` out of `value`, which it
/// takes by move: the answer is built once and handed on, never copied.
fn render_build(field: &ContractField, value: &str) -> String {
    let p = P;
    let name = format!("{:?}.into()", field.fhir_name);
    match &field.kind {
        FieldKind::Value(variant) => {
            let held = if field.boxed {
                format!("Box::new({value})")
            } else {
                value.to_owned()
            };
            format!(
                "{p}::ParametersParameter {{ name: {name}, value: Some({p}::{OPEN_TYPE_ENUM}::{variant}({held})), ..Default::default() }}"
            )
        }
        FieldKind::Resource(resource) => format!(
            "{p}::ParametersParameter {{ name: {name}, resource: Some(super::super::resource::Resource::{resource}(Box::new({value}))), ..Default::default() }}"
        ),
        FieldKind::OpenType => format!(
            "{p}::ParametersParameter {{ name: {name}, value: Some({value}), ..Default::default() }}"
        ),
        FieldKind::AnyResource => format!(
            "{p}::ParametersParameter {{ name: {name}, resource: Some({value}), ..Default::default() }}"
        ),
        FieldKind::Parts => format!(
            "{p}::ParametersParameter {{ name: {name}, part: {value}.into_parameter_list(), ..Default::default() }}"
        ),
    }
}

fn render_struct(out: &mut String, name: &str, doc: &str, fields: &[ContractField]) -> fmt::Result {
    writeln!(out, "/// {doc}")?;
    if derives_default(fields) {
        out.push_str("#[derive(Debug, Clone, Default, PartialEq, Eq)]\n");
    } else {
        out.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    }
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
    writeln!(
        out,
        "{pad}    source: {d}::ParameterSource::{},",
        field.source.variant()
    )?;
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
