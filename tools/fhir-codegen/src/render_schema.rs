//! The per-version XML schema: every struct type's elements in definition
//! order, each with the kind the XML codec needs.
//!
//! The schema is the one input of the runtime XML module (`xml.rs`), so the
//! generated footprint of XML stays one static per version while the JSON
//! codec keeps the strictness (<https://hl7.org/fhir/R4B/xml.html>).

use std::fmt::{self, Write};

use crate::lower::{Cardinality, RESOURCE_ENUM, Scalar, Target, TypeDef, TypeKind, VersionModule};
use crate::naming::type_name;

/// Renders the `schema.rs` module of `model`.
///
/// # Errors
///
/// Returns [`fmt::Error`] only if writing to the string fails, which `String` never does.
pub fn render_schema(model: &VersionModule) -> Result<String, fmt::Error> {
    let mut out = crate::render::banner(model);
    out.push_str(
        "//! The XML schema of the version's types: each element's kind in definition\n//! order, the one input of the XML codec (<https://hl7.org/fhir/R4B/xml.html>).\n\n",
    );
    out.push_str("use super::super::xml::{FieldSchema, Kind, Schemas, TypeSchema, ValueKind};\n\n");
    out.push_str("/// The version's schema.\n");
    out.push_str("pub static SCHEMAS: Schemas = Schemas {\n    types: &[\n");
    for ty in model.types.values() {
        let TypeKind::Struct { fields } = &ty.kind else {
            continue;
        };
        if ty.is_primitive {
            continue;
        }
        writeln!(
            out,
            "        TypeSchema {{\n            name: {:?},\n            fields: &[",
            ty.name
        )?;
        for field in fields {
            let many = field.ty.card == Cardinality::Many;
            let name = field.fhir_name.trim_end_matches("[x]");
            // NOTE: `Resource.id` is an element with a `value` attribute while
            // `Element.id` is an attribute (<https://hl7.org/fhir/R4B/xml.html>);
            // both are plain strings in the model.
            let kind = if ty.is_resource && name == "id" {
                String::from("Kind::Primitive(ValueKind::Text)")
            } else {
                kind_of(model, &field.ty.target)
            };
            writeln!(
                out,
                "                FieldSchema {{ name: {name:?}, kind: {kind}, many: {many} }},"
            )?;
        }
        out.push_str("            ],\n        },\n");
    }
    out.push_str("    ],\n    resources: &[\n");
    for ty in model.types.values().filter(|t| t.is_resource) {
        writeln!(out, "        {:?},", ty.name)?;
    }
    out.push_str("    ],\n};\n");
    Ok(out)
}

/// The XML kind of a field or variant target.
fn kind_of(model: &VersionModule, target: &Target) -> String {
    match target {
        Target::Inline(_) => String::from("Kind::Attribute"),
        Target::Named(name) if name == RESOURCE_ENUM => String::from("Kind::Resource"),
        Target::Named(name) => match model.types.get(name) {
            Some(ty) if ty.is_primitive => primitive_kind(ty),
            Some(TypeDef {
                kind: TypeKind::Choice { variants, .. },
                ..
            }) => {
                let arms: Vec<String> = variants
                    .iter()
                    .map(|v| format!("({:?}, {})", type_name(&v.code), kind_of(model, &v.target)))
                    .collect();
                format!("Kind::Choice(&[{}])", arms.join(", "))
            }
            _ => format!("Kind::Complex({name:?})"),
        },
    }
}

/// The kind of a primitive type: XHTML for `Xhtml`, else the value scalar's
/// JSON form (<https://hl7.org/fhir/R4B/json.html#primitive>).
fn primitive_kind(ty: &TypeDef) -> String {
    if ty.name == "Xhtml" {
        return String::from("Kind::Xhtml");
    }
    let scalar = match &ty.kind {
        TypeKind::Struct { fields } => fields
            .iter()
            .find(|f| f.name == "value")
            .and_then(|f| match f.ty.target {
                Target::Inline(s) => Some(s),
                Target::Named(_) => None,
            })
            .unwrap_or(Scalar::Str),
        _ => Scalar::Str,
    };
    let value_kind = match (scalar, ty.name.as_str()) {
        (Scalar::Bool, _) => "Boolean",
        (Scalar::I32 | Scalar::U32, _) => "Integer",
        (Scalar::Str, "Decimal") => "Decimal",
        (Scalar::I64 | Scalar::Str, _) => "Text",
    };
    format!("Kind::Primitive(ValueKind::{value_kind})")
}
