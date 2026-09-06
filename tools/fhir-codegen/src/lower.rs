//! Lowering resolved structures to the type definitions the renderer emits.
//!
//! Every structure in the closure becomes a struct; a backbone element
//! becomes a struct named by its path; a choice element (`value[x]`,
//! <https://hl7.org/fhir/R4B/formats.html#choice>) becomes an enum with one
//! variant per allowed type; a content reference points at the struct of the
//! element it names. Cardinality maps to `T`, `Option<T>`, and `Vec<T>`
//! (<https://hl7.org/fhir/R4B/conformance-rules.html#cardinality>), and an
//! `Option` or direct field that closes a type cycle is boxed.

use std::collections::{BTreeMap, BTreeSet};

use crate::closure::{STRUCTURAL_TYPES, TypeClosure};
use crate::fhir::StructureKind;
use crate::naming::{backbone_name, field_name, module_name, type_name};
use crate::snapshot::{ElementShape, Max, ResolvedElement, ResolvedStructure, TypeRef};

/// The module holding every primitive type.
pub const PRIMITIVES_MODULE: &str = "primitives";
/// The module holding the `Resource` enum.
pub const RESOURCE_MODULE: &str = "resource";
/// The name of the enum over the root-set resources.
pub const RESOURCE_ENUM: &str = "Resource";
/// The name of the struct carrying a resource outside the root set.
pub const UNKNOWN_RESOURCE: &str = "UnknownResource";

/// A failure while lowering.
#[derive(Debug, thiserror::Error)]
pub enum LowerError {
    /// A non-choice element lists more than one type code.
    #[error("{path} lists {count} types but is not a choice element")]
    MultiTyped {
        /// The element path.
        path: String,
        /// The number of type codes.
        count: usize,
    },
    /// A choice element lists no types.
    #[error("{path} is a choice element with no types")]
    EmptyChoice {
        /// The element path.
        path: String,
    },
    /// Two structures lower to the same Rust type name.
    #[error("two types lower to the Rust name {name}: {first} and {second}")]
    NameCollision {
        /// The colliding name.
        name: String,
        /// The first origin.
        first: String,
        /// The second origin.
        second: String,
    },
}

/// How many values a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Exactly one (`T`).
    One,
    /// Zero or one (`Option<T>`).
    Optional,
    /// Any number (`Vec<T>`).
    Many,
}

/// A value that is not a FHIR type: the `FHIRPath` system types primitives
/// carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    /// `bool`.
    Bool,
    /// `i32`.
    I32,
    /// `u32`.
    U32,
    /// `i64`.
    I64,
    /// `String`; decimals and dates keep their lexical form.
    Str,
}

/// What a field or variant points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A type in this model, by Rust name.
    Named(String),
    /// A scalar value.
    Inline(Scalar),
}

/// A field's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    /// The cardinality.
    pub card: Cardinality,
    /// The target.
    pub target: Target,
    /// Whether the value is boxed to break a type cycle.
    pub boxed: bool,
}

/// Documentation carried from the definition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Docs {
    /// The one-line summary.
    pub short: Option<String>,
    /// The formal definition.
    pub definition: Option<String>,
}

/// A struct field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The Rust field name.
    pub name: String,
    /// The FHIR element name (the last path segment, with `[x]` kept).
    pub fhir_name: String,
    /// The element path.
    pub path: String,
    /// Documentation.
    pub docs: Docs,
    /// The type.
    pub ty: FieldType,
}

/// A choice enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The Rust variant name.
    pub name: String,
    /// The FHIR type code.
    pub code: String,
    /// The type held.
    pub target: Target,
    /// Whether the value is boxed to break a type cycle.
    pub boxed: bool,
}

/// The shape of a Rust type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// A struct.
    Struct {
        /// The fields, in snapshot order.
        fields: Vec<Field>,
    },
    /// A choice enum.
    Choice {
        /// The FHIR element the choice belongs to.
        element_path: String,
        /// The variants, in the order the definition lists the types.
        variants: Vec<Variant>,
    },
    /// The enum over the root-set resources.
    ResourceEnum {
        /// The resource type names, in name order.
        resources: Vec<String>,
    },
    /// The struct carrying a resource outside the root set.
    UnknownResource,
}

/// One type definition to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    /// The Rust name.
    pub name: String,
    /// The module (file) the type lives in.
    pub module: String,
    /// Documentation.
    pub docs: Docs,
    /// The shape.
    pub kind: TypeKind,
    /// Whether the type is a FHIR primitive.
    pub is_primitive: bool,
    /// Whether the type is a root-set resource.
    pub is_resource: bool,
    /// The Rust name of the type this one specializes (`baseDefinition`), for
    /// a primitive: `Code` and `Id` specialize `String`, `Canonical` and `Url`
    /// specialize `Uri` (<https://hl7.org/fhir/R5/datatypes.html#primitive>).
    pub base: Option<String>,
}

/// The generated module for one FHIR version: every type it emits.
#[derive(Debug)]
pub struct VersionModule {
    /// The module name, for example `r4b`.
    pub name: String,
    /// The package the model was lowered from.
    pub package_name: String,
    /// The terminology operations the version declares, in module order.
    pub operations: Vec<crate::operations::OperationContract>,
    /// The package version.
    pub package_version: String,
    /// Every type, keyed by Rust name.
    pub types: BTreeMap<String, TypeDef>,
}

impl VersionModule {
    /// Lowers `closure` into a model.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] for a non-choice element with several types, a
    /// choice with none, or two structures lowering to one Rust name.
    pub fn lower(
        closure: &TypeClosure,
        version_module: &str,
        package_name: &str,
        package_version: &str,
    ) -> Result<Self, LowerError> {
        let mut model = Self {
            name: version_module.to_owned(),
            package_name: package_name.to_owned(),
            package_version: package_version.to_owned(),
            operations: Vec::new(),
            types: BTreeMap::new(),
        };
        for structure in closure.structures().values() {
            let is_primitive = structure.kind == StructureKind::PrimitiveType;
            let name = type_name(&structure.name);
            let module = if is_primitive {
                PRIMITIVES_MODULE.to_owned()
            } else {
                module_name(&name)
            };
            let is_resource = closure.roots().contains(&structure.name);
            let root_path = structure
                .elements
                .first()
                .map_or_else(|| structure.name.clone(), |e| e.path.clone());
            lower_struct(
                &mut model,
                structure,
                &root_path,
                &name,
                &module,
                is_primitive,
                is_resource,
            )?;
        }
        let resources: Vec<String> = closure.roots().iter().map(|name| type_name(name)).collect();
        model.insert(TypeDef {
            name: RESOURCE_ENUM.to_owned(),
            module: RESOURCE_MODULE.to_owned(),
            docs: Docs {
                short: Some(String::from("A resource of the terminology root set, or an unknown resource carried as JSON.")),
                definition: Some(
                    format!("The abstract Resource type (https://hl7.org/fhir/{}/resource.html) as the root set closes over it: one variant per root-set resource, and UnknownResource for any other resource type met inside a Bundle entry or a contained list.", version_module.to_uppercase()),
                ),
            },
            kind: TypeKind::ResourceEnum { resources },
            is_primitive: false,
            is_resource: false,
            base: None,
        }, "the Resource enum")?;
        model.insert(TypeDef {
            name: UNKNOWN_RESOURCE.to_owned(),
            module: RESOURCE_MODULE.to_owned(),
            docs: Docs {
                short: Some(String::from("A resource outside the root set, kept as its JSON body.")),
                definition: Some(String::from(
                    "Carries the resourceType and the complete JSON object so a Bundle or a contained resource of a type the terminology surface does not model round-trips unchanged.",
                )),
            },
            kind: TypeKind::UnknownResource,
            is_primitive: false,
            is_resource: false,
            base: None,
        }, "the UnknownResource struct")?;
        model.box_cycles();
        model.box_wide_variants();
        Ok(model)
    }

    fn insert(&mut self, ty: TypeDef, origin: &str) -> Result<(), LowerError> {
        if let Some(existing) = self.types.get(&ty.name) {
            return Err(LowerError::NameCollision {
                name: ty.name.clone(),
                first: existing.module.clone(),
                second: origin.to_owned(),
            });
        }
        self.types.insert(ty.name.clone(), ty);
        Ok(())
    }

    /// The modules of the model, in name order, each with its types in name order.
    #[must_use]
    pub fn modules(&self) -> BTreeMap<&str, Vec<&TypeDef>> {
        let mut modules: BTreeMap<&str, Vec<&TypeDef>> = BTreeMap::new();
        for ty in self.types.values() {
            modules.entry(ty.module.as_str()).or_default().push(ty);
        }
        modules
    }

    /// Boxes every direct or optional edge that lies inside a type cycle.
    fn box_cycles(&mut self) {
        let sccs = strongly_connected_components(&self.edges());
        let component_of: BTreeMap<String, usize> = sccs
            .iter()
            .enumerate()
            .flat_map(|(index, component)| component.iter().map(move |name| (name.clone(), index)))
            .collect();
        for ty in self.types.values_mut() {
            let own = component_of.get(&ty.name).copied();
            match &mut ty.kind {
                TypeKind::Struct { fields } => box_cyclic_fields(fields, own, &component_of),
                TypeKind::Choice { variants, .. } => {
                    box_cyclic_variants(variants, own, &component_of);
                }
                TypeKind::ResourceEnum { .. } | TypeKind::UnknownResource => {}
            }
        }
    }

    /// Boxes every choice variant holding a complex type, so a choice enum is
    /// only as wide as the widest primitive it admits.
    ///
    /// A Rust enum is as large as its largest variant, so one rare wide type
    /// sets the size every value of the enum pays. `Parameters.value[x]` admits
    /// `Dosage`, which carries a whole `Timing`, and that made every
    /// `valueString` of an answer cost the same as a dosage schedule. Primitives
    /// stay inline: their width is bounded by the base element and they are what
    /// a terminology answer is nearly all of.
    fn box_wide_variants(&mut self) {
        let primitives: BTreeSet<String> = self
            .types
            .values()
            .filter(|ty| ty.is_primitive)
            .map(|ty| ty.name.clone())
            .collect();
        for ty in self.types.values_mut() {
            if let TypeKind::Choice { variants, .. } = &mut ty.kind {
                for variant in variants.iter_mut() {
                    if let Target::Named(target) = &variant.target
                        && !primitives.contains(target)
                    {
                        variant.boxed = true;
                    }
                }
            }
        }
    }

    /// The direct-containment graph: edges for `T` and `Option<T>` fields and
    /// for enum variants; `Vec<T>` already breaks recursion.
    fn edges(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for ty in self.types.values() {
            let out = edges.entry(ty.name.clone()).or_default();
            contained_types(&ty.kind, out);
        }
        edges
    }
}

/// Whether `target` names a type in the component `own`, the caller's cycle.
fn in_component(
    component_of: &BTreeMap<String, usize>,
    own: Option<usize>,
    target: &Target,
) -> bool {
    match target {
        Target::Named(name) => own.is_some() && component_of.get(name).copied() == own,
        Target::Inline(_) => false,
    }
}

/// Boxes every `T` or `Option<T>` field whose target shares the type's cycle.
fn box_cyclic_fields(
    fields: &mut [Field],
    own: Option<usize>,
    component_of: &BTreeMap<String, usize>,
) {
    for field in fields {
        if field.ty.card != Cardinality::Many && in_component(component_of, own, &field.ty.target) {
            field.ty.boxed = true;
        }
    }
}

/// Boxes every choice variant whose target shares the enum's cycle.
fn box_cyclic_variants(
    variants: &mut [Variant],
    own: Option<usize>,
    component_of: &BTreeMap<String, usize>,
) {
    for variant in variants {
        if in_component(component_of, own, &variant.target) {
            variant.boxed = true;
        }
    }
}

/// Collects the types `kind` contains directly into `out`.
fn contained_types(kind: &TypeKind, out: &mut BTreeSet<String>) {
    match kind {
        TypeKind::Struct { fields } => {
            for field in fields {
                if field.ty.card != Cardinality::Many
                    && let Target::Named(target) = &field.ty.target
                {
                    out.insert(target.clone());
                }
            }
        }
        TypeKind::Choice { variants, .. } => {
            for variant in variants {
                if let Target::Named(target) = &variant.target {
                    out.insert(target.clone());
                }
            }
        }
        TypeKind::ResourceEnum { resources } => {
            out.extend(resources.iter().cloned());
        }
        TypeKind::UnknownResource => {}
    }
}

fn lower_struct(
    model: &mut VersionModule,
    structure: &ResolvedStructure,
    path: &str,
    name: &str,
    module: &str,
    is_primitive: bool,
    is_resource: bool,
) -> Result<(), LowerError> {
    let root_docs = structure.element(path).map(docs_of).unwrap_or_default();
    let mut fields = Vec::new();
    for element in structure.children_of(path).cloned().collect::<Vec<_>>() {
        // NOTE: a max of 0 prohibits the element (https://hl7.org/fhir/R4B/conformance-rules.html#cardinality),
        // so it has no field; xhtml prohibits extension this way.
        if element.max == Max::Bounded(0) {
            continue;
        }
        let card = card_of(&element);
        let target = match &element.shape {
            ElementShape::Root => continue,
            ElementShape::ContentReference {
                structure: _,
                path: target_path,
            } => Target::Named(backbone_name(target_path)),
            ElementShape::Choice(types) => {
                let enum_name = format!(
                    "{name}{}",
                    type_name(element.choice_stem().unwrap_or(element.name()))
                );
                let variants = choice_variants(types, &element.path)?;
                model.insert(
                    TypeDef {
                        name: enum_name.clone(),
                        module: module.to_owned(),
                        docs: Docs {
                            short: Some(format!("The `{}` choice of `{name}`.", element.name())),
                            definition: element.definition.clone(),
                        },
                        kind: TypeKind::Choice {
                            element_path: element.path.clone(),
                            variants,
                        },
                        is_primitive: false,
                        base: None,
                        is_resource: false,
                    },
                    &element.path,
                )?;
                Target::Named(enum_name)
            }
            ElementShape::Typed(types) => {
                lower_typed(model, structure, &element, types, module, is_primitive)?
            }
        };
        fields.push(Field {
            name: field_name(element.name()),
            fhir_name: element.name().to_owned(),
            path: element.path.clone(),
            docs: docs_of(&element),
            ty: FieldType {
                card,
                target,
                boxed: false,
            },
        });
    }
    model.insert(
        TypeDef {
            name: name.to_owned(),
            module: module.to_owned(),
            docs: root_docs,
            kind: TypeKind::Struct { fields },
            is_primitive,
            is_resource,
            base: is_primitive
                .then_some(structure.base_definition.as_deref())
                .flatten()
                .and_then(|url| url.rsplit('/').next())
                .map(type_name),
        },
        path,
    )
}

fn choice_variants(types: &[TypeRef], path: &str) -> Result<Vec<Variant>, LowerError> {
    if types.is_empty() {
        return Err(LowerError::EmptyChoice {
            path: path.to_owned(),
        });
    }
    Ok(types
        .iter()
        .map(|type_ref| {
            let target = if STRUCTURAL_TYPES.contains(&type_ref.code.as_str()) {
                Target::Named(RESOURCE_ENUM.to_owned())
            } else {
                Target::Named(type_name(&type_ref.code))
            };
            Variant {
                name: type_name(&type_ref.code),
                code: type_ref.code.clone(),
                target,
                boxed: false,
            }
        })
        .collect())
}

fn card_of(element: &ResolvedElement) -> Cardinality {
    match (element.min, element.max) {
        (_, Max::Unbounded) => Cardinality::Many,
        (_, Max::Bounded(n)) if n > 1 => Cardinality::Many,
        (0, _) => Cardinality::Optional,
        (_, _) => Cardinality::One,
    }
}

fn docs_of(element: &ResolvedElement) -> Docs {
    Docs {
        short: element.short.clone(),
        definition: element.definition.clone(),
    }
}

/// The target of a single-typed element: an inline scalar for a primitive's
/// value, a nested struct for a backbone, the resource enum, or a named type.
fn lower_typed(
    model: &mut VersionModule,
    structure: &ResolvedStructure,
    element: &ResolvedElement,
    types: &[TypeRef],
    module: &str,
    is_primitive: bool,
) -> Result<Target, LowerError> {
    let Some(only) = types.first() else {
        return Err(LowerError::EmptyChoice {
            path: element.path.clone(),
        });
    };
    if types.len() > 1 && types.iter().any(|t| t.code != only.code) {
        return Err(LowerError::MultiTyped {
            path: element.path.clone(),
            count: types.len(),
        });
    }
    if only.fhirpath_type.is_some() {
        // NOTE: a primitive's JSON scalar follows the primitive's own name
        // (<https://hl7.org/fhir/R4/json.html#primitive>); the 4.0.1 package
        // tags unsignedInt.value and positiveInt.value as `string`.
        let is_value = element
            .path
            .rsplit_once('.')
            .is_some_and(|(_, last)| last == "value");
        let scalar_name = if is_primitive && is_value {
            structure.name.as_str()
        } else {
            only.code.as_str()
        };
        Ok(Target::Inline(scalar_for(scalar_name)))
    } else if only.code == "BackboneElement" || only.code == "Element" {
        let nested = backbone_name(&element.path);
        lower_struct(
            model,
            structure,
            &element.path,
            &nested,
            module,
            false,
            false,
        )?;
        Ok(Target::Named(nested))
    } else if only.code == "Resource" {
        Ok(Target::Named(RESOURCE_ENUM.to_owned()))
    } else {
        Ok(Target::Named(type_name(&only.code)))
    }
}

/// The Rust scalar for a FHIR primitive's value, by primitive name.
///
/// The package types positiveInt and unsignedInt values as System.String;
/// the FHIR JSON representation carries them as numbers, so the scalar follows
/// the primitive's definition (<https://hl7.org/fhir/R4B/datatypes.html#primitive>).
/// Decimals and the date and time primitives keep their lexical form so
/// precision and partial dates survive.
pub(crate) fn scalar_for(code: &str) -> Scalar {
    match code {
        "boolean" => Scalar::Bool,
        "integer" => Scalar::I32,
        "positiveInt" | "unsignedInt" => Scalar::U32,
        "integer64" => Scalar::I64,
        _ => Scalar::Str,
    }
}

/// Tarjan's algorithm over the containment graph; components in a stable order.
fn strongly_connected_components(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut search = TarjanSearch {
        edges,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        on_stack: BTreeSet::new(),
        stack: Vec::new(),
        next: 0,
        components: Vec::new(),
    };
    for node in edges.keys() {
        if !search.index.contains_key(node) {
            search.visit(node);
        }
    }
    search.components
}

/// One depth-first search of Tarjan's algorithm, and the components it found.
struct TarjanSearch<'a> {
    edges: &'a BTreeMap<String, BTreeSet<String>>,
    index: BTreeMap<String, usize>,
    low: BTreeMap<String, usize>,
    on_stack: BTreeSet<String>,
    stack: Vec<String>,
    next: usize,
    components: Vec<Vec<String>>,
}

impl TarjanSearch<'_> {
    /// Visits `node`, its unvisited successors, and pops a component at a root.
    fn visit(&mut self, node: &str) {
        self.index.insert(node.to_owned(), self.next);
        self.low.insert(node.to_owned(), self.next);
        self.next += 1;
        self.stack.push(node.to_owned());
        self.on_stack.insert(node.to_owned());
        let successors: Vec<String> = self
            .edges
            .get(node)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        for next in successors {
            if !self.index.contains_key(&next) {
                self.visit(&next);
                let next_low = self.low.get(&next).copied().unwrap_or(usize::MAX);
                let own = self.low.get(node).copied().unwrap_or(usize::MAX);
                self.low.insert(node.to_owned(), own.min(next_low));
            } else if self.on_stack.contains(&next) {
                let next_index = self.index.get(&next).copied().unwrap_or(usize::MAX);
                let own = self.low.get(node).copied().unwrap_or(usize::MAX);
                self.low.insert(node.to_owned(), own.min(next_index));
            }
        }
        if self.low.get(node) == self.index.get(node) {
            let component = self.pop_component(node);
            self.components.push(component);
        }
    }

    /// Pops the stack down to `root`, the component it closes, in name order.
    fn pop_component(&mut self, root: &str) -> Vec<String> {
        let mut component = Vec::new();
        while let Some(top) = self.stack.pop() {
            self.on_stack.remove(&top);
            let done = top == root;
            component.push(top);
            if done {
                break;
            }
        }
        component.sort();
        component
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::strongly_connected_components;

    fn graph(pairs: &[(&str, &str)]) -> BTreeMap<String, BTreeSet<String>> {
        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (from, to) in pairs {
            edges
                .entry((*from).to_owned())
                .or_default()
                .insert((*to).to_owned());
            edges.entry((*to).to_owned()).or_default();
        }
        edges
    }

    #[test]
    fn a_two_cycle_is_one_component() {
        let components = strongly_connected_components(&graph(&[
            ("Identifier", "Reference"),
            ("Reference", "Identifier"),
            ("Coding", "Identifier"),
        ]));
        assert!(components.contains(&vec!["Identifier".to_owned(), "Reference".to_owned()]));
        assert!(components.contains(&vec!["Coding".to_owned()]));
    }

    #[test]
    fn a_dag_has_singleton_components() {
        let components = strongly_connected_components(&graph(&[("A", "B"), ("B", "C")]));
        assert_eq!(components.len(), 3);
        assert!(components.iter().all(|c| c.len() == 1));
    }
}
