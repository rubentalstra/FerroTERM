//! A model back to a generated `ValueSet`, for reads and expansions.
//!
//! One macro produces a module per served version so they cannot drift:
//! `render::r4b::value_set(&model, true)`, `render::r4b::expansion(&outcome)`.

macro_rules! render_value_set {
    ($module:ident) => {
        /// The `ValueSet` renders of one FHIR version.
        pub mod $module {
            use ferroterm_fhir::$module::coding::Coding;
            use ferroterm_fhir::$module::primitives::Integer;
            use ferroterm_fhir::$module::value_set::{
                ValueSet, ValueSetCompose, ValueSetComposeInclude, ValueSetComposeIncludeConcept,
                ValueSetComposeIncludeConceptDesignation, ValueSetComposeIncludeFilter,
                ValueSetExpansion, ValueSetExpansionContains, ValueSetExpansionParameter,
                ValueSetExpansionParameterValue,
            };

            use crate::compose::{Compose, Include};
            use crate::operations::expand::{ExpansionOutcome, ParameterValue};
            use crate::valueset::model::ValueSetModel;

            /// The `ValueSet` of `model`, with its `compose` when `with_compose`.
            #[must_use]
            pub fn value_set(model: &ValueSetModel, with_compose: bool) -> ValueSet {
                ValueSet {
                    url: Some(model.url.as_str().into()),
                    version: model.version.as_deref().map(Into::into),
                    name: model.name.as_deref().map(Into::into),
                    title: model.title.as_deref().map(Into::into),
                    status: model.status.as_str().into(),
                    experimental: model.experimental.map(Into::into),
                    date: model.date.as_deref().map(Into::into),
                    publisher: model.publisher.as_deref().map(Into::into),
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
                            value: f.value.as_str().into(),
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
                    inactive: compose.inactive.map(Into::into),
                    include: compose.include.iter().map(render).collect(),
                    exclude: compose.exclude.iter().map(render).collect(),
                    ..Default::default()
                }
            }

            /// The expanded value set as a resource of the version.
            ///
            /// The definition (the compose when `includeDefinition` asked for it) and the
            /// `expansion` with its identifier, timestamp, total, offset, echoed
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
                let contains = outcome
                    .contains
                    .iter()
                    .map(|item| ValueSetExpansionContains {
                        system: Some(item.system.as_str().into()),
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
                    })
                    .collect();
                value_set.expansion = Some(ValueSetExpansion {
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
                });
                value_set
            }
        }
    };
}

render_value_set!(r4);
render_value_set!(r4b);
