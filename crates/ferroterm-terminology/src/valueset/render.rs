//! A model back to a generated `ValueSet`, for reads and expansions.

use ferroterm_fhir::r4b::value_set::{
    ValueSet, ValueSetCompose, ValueSetComposeInclude, ValueSetComposeIncludeConcept,
    ValueSetComposeIncludeFilter,
};

use super::model::ValueSetModel;
use crate::compose::{Compose, Include};

/// The R4B `ValueSet` of `model`, with its `compose` when `with_compose`.
#[must_use]
pub fn to_r4b(model: &ValueSetModel, with_compose: bool) -> ValueSet {
    ValueSet {
        url: Some(model.url.as_str().into()),
        version: model.version.as_deref().map(Into::into),
        name: model.name.as_deref().map(Into::into),
        title: model.title.as_deref().map(Into::into),
        status: model.status.as_str().into(),
        experimental: model.experimental.map(Into::into),
        date: model.date.as_deref().map(Into::into),
        publisher: model.publisher.as_deref().map(Into::into),
        description: model.description.as_deref().map(Into::into),
        immutable: model.immutable.map(Into::into),
        compose: with_compose.then(|| compose_r4b(&model.compose)),
        ..Default::default()
    }
}

/// `compose` as the R4B element.
#[must_use]
pub fn compose_r4b(compose: &Compose) -> ValueSetCompose {
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
