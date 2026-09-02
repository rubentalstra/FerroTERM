//! The FHIR conformance resources as the vendored package spells them.
//!
//! Serde projections of `StructureDefinition`, `OperationDefinition`,
//! `ValueSet`, and `CodeSystem`, keeping the fields the generator consumes.
//! Field names follow the FHIR JSON representation
//! (<https://hl7.org/fhir/R4B/json.html>); fields the generator never reads
//! are left out rather than modelled and ignored.

use serde::Deserialize;

/// The `resourceType` discriminator every FHIR JSON resource carries.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceHeader {
    /// The resource type name, for example `StructureDefinition`.
    pub resource_type: String,
}

/// A FHIR `StructureDefinition` (<https://hl7.org/fhir/R4B/structuredefinition.html>).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureDefinition {
    /// The canonical URL identifying this definition.
    pub url: String,
    /// The business version of the definition.
    pub version: Option<String>,
    /// The computer-friendly name (the type name for core definitions).
    pub name: String,
    /// `draft`, `active`, `retired`, or `unknown`.
    pub status: String,
    /// `primitive-type`, `complex-type`, `resource`, or `logical`.
    pub kind: StructureKind,
    /// Whether the type is abstract and never instantiated directly.
    #[serde(rename = "abstract")]
    pub is_abstract: bool,
    /// The type this structure describes or constrains.
    #[serde(rename = "type")]
    pub type_name: String,
    /// The canonical URL of the definition this one derives from.
    pub base_definition: Option<String>,
    /// `specialization` (a new type) or `constraint` (a profile).
    pub derivation: Option<Derivation>,
    /// The fully-resolved element list.
    pub snapshot: Option<ElementList>,
    /// The elements that differ from the base.
    pub differential: Option<ElementList>,
}

/// `StructureDefinition.kind` (<https://hl7.org/fhir/R4B/valueset-structure-definition-kind.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructureKind {
    /// A primitive datatype.
    PrimitiveType,
    /// A complex datatype.
    ComplexType,
    /// A resource.
    Resource,
    /// A logical model.
    Logical,
}

/// `StructureDefinition.derivation` (<https://hl7.org/fhir/R4B/valueset-type-derivation-rule.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Derivation {
    /// The definition introduces a new type.
    Specialization,
    /// The definition constrains an existing type.
    Constraint,
}

/// A `snapshot` or `differential` element list.
#[derive(Debug, Clone, Deserialize)]
pub struct ElementList {
    /// The elements in definition order.
    pub element: Vec<ElementDefinition>,
}

/// A FHIR `ElementDefinition` (<https://hl7.org/fhir/R4B/elementdefinition.html>).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDefinition {
    /// The element id, unique within the structure.
    pub id: Option<String>,
    /// The dotted path from the root type, for example `ValueSet.compose.include`.
    pub path: String,
    /// A short description.
    pub short: Option<String>,
    /// The formal definition.
    pub definition: Option<String>,
    /// The minimum cardinality.
    pub min: Option<u32>,
    /// The maximum cardinality: a number or `*`.
    pub max: Option<String>,
    /// The allowed types; more than one makes the element a choice.
    #[serde(default, rename = "type")]
    pub types: Vec<ElementType>,
    /// A `#path` reference to another element whose children this one shares.
    pub content_reference: Option<String>,
    /// The terminology binding for coded elements.
    pub binding: Option<Binding>,
    /// Whether the element is part of the summary view.
    pub is_summary: Option<bool>,
    /// Whether the element modifies the meaning of its parent.
    pub is_modifier: Option<bool>,
}

/// One entry of `ElementDefinition.type`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementType {
    /// The type code: a FHIR type name or a `FHIRPath` system type URL.
    pub code: String,
    /// Profiles the value must conform to.
    #[serde(default)]
    pub profile: Vec<String>,
    /// Profiles a `Reference` or `canonical` target must conform to.
    #[serde(default)]
    pub target_profile: Vec<String>,
    /// Extensions on the type; primitives carry `structuredefinition-fhir-type` here.
    #[serde(default)]
    pub extension: Vec<Extension>,
}

/// A FHIR extension with a URL value, the only value form the loader reads.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    /// The extension's defining URL.
    pub url: String,
    /// The value when the extension carries a `url` value.
    pub value_url: Option<String>,
    /// The value when the extension carries a `string` value.
    pub value_string: Option<String>,
}

/// `ElementDefinition.binding` or `OperationDefinition.parameter.binding`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    /// `required`, `extensible`, `preferred`, or `example`.
    pub strength: String,
    /// The bound value set, as a canonical URL, possibly with a `|version`.
    pub value_set: Option<String>,
    /// A description of the binding.
    pub description: Option<String>,
}

/// A FHIR `OperationDefinition` (<https://hl7.org/fhir/R4B/operationdefinition.html>).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDefinition {
    /// The canonical URL identifying this operation.
    pub url: String,
    /// The business version.
    pub version: Option<String>,
    /// The computer-friendly name.
    pub name: String,
    /// `draft`, `active`, `retired`, or `unknown`.
    pub status: String,
    /// What the operation does.
    pub description: Option<String>,
    /// `operation` or `query`.
    pub kind: String,
    /// The name used to invoke the operation, without the `$`.
    pub code: String,
    /// The resource types the operation applies to.
    #[serde(default)]
    pub resource: Vec<String>,
    /// Whether the operation is invoked at the system level.
    pub system: bool,
    /// Whether the operation is invoked at the resource type level.
    #[serde(rename = "type")]
    pub type_level: bool,
    /// Whether the operation is invoked on a resource instance.
    pub instance: bool,
    /// The in and out parameters.
    #[serde(default)]
    pub parameter: Vec<OperationParameter>,
}

/// One `OperationDefinition.parameter`, possibly with nested parts.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationParameter {
    /// The parameter name.
    pub name: String,
    /// `in` or `out`.
    #[serde(rename = "use")]
    pub usage: ParameterUse,
    /// The minimum cardinality.
    pub min: u32,
    /// The maximum cardinality: a number or `*`.
    pub max: String,
    /// The parameter's documentation.
    pub documentation: Option<String>,
    /// The parameter's type, absent when `part` carries the structure.
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    /// Profiles a `Reference` or `canonical` target must conform to.
    #[serde(default)]
    pub target_profile: Vec<String>,
    /// The search type when the parameter is a search parameter.
    pub search_type: Option<String>,
    /// The terminology binding for coded parameters.
    pub binding: Option<Binding>,
    /// The invocation levels the parameter applies to (`instance`, `type`, `system`); R5.
    #[serde(default)]
    pub scope: Vec<String>,
    /// Nested parts of a multi-part parameter.
    #[serde(default)]
    pub part: Vec<OperationParameter>,
}

/// `OperationDefinition.parameter.use` (<https://hl7.org/fhir/R4B/valueset-operation-parameter-use.html>).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParameterUse {
    /// An input parameter.
    In,
    /// An output parameter.
    Out,
}

/// A FHIR `ValueSet` (<https://hl7.org/fhir/R4B/valueset.html>).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSet {
    /// The canonical URL identifying this value set.
    pub url: String,
    /// The business version.
    pub version: Option<String>,
    /// The computer-friendly name.
    pub name: Option<String>,
    /// `draft`, `active`, `retired`, or `unknown`.
    // NOTE: ValueSet.status is 1..1 (https://hl7.org/fhir/R4B/valueset.html), yet
    // the published 4.3.0 package omits it on one value set, so the loader reads it
    // as optional and the emitter decides how to surface the gap.
    pub status: Option<String>,
    /// The content logical definition.
    pub compose: Option<ValueSetCompose>,
}

/// `ValueSet.compose`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSetCompose {
    /// The included code systems, concepts, and filters.
    #[serde(default)]
    pub include: Vec<ValueSetInclude>,
    /// The excluded code systems, concepts, and filters.
    #[serde(default)]
    pub exclude: Vec<ValueSetInclude>,
}

/// `ValueSet.compose.include` (and `exclude`, which shares the shape).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSetInclude {
    /// The code system the concepts come from.
    pub system: Option<String>,
    /// The code system version.
    pub version: Option<String>,
    /// Explicitly listed concepts.
    #[serde(default)]
    pub concept: Vec<ValueSetConcept>,
    /// Filters selecting concepts from the system.
    #[serde(default)]
    pub filter: Vec<ValueSetFilter>,
    /// Other value sets whose content is included.
    #[serde(default)]
    pub value_set: Vec<String>,
}

/// `ValueSet.compose.include.concept`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSetConcept {
    /// The code.
    pub code: String,
    /// The display text.
    pub display: Option<String>,
}

/// `ValueSet.compose.include.filter`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueSetFilter {
    /// The property the filter tests.
    pub property: String,
    /// The filter operator, from the `filter-operator` value set.
    pub op: String,
    /// The value the filter compares against.
    pub value: String,
}

/// A FHIR `CodeSystem` (<https://hl7.org/fhir/R4B/codesystem.html>).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSystem {
    /// The canonical URL identifying this code system.
    pub url: String,
    /// The business version.
    pub version: Option<String>,
    /// The computer-friendly name.
    pub name: Option<String>,
    /// `draft`, `active`, `retired`, or `unknown`.
    // NOTE: CodeSystem.status is 1..1 (https://hl7.org/fhir/R4B/codesystem.html), yet
    // the published 4.3.0 package omits it on catalogType, so the loader reads it as
    // optional and the emitter decides how to surface the gap.
    pub status: Option<String>,
    /// `not-present`, `example`, `fragment`, `complete`, or `supplement`.
    pub content: String,
    /// Whether the code system defines a compositional grammar.
    pub compositional: Option<bool>,
    /// Whether the concepts form a case-sensitive code space.
    pub case_sensitive: Option<bool>,
    /// The value set containing every concept of this system.
    pub value_set: Option<String>,
    /// The top-level concepts.
    #[serde(default)]
    pub concept: Vec<CodeSystemConcept>,
}

/// `CodeSystem.concept`, recursive through `concept`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSystemConcept {
    /// The code.
    pub code: String,
    /// The display text.
    pub display: Option<String>,
    /// The formal definition.
    pub definition: Option<String>,
    /// Child concepts.
    #[serde(default)]
    pub concept: Vec<CodeSystemConcept>,
}
