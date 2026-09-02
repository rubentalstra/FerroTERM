//! The FHIR JSON representation shared by every version module.
//!
//! Resources carry `resourceType`; a primitive element is a JSON value plus an
//! optional sibling named with a leading underscore that holds its `id` and
//! `extension`; repeating primitives are two arrays aligned with `null`;
//! integers and decimals are JSON numbers with their lexical form preserved;
//! objects and arrays are never empty; unknown properties are refused
//! (<https://hl7.org/fhir/R4B/json.html>).

use std::fmt;

use serde_json::{Map, Number, Value};

/// A JSON object, as `serde_json` spells it.
pub type Object = Map<String, Value>;

/// Why a JSON document was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeErrorKind {
    /// A property the type does not define.
    UnknownProperty,
    /// A required property is absent.
    MissingProperty,
    /// The value has the wrong JSON type.
    WrongType {
        /// What the type expected.
        expected: &'static str,
    },
    /// A property appears with a shape its cardinality forbids (an array
    /// for a single element or the reverse).
    WrongCardinality,
    /// An object or array is empty.
    Empty,
    /// A number is out of range or not a whole number where one is required.
    BadNumber,
    /// The lexical form of a primitive is invalid.
    BadValue,
    /// Two forms of one choice element are both present.
    DuplicateChoice,
    /// `resourceType` is absent, not a string, or names another type.
    ResourceType,
}

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProperty => f.write_str("unknown property"),
            Self::MissingProperty => f.write_str("required property is missing"),
            Self::WrongType { expected } => write!(f, "expected {expected}"),
            Self::WrongCardinality => f.write_str("cardinality mismatch"),
            Self::Empty => f.write_str("empty object or array"),
            Self::BadNumber => f.write_str("number out of range"),
            Self::BadValue => f.write_str("invalid value"),
            Self::DuplicateChoice => f.write_str("more than one form of a choice element"),
            Self::ResourceType => f.write_str("resourceType is missing or does not match"),
        }
    }
}

/// A refused JSON document, located by its element path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    /// The element path, for example `ValueSet.compose.include[2].system`.
    pub path: String,
    /// Why it was refused.
    pub kind: DecodeErrorKind,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.kind)
    }
}

impl std::error::Error for DecodeError {}

/// The element path being decoded, for error reports.
#[derive(Debug, Default, Clone)]
pub struct Path {
    segments: Vec<String>,
}

impl Path {
    /// A path starting at `root`.
    #[must_use]
    pub fn root(root: &str) -> Self {
        Self {
            segments: vec![root.to_owned()],
        }
    }

    /// Runs `f` under `name`.
    ///
    /// # Errors
    ///
    /// Returns whatever `f` returns.
    pub fn with<T>(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Self) -> Result<T, DecodeError>,
    ) -> Result<T, DecodeError> {
        self.segments.push(name.to_owned());
        let out = f(self);
        self.segments.pop();
        out
    }

    /// Runs `f` under `name[index]`.
    ///
    /// # Errors
    ///
    /// Returns whatever `f` returns.
    pub fn with_index<T>(
        &mut self,
        name: &str,
        index: usize,
        f: impl FnOnce(&mut Self) -> Result<T, DecodeError>,
    ) -> Result<T, DecodeError> {
        self.segments.push(format!("{name}[{index}]"));
        let out = f(self);
        self.segments.pop();
        out
    }

    /// An error at the current path.
    #[must_use]
    pub fn error(&self, kind: DecodeErrorKind) -> DecodeError {
        DecodeError {
            path: self.segments.join("."),
            kind,
        }
    }
}

/// A type with a FHIR JSON representation as an object.
pub trait Json: Sized {
    /// The JSON object.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when a held value has no JSON form.
    fn to_json(&self) -> Result<Object, EncodeError>;

    /// Decodes `object`, refusing unknown properties.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] for any deviation from the type's definition.
    fn from_json(object: &Object, path: &mut Path) -> Result<Self, DecodeError>;
}

/// A FHIR primitive: a JSON value plus an optional element object.
pub trait Primitive: Sized {
    /// The JSON value, `None` when only `id` or `extension` are present.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when the held value has no JSON form.
    fn value_json(&self) -> Result<Option<Value>, EncodeError>;

    /// The `_name` object, `None` when the primitive has no `id` or `extension`.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when an extension value has no JSON form.
    fn element_json(&self) -> Result<Option<Value>, EncodeError>;

    /// Decodes the value and its `_name` sibling.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when both parts are absent or either is malformed.
    fn from_json_parts(
        value: Option<&Value>,
        element: Option<&Value>,
        path: &mut Path,
    ) -> Result<Self, DecodeError>;
}

/// Reads a non-empty JSON object.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not an object or is empty.
pub fn expect_object<'a>(value: &'a Value, path: &Path) -> Result<&'a Object, DecodeError> {
    match value {
        Value::Object(object) if object.is_empty() => Err(path.error(DecodeErrorKind::Empty)),
        Value::Object(object) => Ok(object),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "an object",
        })),
    }
}

/// Reads a non-empty JSON array.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not an array or is empty.
pub fn expect_array<'a>(value: &'a Value, path: &Path) -> Result<&'a [Value], DecodeError> {
    match value {
        Value::Array(items) if items.is_empty() => Err(path.error(DecodeErrorKind::Empty)),
        Value::Array(items) => Ok(items),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "an array",
        })),
    }
}

/// Refuses an array where a single value is expected.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is an array.
pub fn expect_single<'a>(value: &'a Value, path: &Path) -> Result<&'a Value, DecodeError> {
    match value {
        Value::Array(_) => Err(path.error(DecodeErrorKind::WrongCardinality)),
        other => Ok(other),
    }
}

/// Reads a JSON string.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a string.
pub fn expect_string(value: &Value, path: &Path) -> Result<String, DecodeError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a string",
        })),
    }
}

/// Reads a JSON boolean.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a boolean.
pub fn expect_bool(value: &Value, path: &Path) -> Result<bool, DecodeError> {
    match value {
        Value::Bool(b) => Ok(*b),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a boolean",
        })),
    }
}

/// Reads a JSON number as an `i32`.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a whole number in range.
pub fn expect_i32(value: &Value, path: &Path) -> Result<i32, DecodeError> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| path.error(DecodeErrorKind::BadNumber)),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a number",
        })),
    }
}

/// Reads a JSON number as a `u32`, at least `min`.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a whole number in range.
pub fn expect_u32(value: &Value, min: u32, path: &Path) -> Result<u32, DecodeError> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .and_then(|u| u32::try_from(u).ok())
            .filter(|u| *u >= min)
            .ok_or_else(|| path.error(DecodeErrorKind::BadNumber)),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a number",
        })),
    }
}

/// Reads a 64-bit integer carried as a JSON string (the R5 `integer64` form).
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a string holding an integer.
pub fn expect_i64_string(value: &Value, path: &Path) -> Result<i64, DecodeError> {
    match value {
        Value::String(s) => s.parse().map_err(|_| path.error(DecodeErrorKind::BadNumber)),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a string holding an integer",
        })),
    }
}

/// Reads a JSON number keeping its lexical form, for the FHIR decimal.
///
/// # Errors
///
/// Returns [`DecodeError`] when `value` is not a number.
pub fn expect_decimal(value: &Value, path: &Path) -> Result<String, DecodeError> {
    match value {
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(path.error(DecodeErrorKind::WrongType {
            expected: "a number",
        })),
    }
}

/// A decimal's lexical form as a JSON number.
///
/// # Errors
///
/// Returns [`DecodeError`] when `text` is not a JSON number.
pub fn decimal_value(text: &str, path: &Path) -> Result<Value, DecodeError> {
    text.parse::<Number>()
        .map(Value::Number)
        .map_err(|_| path.error(DecodeErrorKind::BadValue))
}

/// One position of a repeating primitive: the value and its `_name` object.
pub type ValuePair<'a> = (Option<&'a Value>, Option<&'a Value>);

/// Pairs a repeating primitive's value array with its `_name` array.
///
/// # Errors
///
/// Returns [`DecodeError`] when either is not an array, is empty, or the
/// arrays differ in length.
pub fn pair_arrays<'a>(
    values: Option<&'a Value>,
    elements: Option<&'a Value>,
    path: &Path,
) -> Result<Vec<ValuePair<'a>>, DecodeError> {
    let values = values.map(|v| expect_array(v, path)).transpose()?;
    let elements = elements.map(|v| expect_array(v, path)).transpose()?;
    let len = match (values, elements) {
        (Some(v), Some(e)) if v.len() != e.len() => {
            return Err(path.error(DecodeErrorKind::WrongCardinality));
        }
        (Some(v), _) => v.len(),
        (None, Some(e)) => e.len(),
        (None, None) => 0,
    };
    Ok((0..len)
        .map(|i| {
            let value = values.and_then(|v| v.get(i)).filter(|v| !v.is_null());
            let element = elements.and_then(|e| e.get(i)).filter(|e| !e.is_null());
            (value, element)
        })
        .collect())
}

/// Serializes a list of primitives as the value array and, when any carries
/// an element object, the aligned `_name` array.
///
/// # Errors
///
/// Returns [`EncodeError`] when a held value has no JSON form.
pub fn primitive_arrays<P: Primitive>(items: &[P]) -> Result<(Value, Option<Value>), EncodeError> {
    let mut values = Vec::with_capacity(items.len());
    let mut elements = Vec::with_capacity(items.len());
    for item in items {
        values.push(item.value_json()?.unwrap_or(Value::Null));
        elements.push(item.element_json()?.unwrap_or(Value::Null));
    }
    let any_element = elements.iter().any(|e| !e.is_null());
    Ok((
        Value::Array(values),
        any_element.then_some(Value::Array(elements)),
    ))
}

/// Why a value could not be written as JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// A decimal's lexical form is not a JSON number.
    BadDecimal {
        /// The text that was held.
        text: String,
    },
    /// An unknown resource's body is not a JSON object.
    UnknownResourceBody,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadDecimal { text } => write!(f, "decimal {text:?} is not a JSON number"),
            Self::UnknownResourceBody => f.write_str("an unknown resource's body is not an object"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// The parts of one choice element met while reading an object.
#[derive(Debug, Default)]
pub struct ChoiceSlot<'a> {
    /// The type suffix of the key seen, for example `String` for `valueString`.
    pub suffix: Option<&'static str>,
    /// The value part.
    pub value: Option<&'a Value>,
    /// The `_name` part.
    pub element: Option<&'a Value>,
}

impl<'a> ChoiceSlot<'a> {
    fn claim(&mut self, suffix: &'static str, path: &Path) -> Result<(), DecodeError> {
        match self.suffix {
            Some(seen) if seen != suffix => Err(path.error(DecodeErrorKind::DuplicateChoice)),
            _ => {
                self.suffix = Some(suffix);
                Ok(())
            }
        }
    }

    /// Records the value part of the form `suffix`.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when another form was already seen.
    pub fn value(&mut self, suffix: &'static str, value: &'a Value, path: &Path) -> Result<(), DecodeError> {
        self.claim(suffix, path)?;
        self.value = Some(value);
        Ok(())
    }

    /// Records the `_name` part of the form `suffix`.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when another form was already seen.
    pub fn element(&mut self, suffix: &'static str, element: &'a Value, path: &Path) -> Result<(), DecodeError> {
        self.claim(suffix, path)?;
        self.element = Some(element);
        Ok(())
    }
}

/// The `resourceType` of an object, for dispatch.
///
/// # Errors
///
/// Returns [`DecodeError`] when the property is absent or not a string.
pub fn resource_type<'a>(object: &'a Object, path: &Path) -> Result<&'a str, DecodeError> {
    match object.get("resourceType") {
        Some(Value::String(name)) => Ok(name),
        _ => Err(path.error(DecodeErrorKind::ResourceType)),
    }
}
