//! From a generated `ValueSet` of any served FHIR version to the model.
//!
//! The four versions share the compose elements the model needs; one macro
//! produces a module per version so they cannot drift:
//! `convert::r4b::convert(&resource)`.

use super::model::ModelError;
use crate::filter::{Filter, FilterOperator};

fn text<T: AsRef<str>>(value: Option<T>) -> Option<String> {
    value.map(|v| v.as_ref().to_owned())
}

/// One `compose.include.filter`.
fn filter(
    property: Option<&str>,
    op: Option<&str>,
    value: Option<&str>,
) -> Result<Filter, ModelError> {
    let property = property.unwrap_or_default().to_owned();
    let op_code = op.unwrap_or_default();
    let op = FilterOperator::parse(op_code).ok_or_else(|| ModelError::FilterOperator {
        property: property.clone(),
        op: op_code.to_owned(),
    })?;
    Ok(Filter {
        property,
        op,
        value: value.unwrap_or_default().to_owned(),
    })
}

macro_rules! convert_value_set {
    (@value required, $f:ident) => {
        $f.value.value.as_deref()
    };
    (@value optional, $f:ident) => {
        $f.value.as_ref().and_then(|v| v.value.as_deref())
    };
    ($module:ident, $value:tt) => {
        /// The conversion for this FHIR version's `ValueSet`.
        pub mod $module {
            use ferroterm_fhir::$module::value_set::{ValueSet, ValueSetComposeInclude};

            use super::super::model::{ModelError, ValueSetModel};
            use super::text;
            use crate::compose::{Compose, ConceptRef, Include, SystemRef};

            fn include(source: &ValueSetComposeInclude) -> Result<Include, ModelError> {
                let mut filters = Vec::with_capacity(source.filter.len());
                for f in &source.filter {
                    filters.push(super::filter(
                        f.property.value.as_deref(),
                        f.op.value.as_deref(),
                        convert_value_set!(@value $value, f),
                    )?);
                }
                Ok(Include {
                    system: text(source.system.as_ref().and_then(|s| s.value.as_deref())).map(
                        |url| SystemRef {
                            url,
                            version: text(source.version.as_ref().and_then(|v| v.value.as_deref())),
                        },
                    ),
                    concepts: source
                        .concept
                        .iter()
                        .map(|c| ConceptRef {
                            code: text(c.code.value.as_deref()).unwrap_or_default(),
                            display: text(c.display.as_ref().and_then(|d| d.value.as_deref())),
                        })
                        .collect(),
                    filters,
                    value_sets: source
                        .value_set
                        .iter()
                        .filter_map(|v| text(v.value.as_deref()))
                        .collect(),
                })
            }

            /// Reduces a `ValueSet` of this FHIR version to the model.
            ///
            /// # Errors
            ///
            /// Returns [`ModelError`] for a resource without a `url` or a filter
            /// with an unknown operator.
            pub fn convert(resource: &ValueSet) -> Result<ValueSetModel, ModelError> {
                let url = text(resource.url.as_ref().and_then(|u| u.value.as_deref()))
                    .ok_or(ModelError::NoUrl)?;
                let mut compose = Compose::default();
                if let Some(source) = &resource.compose {
                    compose.inactive = source.inactive.as_ref().and_then(|b| b.value);
                    for i in &source.include {
                        compose.include.push(include(i)?);
                    }
                    for e in &source.exclude {
                        compose.exclude.push(include(e)?);
                    }
                }
                let string = |value: &Option<ferroterm_fhir::$module::primitives::String>| {
                    text(value.as_ref().and_then(|v| v.value.as_deref()))
                };
                let flag = |value: &Option<ferroterm_fhir::$module::primitives::Boolean>| {
                    value.as_ref().and_then(|b| b.value)
                };
                Ok(ValueSetModel {
                    url,
                    version: string(&resource.version),
                    name: string(&resource.name),
                    title: string(&resource.title),
                    status: text(resource.status.value.as_deref()).unwrap_or_default(),
                    experimental: flag(&resource.experimental),
                    date: text(resource.date.as_ref().and_then(|d| d.value.as_deref())),
                    publisher: string(&resource.publisher),
                    description: text(
                        resource
                            .description
                            .as_ref()
                            .and_then(|d| d.value.as_deref()),
                    ),
                    immutable: flag(&resource.immutable),
                    compose,
                })
            }
        }
    };
}

convert_value_set!(r4, required);
convert_value_set!(r4b, required);
convert_value_set!(r5, required);
convert_value_set!(r6, optional);
