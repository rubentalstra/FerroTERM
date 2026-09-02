//! The root-set closure: every type the root resources reach.
//!
//! Starting from the root resources, the closure follows every element type
//! and every content reference to the `StructureDefinition` that defines it,
//! transitively, so the emitted module holds the complete set of datatypes
//! and primitives the terminology surface can carry, and nothing else
//! (`codegen.md`: complete within a declared closure).

use std::collections::{BTreeMap, BTreeSet};

use crate::fhir::{Derivation, StructureKind};
use crate::package::Package;
use crate::roots::RootSet;
use crate::snapshot::{ElementShape, ResolveError, ResolvedStructure};

/// Type codes that name a structural base rather than a datatype to emit.
///
/// `Element` and `BackboneElement` mark nested structures, which are emitted
/// as part of their parent; `Resource` is the abstract resource type, emitted
/// as the `Resource` enum over the root set.
pub const STRUCTURAL_TYPES: [&str; 3] = ["BackboneElement", "Element", "Resource"];

/// A failure while computing the closure.
#[derive(Debug, thiserror::Error)]
pub enum ClosureError {
    /// An element names a type the package does not define.
    #[error("{path} has type {code}, which the package does not define")]
    UnknownType {
        /// The element path.
        path: String,
        /// The type code.
        code: String,
    },
    /// An element names a type whose definition is a profile, not a type.
    #[error("{path} has type {code}, whose definition is a constraint profile, not a type")]
    ProfileAsType {
        /// The element path.
        path: String,
        /// The type code.
        code: String,
    },
    /// A snapshot failed to resolve.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
}

/// The resolved structures of the root set and everything they reference.
#[derive(Debug)]
pub struct TypeClosure {
    structures: BTreeMap<String, ResolvedStructure>,
    roots: BTreeSet<String>,
}

impl TypeClosure {
    /// Computes the closure of `roots` over `package`.
    ///
    /// # Errors
    ///
    /// Returns [`ClosureError`] when an element names an undefined type or a
    /// profile, or when a snapshot does not resolve.
    pub fn compute(package: &Package, roots: &RootSet<'_>) -> Result<Self, ClosureError> {
        let mut structures = BTreeMap::new();
        let mut pending: Vec<(String, String)> = roots
            .resources
            .values()
            .map(|definition| (definition.name.clone(), String::from("(root)")))
            .collect();
        let root_names: BTreeSet<String> = pending.iter().map(|(name, _)| name.clone()).collect();

        while let Some((code, referrer)) = pending.pop() {
            if structures.contains_key(&code) || STRUCTURAL_TYPES.contains(&code.as_str()) {
                continue;
            }
            let definition = package.structure_definition_named(&code).ok_or_else(|| {
                ClosureError::UnknownType {
                    path: referrer.clone(),
                    code: code.clone(),
                }
            })?;
            if definition.derivation == Some(Derivation::Constraint) {
                return Err(ClosureError::ProfileAsType {
                    path: referrer,
                    code,
                });
            }
            let resolved = ResolvedStructure::resolve(definition)?;
            if resolved.kind != StructureKind::PrimitiveType {
                for element in &resolved.elements {
                    match &element.shape {
                        ElementShape::Typed(types) | ElementShape::Choice(types) => {
                            for type_ref in types {
                                if type_ref.fhirpath_type.is_none() {
                                    pending.push((type_ref.code.clone(), element.path.clone()));
                                }
                            }
                        }
                        ElementShape::ContentReference {
                            structure: Some(url),
                            ..
                        } => {
                            if let Some(target) = package.structure_definitions().get(url) {
                                pending.push((target.name.clone(), element.path.clone()));
                            }
                        }
                        ElementShape::ContentReference { .. } | ElementShape::Root => {}
                    }
                }
            }
            structures.insert(code, resolved);
        }
        Ok(Self {
            structures,
            roots: root_names,
        })
    }

    /// Every structure in the closure, keyed by type name, in name order.
    #[must_use]
    pub fn structures(&self) -> &BTreeMap<String, ResolvedStructure> {
        &self.structures
    }

    /// The names of the root resources.
    #[must_use]
    pub fn roots(&self) -> &BTreeSet<String> {
        &self.roots
    }

    /// The structures of one kind, in name order.
    pub fn of_kind(&self, kind: StructureKind) -> impl Iterator<Item = &ResolvedStructure> {
        self.structures
            .values()
            .filter(move |structure| structure.kind == kind)
    }
}
