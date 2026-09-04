//! A model back to a generated `ValueSet`, for reads and expansions.
//!
//! One macro produces a module per served version so they cannot drift:
//! `render::r4b::value_set(&model, true)`, `render::r4b::expansion(&outcome)`.
//! R5 declares `expansion.property` and `contains.property`
//! (<https://hl7.org/fhir/R5/valueset.html>); the `element` arms fill them,
//! the `none` arms are for the versions without them.

macro_rules! render_value_set {
    // R6 makes `compose.include.filter.value` optional
    // (<https://hl7.org/fhir/6.0.0-ballot5/valueset-definitions.html#ValueSet.compose.include.filter.value>).
    (@filter_value required, $f:ident) => {
        $f.value.as_str().into()
    };
    (@filter_value optional, $f:ident) => {
        Some($f.value.as_str().into())
    };
    (@helpers none, $module:ident) => {};
    (@helpers element, $module:ident) => {
        render_value_set!(
            @value_fn property_value,
            fhir_types::$module::value_set::ValueSetExpansionContainsPropertyValue
        );
        render_value_set!(
            @value_fn sub_property_value,
            fhir_types::$module::value_set::ValueSetExpansionContainsPropertySubPropertyValue
        );
    };
    (@value_fn $name:ident, $value:ty) => {
        /// A property value as the wire element `$name` fills.
        fn $name(value: &crate::provider::PropertyValue) -> $value {
            use crate::provider::PropertyValue;
            match value {
                PropertyValue::Code(c) => <$value>::Code(c.as_str().into()),
                PropertyValue::Uri(u) | PropertyValue::String(u) => {
                    <$value>::String(u.as_str().into())
                }
                PropertyValue::Coding {
                    system,
                    code,
                    display,
                } => <$value>::Coding(Coding {
                    system: Some(system.as_str().into()),
                    code: Some(code.as_str().into()),
                    display: display.as_deref().map(Into::into),
                    ..Default::default()
                }),
                PropertyValue::Integer(i) => match i32::try_from(*i) {
                    Ok(i) => <$value>::Integer(i.into()),
                    Err(_) => <$value>::String(i.to_string().as_str().into()),
                },
                PropertyValue::Boolean(b) => <$value>::Boolean((*b).into()),
                PropertyValue::DateTime(d) => <$value>::DateTime(d.as_str().into()),
                PropertyValue::Decimal(d) => <$value>::Decimal(d.as_str().into()),
            }
        }
    };
    (@properties none, $module:ident, $entry:expr, $item:ident) => {
        $entry
    };
    (@properties element, $module:ident, $entry:expr, $item:ident) => {{
        let mut entry = $entry;
        entry.property = $item
            .properties
            .iter()
            .map(|p| fhir_types::$module::value_set::ValueSetExpansionContainsProperty {
                id: None,
                extension: Vec::new(),
                modifier_extension: Vec::new(),
                code: p.code.as_str().into(),
                value: property_value(&p.value),
                sub_property: p
                    .subproperties
                    .iter()
                    .map(|s| {
                        fhir_types::$module::value_set::ValueSetExpansionContainsPropertySubProperty {
                            id: None,
                            extension: Vec::new(),
                            modifier_extension: Vec::new(),
                            code: s.code.as_str().into(),
                            value: sub_property_value(&s.value),
                        }
                    })
                    .collect(),
            })
            .collect();
        entry
    }};
    (@expansion_properties none, $module:ident, $expansion:expr, $outcome:ident) => {
        $expansion
    };
    (@expansion_properties element, $module:ident, $expansion:expr, $outcome:ident) => {{
        let mut expansion = $expansion;
        expansion.property = $outcome
            .properties
            .iter()
            .map(|p| fhir_types::$module::value_set::ValueSetExpansionProperty {
                code: p.code.as_str().into(),
                uri: p.uri.as_deref().map(Into::into),
                ..Default::default()
            })
            .collect();
        expansion
    }};
    ($module:ident, $properties:ident, $filter_value:ident) => {
        /// The `ValueSet` renders of one FHIR version.
        pub mod $module {
            use fhir_types::$module::coding::Coding;
            use fhir_types::$module::primitives::Integer;
            use fhir_types::$module::value_set::{
                ValueSet, ValueSetCompose, ValueSetComposeInclude, ValueSetComposeIncludeConcept,
                ValueSetComposeIncludeConceptDesignation, ValueSetComposeIncludeFilter,
                ValueSetExpansion, ValueSetExpansionContains, ValueSetExpansionParameter,
                ValueSetExpansionParameterValue,
            };

            use crate::compose::{Compose, Include};
            use crate::operations::expand::{ExpansionOutcome, ParameterValue};
            use crate::valueset::model::ValueSetModel;

            render_value_set!(@helpers $properties, $module);

            /// The `ValueSet` of `model`, with its `compose` when `with_compose`.
            #[must_use]
            pub fn value_set(model: &ValueSetModel, with_compose: bool) -> ValueSet {
                ValueSet {
                    url: Some(model.url.as_str().into()),
                    version: model.version.as_deref().map(Into::into),
                    language: model.language.as_deref().map(Into::into),
                    name: model.name.as_deref().map(Into::into),
                    title: model.title.as_deref().map(Into::into),
                    status: model.status.as_str().into(),
                    experimental: model.experimental.map(Into::into),
                    date: model.date.as_deref().map(Into::into),
                    // NOTE: `publisher` travels with the definition, as `compose` does
                    // (the ecosystem's expansions omit it unless `includeDefinition`).
                    publisher: with_compose
                        .then(|| model.publisher.as_deref().map(Into::into))
                        .flatten(),
                    description: None,
                    immutable: model.immutable.map(Into::into),
                    compose: with_compose.then(|| compose(&model.compose)),
                    ..Default::default()
                }
            }

            /// `compose` as the version's element.
            #[must_use]
            pub fn compose(compose: &Compose) -> ValueSetCompose {
                let render = |include: &Include| ValueSetComposeInclude {
                    system: include.system.as_ref().map(|s| s.url.as_str().into()),
                    version: include
                        .system
                        .as_ref()
                        .and_then(|s| s.version.as_deref())
                        .map(Into::into),
                    concept: include
                        .concepts
                        .iter()
                        .map(|c| ValueSetComposeIncludeConcept {
                            code: c.code.as_str().into(),
                            display: c.display.as_deref().map(Into::into),
                            ..Default::default()
                        })
                        .collect(),
                    filter: include
                        .filters
                        .iter()
                        .map(|f| ValueSetComposeIncludeFilter {
                            property: f.property.as_str().into(),
                            op: f.op.code().into(),
                            value: render_value_set!(@filter_value $filter_value, f),
                            ..Default::default()
                        })
                        .collect(),
                    value_set: include
                        .value_sets
                        .iter()
                        .map(|v| v.as_str().into())
                        .collect(),
                    ..Default::default()
                };
                ValueSetCompose {
                    include: compose.include.iter().map(render).collect(),
                    exclude: compose.exclude.iter().map(render).collect(),
                    ..Default::default()
                }
            }

            /// The expanded value set as a resource of the version.
            ///
            /// The definition (the compose when `includeDefinition` asked for it) and
            /// the `expansion` with its identifier, timestamp, total, offset, echoed
            /// parameters, and page.
            #[must_use]
            pub fn expansion(outcome: &ExpansionOutcome) -> ValueSet {
                let mut value_set = value_set(&outcome.model, outcome.include_definition);
                let parameter = outcome
                    .parameters
                    .iter()
                    .map(|p| ValueSetExpansionParameter {
                        name: p.name.as_str().into(),
                        value: Some(match &p.value {
                            ParameterValue::String(s) => {
                                ValueSetExpansionParameterValue::String(s.as_str().into())
                            }
                            ParameterValue::Boolean(b) => {
                                ValueSetExpansionParameterValue::Boolean((*b).into())
                            }
                            ParameterValue::Integer(i) => ValueSetExpansionParameterValue::Integer(
                                i32::try_from(*i).unwrap_or(i32::MAX).into(),
                            ),
                            ParameterValue::Code(c) => {
                                ValueSetExpansionParameterValue::Code(c.as_str().into())
                            }
                            ParameterValue::Uri(u) => {
                                ValueSetExpansionParameterValue::Uri(u.as_str().into())
                            }
                        }),
                        ..Default::default()
                    })
                    .collect();
                // NOTE: `contains.version` is populated when the expansion draws one
                // system from several versions, so the codes stay distinguishable
                // (<https://hl7.org/fhir/R4B/valueset-definitions.html#ValueSet.expansion.contains.version>).
                let mut versions: std::collections::BTreeMap<&str, std::collections::BTreeSet<&str>> =
                    std::collections::BTreeMap::new();
                for item in &outcome.contains {
                    versions
                        .entry(item.system.as_str())
                        .or_default()
                        .insert(item.version.as_str());
                }
                let contains = outcome
                    .contains
                    .iter()
                    .map(|item| {
                        let entry = ValueSetExpansionContains {
                            system: Some(item.system.as_str().into()),
                            version: versions
                                .get(item.system.as_str())
                                .filter(|v| v.len() > 1)
                                .map(|_| item.version.as_str().into()),
                            r#abstract: item.abstract_concept.then_some(true.into()),
                            inactive: item.inactive.then_some(true.into()),
                            code: Some(item.code.as_str().into()),
                            display: item.display.as_deref().map(Into::into),
                            designation: item
                                .designations
                                .iter()
                                .map(|d| ValueSetComposeIncludeConceptDesignation {
                                    language: d.language.as_deref().map(Into::into),
                                    r#use: d.use_.as_ref().map(|u| Coding {
                                        system: Some(u.system.as_str().into()),
                                        code: Some(u.code.as_str().into()),
                                        display: u.display.as_deref().map(Into::into),
                                        ..Default::default()
                                    }),
                                    value: d.value.as_str().into(),
                                    ..Default::default()
                                })
                                .collect(),
                            ..Default::default()
                        };
                        render_value_set!(@properties $properties, $module, entry, item)
                    })
                    .collect();
                let expansion = ValueSetExpansion {
                    identifier: Some(outcome.identifier.as_str().into()),
                    timestamp: outcome.timestamp.as_str().into(),
                    total: Some(Integer::from(
                        i32::try_from(outcome.total).unwrap_or(i32::MAX),
                    )),
                    offset: outcome
                        .offset
                        .map(|o| Integer::from(i32::try_from(o).unwrap_or(i32::MAX))),
                    parameter,
                    contains,
                    ..Default::default()
                };
                value_set.expansion = Some(render_value_set!(
                    @expansion_properties $properties, $module, expansion, outcome
                ));
                value_set
            }
        }
    };
}

render_value_set!(r4, none, required);
render_value_set!(r4b, none, required);
render_value_set!(r5, element, required);
render_value_set!(r6, element, optional);
