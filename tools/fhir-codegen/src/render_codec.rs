//! Rendering the JSON codec impls beside each generated type.
//!
//! The runtime lives in the generated `codec` module (from the template in
//! `templates/codec.rs`); this module writes, per type, the `Json` or
//! `Primitive` impl and the serde bridges, following the FHIR JSON
//! representation (<https://hl7.org/fhir/R4B/json.html>).

use std::fmt::{self, Write};

use crate::lower::{Cardinality, Field, Scalar, Target, TypeDef, TypeKind, VersionModule};
use crate::naming::type_name;

/// The codec module path from a type file (`<version>/<module>.rs`).
const C: &str = "super::super::codec";

/// What a field holds, from the codec's point of view.
enum Shape<'a> {
    /// A `FHIRPath` system scalar (`id`, `url`): a bare JSON value.
    Scalar(Scalar),
    /// A FHIR primitive: value plus `_name` sibling.
    Primitive,
    /// A complex type, backbone struct, or the `Resource` enum: a JSON object.
    Complex,
    /// A choice enum: one of several keys.
    Choice(&'a TypeDef),
}

fn shape<'a>(model: &'a VersionModule, field: &Field) -> Shape<'a> {
    match &field.ty.target {
        Target::Inline(scalar) => Shape::Scalar(*scalar),
        Target::Named(name) => match model.types.get(name) {
            Some(ty) if ty.is_primitive => Shape::Primitive,
            Some(ty) if matches!(ty.kind, TypeKind::Choice { .. }) => Shape::Choice(ty),
            _ => Shape::Complex,
        },
    }
}

/// The scalar's JSON constructor expression over `v` (a reference).
fn scalar_to_value(scalar: Scalar, v: &str) -> String {
    match scalar {
        Scalar::Bool => format!("serde_json::Value::Bool(*{v})"),
        Scalar::I32 | Scalar::U32 => format!("serde_json::Value::from(*{v})"),
        Scalar::I64 => format!("serde_json::Value::String({v}.to_string())"),
        Scalar::Str => format!("serde_json::Value::String({v}.clone())"),
    }
}

/// The scalar's JSON constructor expression over `v` (an owned place).
fn scalar_to_value_owned(scalar: Scalar, v: &str) -> String {
    match scalar {
        Scalar::Bool => format!("serde_json::Value::Bool({v})"),
        Scalar::I32 | Scalar::U32 => format!("serde_json::Value::from({v})"),
        Scalar::I64 => format!("serde_json::Value::String({v}.to_string())"),
        Scalar::Str => format!("serde_json::Value::String({v}.clone())"),
    }
}

/// The scalar's decoder expression over `value` (a `&Value`) and `path` (a `&Path`).
fn scalar_from_value(scalar: Scalar, primitive: Option<&str>) -> String {
    match (scalar, primitive) {
        (Scalar::Bool, _) => format!("{C}::expect_bool(value, path)"),
        (Scalar::I32, _) => format!("{C}::expect_i32(value, path)"),
        (Scalar::U32, Some("PositiveInt")) => format!("{C}::expect_u32(value, 1, path)"),
        (Scalar::U32, _) => format!("{C}::expect_u32(value, 0, path)"),
        (Scalar::I64, _) => format!("{C}::expect_i64_string(value, path)"),
        (Scalar::Str, Some("Decimal")) => format!("{C}::expect_decimal(value, path)"),
        (Scalar::Str, _) => format!("{C}::expect_string(value, path)"),
    }
}

/// Renders the codec impls for `ty`.
///
/// # Errors
///
/// Returns [`fmt::Error`] only if writing to the string fails, which `String` never does.
pub fn render_codec(model: &VersionModule, out: &mut String, ty: &TypeDef) -> fmt::Result {
    match &ty.kind {
        TypeKind::Struct { fields } if ty.is_primitive => render_primitive(out, ty, fields),
        TypeKind::Struct { fields } => {
            render_struct(model, out, ty, fields)?;
            render_serde_bridge(out, &ty.name)
        }
        TypeKind::Choice { variants, .. } => render_choice(model, out, ty, variants),
        TypeKind::ResourceEnum { resources } => {
            render_resource_enum(out, ty, resources)?;
            render_serde_bridge(out, &ty.name)
        }
        TypeKind::UnknownResource => Ok(()),
    }
}

fn render_serde_bridge(out: &mut String, name: &str) -> fmt::Result {
    writeln!(out, "\nimpl serde::Serialize for {name} {{")?;
    writeln!(
        out,
        "    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {{"
    )?;
    writeln!(out, "        {C}::Json::to_json(self)")?;
    writeln!(out, "            .map_err(serde::ser::Error::custom)?")?;
    writeln!(out, "            .serialize(serializer)")?;
    writeln!(out, "    }}\n}}\n")?;
    writeln!(out, "impl<'de> serde::Deserialize<'de> for {name} {{")?;
    writeln!(
        out,
        "    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{"
    )?;
    writeln!(
        out,
        "        let value = serde_json::Value::deserialize(deserializer)?;"
    )?;
    writeln!(out, "        let mut path = {C}::Path::root({name:?});")?;
    writeln!(
        out,
        "        let object = {C}::expect_object(&value, &path).map_err(serde::de::Error::custom)?;"
    )?;
    writeln!(
        out,
        "        {C}::Json::from_json(object, &mut path).map_err(serde::de::Error::custom)"
    )?;
    writeln!(out, "    }}\n}}")
}

fn render_primitive(out: &mut String, ty: &TypeDef, fields: &[Field]) -> fmt::Result {
    let value_field = fields.iter().find(|f| f.name == "value");
    let scalar = value_field
        .and_then(|f| match f.ty.target {
            Target::Inline(s) => Some(s),
            Target::Named(_) => None,
        })
        .unwrap_or(Scalar::Str);
    let value_required = value_field.is_some_and(|f| f.ty.card == Cardinality::One);
    let has_extension = fields.iter().any(|f| f.name == "extension");
    // R6 prohibits `xhtml.id` (max 0), so a primitive may carry no id at all.
    let has_id = fields.iter().any(|f| f.name == "id");
    writeln!(out, "\nimpl {C}::Primitive for {} {{", ty.name)?;
    writeln!(
        out,
        "    fn value_json(&self) -> Result<Option<serde_json::Value>, {C}::EncodeError> {{"
    )?;
    let held = if value_required {
        "Some(&self.value)"
    } else {
        "self.value.as_ref()"
    };
    match (scalar, ty.name.as_str()) {
        (Scalar::Str, "Decimal") => {
            writeln!(out, "        {held}.map(|text| {{")?;
            writeln!(out, "            text.parse::<serde_json::Number>()")?;
            writeln!(out, "                .map(serde_json::Value::Number)")?;
            writeln!(
                out,
                "                .map_err(|_| {C}::EncodeError::BadDecimal {{ text: text.clone() }})"
            )?;
            writeln!(out, "        }}).transpose()")?;
        }
        _ => writeln!(
            out,
            "        Ok({held}.map(|v| {}))",
            scalar_to_value(scalar, "v")
        )?,
    }
    writeln!(out, "    }}\n")?;
    writeln!(
        out,
        "    fn element_json(&self) -> Result<Option<serde_json::Value>, {C}::EncodeError> {{"
    )?;
    match (has_id, has_extension) {
        (true, true) => writeln!(
            out,
            "        if self.id.is_none() && self.extension.is_empty() {{"
        )?,
        (true, false) => writeln!(out, "        if self.id.is_none() {{")?,
        (false, true) => writeln!(out, "        if self.extension.is_empty() {{")?,
        (false, false) => {}
    }
    if has_id || has_extension {
        writeln!(out, "            return Ok(None);")?;
        writeln!(out, "        }}")?;
        writeln!(out, "        let mut object = {C}::Object::new();")?;
    } else {
        // Nothing but the value: the element form never carries anything.
        writeln!(out, "        Ok(None)")?;
    }
    if has_id {
        writeln!(out, "        if let Some(id) = &self.id {{")?;
        writeln!(
            out,
            "            object.insert(std::string::String::from(\"id\"), serde_json::Value::String(id.clone()));"
        )?;
        writeln!(out, "        }}")?;
    }
    if has_extension {
        writeln!(out, "        if !self.extension.is_empty() {{")?;
        writeln!(
            out,
            "            let mut items = Vec::with_capacity(self.extension.len());"
        )?;
        writeln!(out, "            for item in &self.extension {{")?;
        writeln!(
            out,
            "                items.push(serde_json::Value::Object({C}::Json::to_json(item)?));"
        )?;
        writeln!(out, "            }}")?;
        writeln!(
            out,
            "            object.insert(std::string::String::from(\"extension\"), serde_json::Value::Array(items));"
        )?;
        writeln!(out, "        }}")?;
    }
    if has_id || has_extension {
        writeln!(out, "        Ok(Some(serde_json::Value::Object(object)))")?;
    }
    writeln!(out, "    }}\n")?;
    render_primitive_decode(out, ty, scalar, value_required, has_id, has_extension)?;
    writeln!(out, "}}")
}

fn render_primitive_decode(
    out: &mut String,
    ty: &TypeDef,
    scalar: Scalar,
    value_required: bool,
    has_id: bool,
    has_extension: bool,
) -> fmt::Result {
    writeln!(out, "    fn from_json_parts(")?;
    writeln!(out, "        value: Option<&serde_json::Value>,")?;
    writeln!(out, "        element: Option<&serde_json::Value>,")?;
    writeln!(out, "        path: &mut {C}::Path,")?;
    writeln!(out, "    ) -> Result<Self, {C}::DecodeError> {{")?;
    writeln!(out, "        if value.is_none() && element.is_none() {{")?;
    writeln!(
        out,
        "            return Err(path.error({C}::DecodeErrorKind::MissingProperty));"
    )?;
    writeln!(out, "        }}")?;
    writeln!(out, "        let value = match value {{")?;
    writeln!(out, "            Some(value) => {{")?;
    writeln!(
        out,
        "                let value = {C}::expect_single(value, path)?;"
    )?;
    writeln!(
        out,
        "                Some({}?)",
        scalar_from_value(scalar, Some(ty.name.as_str()))
    )?;
    writeln!(out, "            }}")?;
    writeln!(out, "            None => None,")?;
    writeln!(out, "        }};")?;
    if value_required {
        writeln!(
            out,
            "        let value = value.ok_or_else(|| path.error({C}::DecodeErrorKind::MissingProperty))?;"
        )?;
    }
    if has_id {
        writeln!(out, "        let mut id = None;")?;
    }
    if has_extension {
        writeln!(out, "        let mut extension = Vec::new();")?;
    }
    writeln!(out, "        if let Some(element) = element {{")?;
    writeln!(
        out,
        "            let object = {C}::expect_object(element, path)?;"
    )?;
    if !has_id && !has_extension {
        // No element property is admitted: the first key is the error.
        writeln!(
            out,
            "            if let Some(key) = object.keys().next() {{"
        )?;
        writeln!(
            out,
            "                return path.with(key, |path| Err(path.error({C}::DecodeErrorKind::UnknownProperty)));"
        )?;
        writeln!(out, "            }}")?;
        writeln!(out, "        }}")?;
        writeln!(out, "        Ok(Self {{ value }})")?;
        writeln!(out, "    }}")?;
        return Ok(());
    }
    writeln!(out, "            for (key, item) in object {{")?;
    writeln!(out, "                match key.as_str() {{")?;
    if has_id {
        writeln!(
            out,
            "                    \"id\" => {{ id = Some(path.with(\"id\", |path| {C}::expect_string(item, path))?); }}"
        )?;
    }
    if has_extension {
        writeln!(out, "                    \"extension\" => {{")?;
        writeln!(
            out,
            "                        for (index, entry) in {C}::expect_array(item, path)?.iter().enumerate() {{"
        )?;
        writeln!(
            out,
            "                            extension.push(path.with_index(\"extension\", index, |path| {{"
        )?;
        writeln!(
            out,
            "                                {C}::Json::from_json({C}::expect_object(entry, path)?, path)"
        )?;
        writeln!(out, "                            }})?);")?;
        writeln!(out, "                        }}")?;
        writeln!(out, "                    }}")?;
    }
    writeln!(
        out,
        "                    _ => return path.with(key, |path| Err(path.error({C}::DecodeErrorKind::UnknownProperty))),"
    )?;
    writeln!(out, "                }}")?;
    writeln!(out, "            }}")?;
    writeln!(out, "        }}")?;
    let mut init = Vec::new();
    if has_id {
        init.push("id");
    }
    if has_extension {
        init.push("extension");
    }
    init.push("value");
    writeln!(out, "        Ok(Self {{ {} }})", init.join(", "))?;
    writeln!(out, "    }}")
}

fn render_choice(
    model: &VersionModule,
    out: &mut String,
    ty: &TypeDef,
    variants: &[crate::lower::Variant],
) -> fmt::Result {
    writeln!(out, "\nimpl {} {{", ty.name)?;
    render_choice_to_json_parts(model, out, variants)?;
    render_choice_from_json_parts(model, out, variants)?;
    writeln!(out, "}}")
}

/// Whether the variant holds a FHIR primitive, which travels as value plus
/// `_name`.
fn holds_primitive(model: &VersionModule, variant: &crate::lower::Variant) -> bool {
    matches!(&variant.target, Target::Named(name) if model.types.get(name).is_some_and(|ty| ty.is_primitive))
}

/// The choice enum's `to_json_parts`, one match arm per variant.
fn render_choice_to_json_parts(
    model: &VersionModule,
    out: &mut String,
    variants: &[crate::lower::Variant],
) -> fmt::Result {
    writeln!(
        out,
        "    /// The key suffix, the value part, and the `_name` part of this form."
    )?;
    writeln!(out, "    ///")?;
    writeln!(out, "    /// # Errors")?;
    writeln!(out, "    ///")?;
    writeln!(
        out,
        "    /// Returns [`{C}::EncodeError`] when a held value has no JSON form."
    )?;
    writeln!(
        out,
        "    pub fn to_json_parts(&self) -> Result<(&'static str, Option<serde_json::Value>, Option<serde_json::Value>), {C}::EncodeError> {{"
    )?;
    writeln!(out, "        match self {{")?;
    for variant in variants {
        let suffix = type_name(&variant.code);
        if holds_primitive(model, variant) {
            writeln!(
                out,
                "            Self::{}(inner) => Ok(({suffix:?}, {C}::Primitive::value_json(inner)?, {C}::Primitive::element_json(inner)?)),",
                variant.name
            )?;
        } else {
            let deref = if variant.boxed {
                "inner.as_ref()"
            } else {
                "inner"
            };
            writeln!(
                out,
                "            Self::{}(inner) => Ok(({suffix:?}, Some(serde_json::Value::Object({C}::Json::to_json({deref})?)), None)),",
                variant.name
            )?;
        }
    }
    writeln!(out, "        }}\n    }}\n")
}

/// The choice enum's `from_json_parts`, one match arm per variant suffix.
fn render_choice_from_json_parts(
    model: &VersionModule,
    out: &mut String,
    variants: &[crate::lower::Variant],
) -> fmt::Result {
    writeln!(
        out,
        "    /// Decodes the form named by `suffix` from its value and `_name` parts."
    )?;
    writeln!(out, "    ///")?;
    writeln!(out, "    /// # Errors")?;
    writeln!(out, "    ///")?;
    writeln!(
        out,
        "    /// Returns [`{C}::DecodeError`] for an unknown suffix or a malformed part."
    )?;
    writeln!(out, "    pub fn from_json_parts(")?;
    writeln!(out, "        suffix: &str,")?;
    writeln!(out, "        value: Option<&serde_json::Value>,")?;
    writeln!(out, "        element: Option<&serde_json::Value>,")?;
    writeln!(out, "        path: &mut {C}::Path,")?;
    writeln!(out, "    ) -> Result<Self, {C}::DecodeError> {{")?;
    writeln!(out, "        match suffix {{")?;
    for variant in variants {
        let suffix = type_name(&variant.code);
        if holds_primitive(model, variant) {
            writeln!(
                out,
                "            {suffix:?} => Ok(Self::{}({C}::Primitive::from_json_parts(value, element, path)?)),",
                variant.name
            )?;
        } else {
            let wrap = if variant.boxed {
                "Box::new(inner)"
            } else {
                "inner"
            };
            writeln!(out, "            {suffix:?} => {{")?;
            writeln!(out, "                if element.is_some() {{")?;
            writeln!(
                out,
                "                    return Err(path.error({C}::DecodeErrorKind::WrongType {{ expected: \"no underscore form for a complex type\" }}));"
            )?;
            writeln!(out, "                }}")?;
            writeln!(
                out,
                "                let value = value.ok_or_else(|| path.error({C}::DecodeErrorKind::MissingProperty))?;"
            )?;
            writeln!(
                out,
                "                let inner = {C}::Json::from_json({C}::expect_object({C}::expect_single(value, path)?, path)?, path)?;"
            )?;
            writeln!(out, "                Ok(Self::{}({wrap}))", variant.name)?;
            writeln!(out, "            }}")?;
        }
    }
    writeln!(
        out,
        "            _ => Err(path.error({C}::DecodeErrorKind::UnknownProperty)),"
    )?;
    writeln!(out, "        }}\n    }}")
}

fn render_resource_enum(out: &mut String, ty: &TypeDef, resources: &[String]) -> fmt::Result {
    writeln!(out, "\nimpl {C}::Json for {} {{", ty.name)?;
    writeln!(
        out,
        "    fn to_json(&self) -> Result<{C}::Object, {C}::EncodeError> {{"
    )?;
    writeln!(out, "        match self {{")?;
    for resource in resources {
        writeln!(
            out,
            "            Self::{resource}(inner) => {C}::Json::to_json(inner.as_ref()),"
        )?;
    }
    writeln!(
        out,
        "            Self::Unknown(inner) => match &inner.body {{"
    )?;
    writeln!(
        out,
        "                serde_json::Value::Object(object) => Ok(object.clone()),"
    )?;
    writeln!(
        out,
        "                _ => Err({C}::EncodeError::UnknownResourceBody),"
    )?;
    writeln!(out, "            }},")?;
    writeln!(out, "        }}\n    }}\n")?;
    writeln!(
        out,
        "    fn from_json(object: &{C}::Object, path: &mut {C}::Path) -> Result<Self, {C}::DecodeError> {{"
    )?;
    writeln!(out, "        match {C}::resource_type(object, path)? {{")?;
    for resource in resources {
        writeln!(
            out,
            "            {resource:?} => Ok(Self::{resource}(Box::new({C}::Json::from_json(object, path)?))),"
        )?;
    }
    writeln!(
        out,
        "            other => Ok(Self::Unknown(UnknownResource {{"
    )?;
    writeln!(out, "                resource_type: other.to_owned(),")?;
    writeln!(
        out,
        "                body: serde_json::Value::Object(object.clone()),"
    )?;
    writeln!(out, "            }})),")?;
    writeln!(out, "        }}\n    }}\n}}")
}

fn render_struct(
    model: &VersionModule,
    out: &mut String,
    ty: &TypeDef,
    fields: &[Field],
) -> fmt::Result {
    writeln!(out, "\nimpl {C}::Json for {} {{", ty.name)?;
    render_to_json(model, out, ty, fields)?;
    writeln!(out)?;
    render_from_json(model, out, ty, fields)?;
    writeln!(out, "}}")
}

fn render_to_json(
    model: &VersionModule,
    out: &mut String,
    ty: &TypeDef,
    fields: &[Field],
) -> fmt::Result {
    writeln!(
        out,
        "    fn to_json(&self) -> Result<{C}::Object, {C}::EncodeError> {{"
    )?;
    writeln!(out, "        let mut object = {C}::Object::new();")?;
    if ty.is_resource {
        writeln!(
            out,
            "        object.insert(std::string::String::from(\"resourceType\"), serde_json::Value::String(std::string::String::from({:?})));",
            ty.name
        )?;
    }
    for field in fields {
        render_field_to_json(model, out, field)?;
    }
    writeln!(out, "        Ok(object)")?;
    writeln!(out, "    }}")
}

/// Writes one field into the object under construction.
#[expect(
    clippy::too_many_lines,
    reason = "one match arm per FHIR field shape and cardinality; splitting scatters the table"
)]
fn render_field_to_json(model: &VersionModule, out: &mut String, field: &Field) -> fmt::Result {
    let key = &field.fhir_name;
    let access = format!("self.{}", field.name);
    match (shape(model, field), field.ty.card) {
        (Shape::Scalar(scalar), Cardinality::Optional) => {
            writeln!(out, "        if let Some(v) = &{access} {{")?;
            writeln!(
                out,
                "            object.insert(std::string::String::from({key:?}), {});",
                scalar_to_value(scalar, "v")
            )?;
            writeln!(out, "        }}")?;
        }
        (Shape::Scalar(scalar), Cardinality::One) => {
            writeln!(
                out,
                "        object.insert(std::string::String::from({key:?}), {});",
                scalar_to_value_owned(scalar, &access)
            )?;
        }
        (Shape::Scalar(scalar), Cardinality::Many) => {
            writeln!(out, "        if !{access}.is_empty() {{")?;
            writeln!(
                out,
                "            object.insert(std::string::String::from({key:?}), serde_json::Value::Array({access}.iter().map(|v| {}).collect()));",
                scalar_to_value(scalar, "v")
            )?;
            writeln!(out, "        }}")?;
        }
        (Shape::Primitive, Cardinality::One | Cardinality::Optional) => {
            let (open, close, item) = if field.ty.card == Cardinality::Optional {
                (
                    format!("        if let Some(item) = &{access} {{\n"),
                    "        }\n",
                    String::from("item"),
                )
            } else {
                (String::new(), "", format!("&{access}"))
            };
            let indent = if field.ty.card == Cardinality::Optional {
                "            "
            } else {
                "        "
            };
            out.push_str(&open);
            writeln!(
                out,
                "{indent}if let Some(v) = {C}::Primitive::value_json({item})? {{"
            )?;
            writeln!(
                out,
                "{indent}    object.insert(std::string::String::from({key:?}), v);"
            )?;
            writeln!(out, "{indent}}}")?;
            writeln!(
                out,
                "{indent}if let Some(e) = {C}::Primitive::element_json({item})? {{"
            )?;
            writeln!(
                out,
                "{indent}    object.insert(std::string::String::from(\"_{key}\"), e);"
            )?;
            writeln!(out, "{indent}}}")?;
            out.push_str(close);
        }
        (Shape::Primitive, Cardinality::Many) => {
            writeln!(out, "        if !{access}.is_empty() {{")?;
            writeln!(
                out,
                "            let (values, elements) = {C}::primitive_arrays(&{access})?;"
            )?;
            writeln!(
                out,
                "            object.insert(std::string::String::from({key:?}), values);"
            )?;
            writeln!(out, "            if let Some(elements) = elements {{")?;
            writeln!(
                out,
                "                object.insert(std::string::String::from(\"_{key}\"), elements);"
            )?;
            writeln!(out, "            }}")?;
            writeln!(out, "        }}")?;
        }
        (Shape::Complex, Cardinality::One) => {
            let inner = if field.ty.boxed {
                format!("{access}.as_ref()")
            } else {
                format!("&{access}")
            };
            writeln!(
                out,
                "        object.insert(std::string::String::from({key:?}), serde_json::Value::Object({C}::Json::to_json({inner})?));"
            )?;
        }
        (Shape::Complex, Cardinality::Optional) => {
            let inner = if field.ty.boxed {
                "item.as_ref()"
            } else {
                "item"
            };
            writeln!(out, "        if let Some(item) = &{access} {{")?;
            writeln!(
                out,
                "            object.insert(std::string::String::from({key:?}), serde_json::Value::Object({C}::Json::to_json({inner})?));"
            )?;
            writeln!(out, "        }}")?;
        }
        (Shape::Complex, Cardinality::Many) => {
            writeln!(out, "        if !{access}.is_empty() {{")?;
            writeln!(
                out,
                "            let mut items = Vec::with_capacity({access}.len());"
            )?;
            writeln!(out, "            for item in &{access} {{")?;
            writeln!(
                out,
                "                items.push(serde_json::Value::Object({C}::Json::to_json(item)?));"
            )?;
            writeln!(out, "            }}")?;
            writeln!(
                out,
                "            object.insert(std::string::String::from({key:?}), serde_json::Value::Array(items));"
            )?;
            writeln!(out, "        }}")?;
        }
        (Shape::Choice(_), card) => {
            let stem = key.trim_end_matches("[x]");
            let (open, close, item, indent) = match card {
                Cardinality::Optional => (
                    format!("        if let Some(item) = &{access} {{\n"),
                    "        }\n",
                    String::from("item"),
                    "            ",
                ),
                _ => (String::new(), "", access.clone(), "        "),
            };
            out.push_str(&open);
            writeln!(
                out,
                "{indent}let (suffix, value, element) = {item}.to_json_parts()?;"
            )?;
            writeln!(out, "{indent}if let Some(value) = value {{")?;
            writeln!(
                out,
                "{indent}    object.insert(format!(\"{stem}{{suffix}}\"), value);"
            )?;
            writeln!(out, "{indent}}}")?;
            writeln!(out, "{indent}if let Some(element) = element {{")?;
            writeln!(
                out,
                "{indent}    object.insert(format!(\"_{stem}{{suffix}}\"), element);"
            )?;
            writeln!(out, "{indent}}}")?;
            out.push_str(close);
        }
    }
    Ok(())
}

fn render_from_json(
    model: &VersionModule,
    out: &mut String,
    ty: &TypeDef,
    fields: &[Field],
) -> fmt::Result {
    writeln!(
        out,
        "    fn from_json(object: &{C}::Object, path: &mut {C}::Path) -> Result<Self, {C}::DecodeError> {{"
    )?;
    // Raw slots: one per field, holding borrowed JSON until the loop finishes.
    for field in fields {
        let slot = &field.name.trim_start_matches("r#");
        match shape(model, field) {
            Shape::Choice(_) => writeln!(
                out,
                "        let mut raw_{slot} = {C}::ChoiceSlot::default();"
            )?,
            Shape::Primitive => {
                writeln!(
                    out,
                    "        let mut raw_{slot}: Option<&serde_json::Value> = None;"
                )?;
                writeln!(
                    out,
                    "        let mut raw_{slot}_element: Option<&serde_json::Value> = None;"
                )?;
            }
            Shape::Scalar(_) | Shape::Complex => writeln!(
                out,
                "        let mut raw_{slot}: Option<&serde_json::Value> = None;"
            )?,
        }
    }
    writeln!(out, "        for (key, value) in object {{")?;
    writeln!(out, "            match key.as_str() {{")?;
    if ty.is_resource {
        writeln!(out, "                \"resourceType\" => {{")?;
        writeln!(
            out,
            "                    if value.as_str() != Some({:?}) {{",
            ty.name
        )?;
        writeln!(
            out,
            "                        return Err(path.error({C}::DecodeErrorKind::ResourceType));"
        )?;
        writeln!(out, "                    }}")?;
        writeln!(out, "                }}")?;
    }
    for field in fields {
        let slot = &field.name.trim_start_matches("r#");
        let key = &field.fhir_name;
        match shape(model, field) {
            Shape::Choice(choice) => {
                let stem = key.trim_end_matches("[x]");
                if let TypeKind::Choice { variants, .. } = &choice.kind {
                    for variant in variants {
                        let suffix = type_name(&variant.code);
                        writeln!(
                            out,
                            "                \"{stem}{suffix}\" => {{ raw_{slot}.value({suffix:?}, value, path)?; }}"
                        )?;
                        writeln!(
                            out,
                            "                \"_{stem}{suffix}\" => {{ raw_{slot}.element({suffix:?}, value, path)?; }}"
                        )?;
                    }
                }
            }
            Shape::Primitive => {
                writeln!(out, "                {key:?} => raw_{slot} = Some(value),")?;
                writeln!(
                    out,
                    "                \"_{key}\" => raw_{slot}_element = Some(value),"
                )?;
            }
            Shape::Scalar(_) | Shape::Complex => {
                writeln!(out, "                {key:?} => raw_{slot} = Some(value),")?;
            }
        }
    }
    writeln!(
        out,
        "                other => return path.with(other, |path| Err(path.error({C}::DecodeErrorKind::UnknownProperty))),"
    )?;
    writeln!(out, "            }}")?;
    writeln!(out, "        }}")?;
    render_field_builders(model, out, fields)?;
    writeln!(out, "        Ok(Self {{")?;
    for field in fields {
        writeln!(
            out,
            "            {}: field_{},",
            field.name,
            field.name.trim_start_matches("r#")
        )?;
    }
    writeln!(out, "        }})")?;
    writeln!(out, "    }}")
}

fn render_field_builders(model: &VersionModule, out: &mut String, fields: &[Field]) -> fmt::Result {
    // Build each field from its raw slot into a `field_` local, so a FHIR element
    // named `path` or `object` cannot shadow the decoder's own bindings.
    for field in fields {
        match shape(model, field) {
            Shape::Scalar(scalar) => render_scalar_builder(out, field, scalar)?,
            Shape::Primitive => render_primitive_builder(out, field)?,
            Shape::Complex => render_complex_builder(out, field)?,
            Shape::Choice(choice) => render_choice_builder(out, field, choice)?,
        }
    }
    Ok(())
}

/// The decoder lines for a `FHIRPath` system scalar field.
fn render_scalar_builder(out: &mut String, field: &Field, scalar: Scalar) -> fmt::Result {
    let slot = field.name.trim_start_matches("r#");
    let key = &field.fhir_name;
    let name = format!("field_{slot}");
    let decode = scalar_from_value(scalar, None);
    match field.ty.card {
        Cardinality::Optional => writeln!(
            out,
            "        let {name} = raw_{slot}.map(|value| path.with({key:?}, |path| {{ let value = {C}::expect_single(value, path)?; {decode} }})).transpose()?;"
        ),
        Cardinality::One => writeln!(
            out,
            "        let {name} = path.with({key:?}, |path| {{ let value = raw_{slot}.ok_or_else(|| path.error({C}::DecodeErrorKind::MissingProperty))?; let value = {C}::expect_single(value, path)?; {decode} }})?;"
        ),
        Cardinality::Many => {
            writeln!(out, "        let mut {name} = Vec::new();")?;
            writeln!(out, "        if let Some(raw) = raw_{slot} {{")?;
            writeln!(
                out,
                "            for (index, value) in {C}::expect_array(raw, path)?.iter().enumerate() {{"
            )?;
            writeln!(
                out,
                "                {name}.push(path.with_index({key:?}, index, |path| {decode})?);"
            )?;
            writeln!(out, "            }}")?;
            writeln!(out, "        }}")
        }
    }
}

/// The decoder lines for a FHIR primitive field, value plus `_name` sibling.
fn render_primitive_builder(out: &mut String, field: &Field) -> fmt::Result {
    let slot = field.name.trim_start_matches("r#");
    let key = &field.fhir_name;
    let name = format!("field_{slot}");
    match field.ty.card {
        Cardinality::Optional => {
            writeln!(
                out,
                "        let {name} = match (raw_{slot}, raw_{slot}_element) {{"
            )?;
            writeln!(out, "            (None, None) => None,")?;
            writeln!(
                out,
                "            (value, element) => Some(path.with({key:?}, |path| {C}::Primitive::from_json_parts(value, element, path))?),"
            )?;
            writeln!(out, "        }};")
        }
        Cardinality::One => writeln!(
            out,
            "        let {name} = path.with({key:?}, |path| {C}::Primitive::from_json_parts(raw_{slot}, raw_{slot}_element, path))?;"
        ),
        Cardinality::Many => {
            writeln!(out, "        let mut {name} = Vec::new();")?;
            writeln!(
                out,
                "        for (index, (value, element)) in {C}::pair_arrays(raw_{slot}, raw_{slot}_element, path)?.into_iter().enumerate() {{"
            )?;
            writeln!(
                out,
                "            {name}.push(path.with_index({key:?}, index, |path| {C}::Primitive::from_json_parts(value, element, path))?);"
            )?;
            writeln!(out, "        }}")
        }
    }
}

/// The decoder lines for a complex, backbone, or resource field.
fn render_complex_builder(out: &mut String, field: &Field) -> fmt::Result {
    let slot = field.name.trim_start_matches("r#");
    let key = &field.fhir_name;
    let name = format!("field_{slot}");
    let wrap_map = if field.ty.boxed { ".map(Box::new)" } else { "" };
    let decode = format!(
        "{C}::Json::from_json({C}::expect_object({C}::expect_single(value, path)?, path)?, path){wrap_map}"
    );
    match field.ty.card {
        Cardinality::Optional => writeln!(
            out,
            "        let {name} = raw_{slot}.map(|value| path.with({key:?}, |path| {decode})).transpose()?;"
        ),
        Cardinality::One => writeln!(
            out,
            "        let {name} = path.with({key:?}, |path| {{ let value = raw_{slot}.ok_or_else(|| path.error({C}::DecodeErrorKind::MissingProperty))?; {decode} }})?;"
        ),
        Cardinality::Many => {
            writeln!(out, "        let mut {name} = Vec::new();")?;
            writeln!(out, "        if let Some(raw) = raw_{slot} {{")?;
            writeln!(
                out,
                "            for (index, value) in {C}::expect_array(raw, path)?.iter().enumerate() {{"
            )?;
            writeln!(
                out,
                "                {name}.push(path.with_index({key:?}, index, |path| {C}::Json::from_json({C}::expect_object(value, path)?, path){wrap_map})?);"
            )?;
            writeln!(out, "            }}")?;
            writeln!(out, "        }}")
        }
    }
}

/// The decoder lines for a choice field, keyed by the suffix its raw slot found.
fn render_choice_builder(out: &mut String, field: &Field, choice: &TypeDef) -> fmt::Result {
    let slot = field.name.trim_start_matches("r#");
    let name = format!("field_{slot}");
    let stem = field.fhir_name.trim_end_matches("[x]");
    let decode = format!(
        "{}::from_json_parts(suffix, raw_{slot}.value, raw_{slot}.element, path)",
        choice.name
    );
    match field.ty.card {
        Cardinality::Optional => {
            writeln!(out, "        let {name} = match raw_{slot}.suffix {{")?;
            writeln!(out, "            None => None,")?;
            writeln!(
                out,
                "            Some(suffix) => Some(path.with({stem:?}, |path| {decode})?),"
            )?;
            writeln!(out, "        }};")
        }
        _ => writeln!(
            out,
            "        let {name} = path.with({stem:?}, |path| {{ let suffix = raw_{slot}.suffix.ok_or_else(|| path.error({C}::DecodeErrorKind::MissingProperty))?; {decode} }})?;"
        ),
    }
}
