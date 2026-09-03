//! `ValueSet/$expand` on R4B
//! (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>).
//!
//! The value set is inline, stored, or implicit; its compose is evaluated by
//! the compose layer and the expansion is flat (`excludeNested` is always
//! honoured). Every effective parameter is echoed in `expansion.parameter`,
//! with one `used-codesystem` per code system version the expansion drew on.
//! `context`, `contextDirection`, and `date` are refused as not supported.

use ferroterm_fhir::r4b::coding::Coding;
use ferroterm_fhir::r4b::operations::value_set_expand::{
    ValueSetExpandRequest, ValueSetExpandResponse,
};
use ferroterm_fhir::r4b::primitives::{Canonical, Integer};
use ferroterm_fhir::r4b::value_set::{
    ValueSet, ValueSetComposeIncludeConceptDesignation, ValueSetExpansion,
    ValueSetExpansionContains, ValueSetExpansionParameter, ValueSetExpansionParameterValue,
};

use super::{OperationError, Sources, code_text, string_text, uri_text};
use crate::compose::{Compose, Expansion, Include, Item, Options};
use crate::provider::Designation;
use crate::valueset::model::ValueSetModel;
use crate::valueset::store::Resolver;
use crate::valueset::{convert, render};
use crate::versioned::Versioned;

/// The largest expansion returned without `count`.
pub const EXPANSION_LIMIT: u64 = 1000;

/// Runs `$expand`.
///
/// # Errors
///
/// Returns [`OperationError`] when the value set is unknown or invalid, a
/// parameter is out of range or unsupported, or a provider fails.
pub fn expand(
    sources: &Sources<'_>,
    request: &ValueSetExpandRequest,
) -> Result<ValueSetExpandResponse, OperationError> {
    refuse_unsupported(request)?;
    let model = sources.value_set(
        request.value_set.as_ref().map(convert::r4b::convert),
        uri_text(request.url.as_ref()),
        string_text(request.value_set_version.as_ref()),
    )?;
    let compose = pinned_compose(&model.compose, request)?;
    let options = options(request)?;
    let resolver = Resolver::new(sources.registry, sources.value_sets);
    let expansion = resolver.expand_compose(&model.canonical(), &compose, &options)?;
    // NOTE: `too-costly` is the issue type for an expansion the server declines to
    // return whole (<https://hl7.org/fhir/R4B/valueset-issue-type.html>); the
    // size at which it declines is our own design.
    if options.count.is_none() && expansion.total > EXPANSION_LIMIT {
        return Err(OperationError::TooCostly(format!(
            "the expansion of `{}` has {} concepts; page it with `count` (and `offset`) to fetch it",
            model.canonical(),
            expansion.total
        )));
    }
    let contains = contains(sources, &expansion, request, options.language.as_deref())?;
    let total = i32::try_from(expansion.total).map_err(|_| {
        OperationError::NotSupported(String::from(
            "the expansion is larger than an R4B integer can count",
        ))
    })?;
    let offset = i32::try_from(expansion.offset).map_err(|_| {
        OperationError::Invalid(String::from("`offset` is larger than an R4B integer"))
    })?;
    let include_definition = request
        .include_definition
        .as_ref()
        .and_then(|b| b.value)
        .unwrap_or(false);
    let mut value_set = render::to_r4b(&model, include_definition);
    value_set.expansion = Some(ValueSetExpansion {
        identifier: Some(format!("urn:uuid:{}", uuid::Uuid::new_v4()).as_str().into()),
        timestamp: jiff::Timestamp::now().to_string().as_str().into(),
        total: Some(Integer::from(total)),
        offset: (request.offset.is_some() || request.count.is_some())
            .then_some(Integer::from(offset)),
        parameter: parameters(request, &expansion),
        contains,
        ..Default::default()
    });
    Ok(ValueSetExpandResponse {
        r#return: value_set,
    })
}

/// Refuses the R4B parameters this server does not evaluate.
fn refuse_unsupported(request: &ValueSetExpandRequest) -> Result<(), OperationError> {
    if request.context.is_some() || request.context_direction.is_some() {
        return Err(OperationError::NotSupported(String::from(
            "`context` and `contextDirection` are not supported; name the value set with `url` or `valueSet`",
        )));
    }
    if request.date.is_some() {
        return Err(OperationError::NotSupported(String::from(
            "`date` is not supported: expansions are generated from the versions served now",
        )));
    }
    Ok(())
}

/// The request-time options of the expansion.
fn options(request: &ValueSetExpandRequest) -> Result<Options, OperationError> {
    let non_negative = |value: Option<&Integer>,
                        name: &str|
     -> Result<Option<usize>, OperationError> {
        value
            .and_then(|v| v.value)
            .map(|v| {
                usize::try_from(v)
                    .map_err(|_| OperationError::Invalid(format!("`{name}` must not be negative")))
            })
            .transpose()
    };
    Ok(Options {
        active_only: request
            .active_only
            .as_ref()
            .and_then(|b| b.value)
            .unwrap_or(false),
        text: string_text(request.filter.as_ref()).map(str::to_owned),
        language: code_text(request.display_language.as_ref()).map(str::to_owned),
        offset: non_negative(request.offset.as_ref(), "offset")?.unwrap_or(0),
        count: non_negative(request.count.as_ref(), "count")?,
    })
}

/// `compose` with the version pins and system exclusions applied
/// (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>, `system-version`,
/// `check-system-version`, `force-system-version`, `exclude-system`).
fn pinned_compose(
    compose: &Compose,
    request: &ValueSetExpandRequest,
) -> Result<Compose, OperationError> {
    let canonicals = |list: &[Canonical]| -> Vec<(String, Option<String>)> {
        list.iter()
            .filter_map(|c| c.value.as_deref())
            .map(|c| match c.split_once('|') {
                Some((url, version)) => (url.to_owned(), Some(version.to_owned())),
                None => (c.to_owned(), None),
            })
            .collect()
    };
    let excluded = canonicals(&request.exclude_system);
    let defaults = canonicals(&request.system_version);
    let checks = canonicals(&request.check_system_version);
    let forced = canonicals(&request.force_system_version);
    let pin = |include: &Include| -> Result<Option<Include>, OperationError> {
        let Some(system) = &include.system else {
            return Ok(Some(include.clone()));
        };
        if excluded.iter().any(|(url, version)| {
            *url == system.url
                && version
                    .as_ref()
                    .is_none_or(|v| Some(v) == system.version.as_ref())
        }) {
            return Ok(None);
        }
        let mut pinned = include.clone();
        let Some(target) = pinned.system.as_mut() else {
            return Ok(Some(pinned));
        };
        for (url, version) in &checks {
            if *url == system.url
                && let Some(named) = &system.version
                && version.as_ref().is_some_and(|v| v != named)
            {
                return Err(OperationError::Invalid(format!(
                    "`check-system-version` names `{url}|{}` but the value set uses version `{named}`",
                    version.as_deref().unwrap_or_default()
                )));
            }
        }
        if target.version.is_none()
            && let Some((_, version)) = defaults
                .iter()
                .chain(&checks)
                .find(|(url, _)| *url == system.url)
        {
            target.version.clone_from(version);
        }
        if let Some((_, version)) = forced.iter().find(|(url, _)| *url == system.url) {
            target.version.clone_from(version);
        }
        Ok(Some(pinned))
    };
    let mut pinned = Compose {
        include: Vec::with_capacity(compose.include.len()),
        exclude: Vec::with_capacity(compose.exclude.len()),
        inactive: compose.inactive,
    };
    for include in &compose.include {
        pinned.include.extend(pin(include)?);
    }
    for exclude in &compose.exclude {
        pinned.exclude.extend(pin(exclude)?);
    }
    Ok(pinned)
}

/// The `expansion.parameter` echo: every effective request parameter, then
/// one `used-codesystem` per code system version used.
fn parameters(
    request: &ValueSetExpandRequest,
    expansion: &Expansion,
) -> Vec<ValueSetExpansionParameter> {
    let mut out = Vec::new();
    let mut push = |name: &str, value: ValueSetExpansionParameterValue| {
        out.push(ValueSetExpansionParameter {
            name: name.into(),
            value: Some(value),
            ..Default::default()
        });
    };
    if let Some(filter) = &request.filter {
        push(
            "filter",
            ValueSetExpansionParameterValue::String(filter.clone()),
        );
    }
    for (name, flag) in [
        ("activeOnly", &request.active_only),
        ("excludeNested", &request.exclude_nested),
        ("includeDesignations", &request.include_designations),
        ("includeDefinition", &request.include_definition),
        ("excludeNotForUI", &request.exclude_not_for_u_i),
        ("excludePostCoordinated", &request.exclude_post_coordinated),
    ] {
        if let Some(flag) = flag {
            push(name, ValueSetExpansionParameterValue::Boolean(flag.clone()));
        }
    }
    for (name, number) in [("offset", &request.offset), ("count", &request.count)] {
        if let Some(number) = number {
            push(
                name,
                ValueSetExpansionParameterValue::Integer(number.clone()),
            );
        }
    }
    if let Some(language) = &request.display_language {
        push(
            "displayLanguage",
            ValueSetExpansionParameterValue::Code(language.clone()),
        );
    }
    for designation in &request.designation {
        push(
            "designation",
            ValueSetExpansionParameterValue::String(designation.clone()),
        );
    }
    for (name, list) in [
        ("exclude-system", &request.exclude_system),
        ("system-version", &request.system_version),
        ("check-system-version", &request.check_system_version),
        ("force-system-version", &request.force_system_version),
    ] {
        for canonical in list.iter().filter_map(|c| c.value.as_deref()) {
            push(name, ValueSetExpansionParameterValue::Uri(canonical.into()));
        }
    }
    for used in &expansion.versions {
        push(
            "used-codesystem",
            ValueSetExpansionParameterValue::Uri(
                canonical(&used.url, &used.version).as_str().into(),
            ),
        );
    }
    out
}

/// The flat `expansion.contains` list, with designations when asked.
fn contains(
    sources: &Sources<'_>,
    expansion: &Expansion,
    request: &ValueSetExpandRequest,
    language: Option<&str>,
) -> Result<Vec<ValueSetExpansionContains>, OperationError> {
    let include_designations = request
        .include_designations
        .as_ref()
        .and_then(|b| b.value)
        .unwrap_or(false);
    let wanted: Vec<&str> = request
        .designation
        .iter()
        .filter_map(|d| d.value.as_deref())
        .collect();
    let mut out = Vec::with_capacity(expansion.items.len());
    for item in &expansion.items {
        let designation = if include_designations {
            designations_of(sources, item, language, &wanted)?
        } else {
            Vec::new()
        };
        out.push(ValueSetExpansionContains {
            system: Some(item.system.as_str().into()),
            r#abstract: item.abstract_concept.then_some(true.into()),
            inactive: item.inactive.then_some(true.into()),
            code: Some(item.code.as_str().into()),
            display: item.display.as_deref().map(Into::into),
            designation,
            ..Default::default()
        });
    }
    Ok(out)
}

/// The designations of one expansion item that `designation` asks for: every
/// designation, or those whose language or `use` (`system|code`) is listed.
fn designations_of(
    sources: &Sources<'_>,
    item: &Item,
    language: Option<&str>,
    wanted: &[&str],
) -> Result<Vec<ValueSetComposeIncludeConceptDesignation>, OperationError> {
    let resolved = sources
        .registry
        .resolve(&item.system, Some(&item.version))?;
    let Some(located) = resolved.provider.locate(&item.code)? else {
        return Ok(Vec::new());
    };
    let selected = |d: &Designation| {
        if wanted.is_empty() {
            return language.is_none_or(|l| d.language.as_deref().is_none_or(|dl| dl == l));
        }
        wanted.iter().any(|w| match w.split_once('|') {
            Some((system, code)) => d
                .use_
                .as_ref()
                .is_some_and(|u| u.system == system && u.code == code),
            None => d.language.as_deref() == Some(*w),
        })
    };
    Ok(resolved
        .provider
        .designations(located.concept, None)?
        .into_iter()
        .filter(selected)
        .map(|d| ValueSetComposeIncludeConceptDesignation {
            language: d.language.as_deref().map(Into::into),
            r#use: d.use_.map(|u| Coding {
                system: Some(u.system.as_str().into()),
                code: Some(u.code.as_str().into()),
                display: u.display.as_deref().map(Into::into),
                ..Default::default()
            }),
            value: d.value.as_str().into(),
            ..Default::default()
        })
        .collect())
}

/// The model of an inline R4B `ValueSet`, for callers outside this module.
///
/// # Errors
///
/// Returns [`OperationError::ValueSetInvalid`] when the resource has no `url`
/// or an unknown filter operator.
pub fn model_of(value_set: &ValueSet) -> Result<ValueSetModel, OperationError> {
    convert::r4b::convert(value_set).map_err(|e| OperationError::ValueSetInvalid(e.to_string()))
}

/// `url|version`, or `url` alone for a system without a version.
fn canonical(url: &str, version: &str) -> String {
    if version.is_empty() {
        url.to_owned()
    } else {
        format!("{url}|{version}")
    }
}
