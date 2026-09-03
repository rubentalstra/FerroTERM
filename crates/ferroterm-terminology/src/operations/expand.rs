//! `ValueSet/$expand` in the terms every served FHIR version shares.
//!
//! The operation pages: <https://hl7.org/fhir/R4B/valueset-operation-expand.html>
//! and <https://hl7.org/fhir/R5/valueset-operation-expand.html>. The value set
//! is inline, stored, or implicit; its compose is pinned by the version
//! parameters, expanded as bitmap algebra, paged, and the page is read into
//! neutral items with their designations; the wire layer of each version
//! renders the `ValueSet` resource with its `expansion`.

use std::sync::Arc;

use super::{OperationError, Sources};
use crate::compose::{Compose, Expansion, Include, Item, Options};
use crate::provider::Designation;
use crate::valueset::model::{ModelError, ValueSetModel};
use crate::valueset::store::Resolver;
use crate::versioned::Versioned;

/// The most concepts an unpaged expansion returns before it is `too-costly`.
///
/// No specification names the limit; the ecosystem's servers refuse whole
/// expansions of the large systems, and a client pages with `count`.
pub const EXPANSION_LIMIT: u64 = 1000;

/// The input of `$expand`: the union of the parameters the served versions
/// declare.
#[derive(Debug, Default)]
pub struct ExpandInput {
    /// The value set URL (`url`).
    pub url: Option<String>,
    /// The value set version (`valueSetVersion`).
    pub value_set_version: Option<String>,
    /// The inline `valueSet`, converted by the wire layer of its version.
    pub inline_value_set: Option<Result<ValueSetModel, ModelError>>,
    /// Whether `context` or `contextDirection` was given; not supported.
    pub context: bool,
    /// Whether `date` was given; not supported.
    pub date: bool,
    /// The text filter.
    pub filter: Option<String>,
    /// The offset into the expansion.
    pub offset: Option<i64>,
    /// The page size.
    pub count: Option<i64>,
    /// `includeDesignations`.
    pub include_designations: Option<bool>,
    /// `designation`: the languages or `system|code` uses to include.
    pub designation: Vec<String>,
    /// `includeDefinition`.
    pub include_definition: Option<bool>,
    /// `activeOnly`.
    pub active_only: Option<bool>,
    /// `excludeNested`.
    pub exclude_nested: Option<bool>,
    /// `excludeNotForUI`.
    pub exclude_not_for_ui: Option<bool>,
    /// `excludePostCoordinated`.
    pub exclude_post_coordinated: Option<bool>,
    /// The language of the displays (a BCP 47 range list).
    pub display_language: Option<String>,
    /// `exclude-system` canonicals.
    pub exclude_system: Vec<String>,
    /// `system-version` canonicals.
    pub system_version: Vec<String>,
    /// `check-system-version` canonicals.
    pub check_system_version: Vec<String>,
    /// `force-system-version` canonicals.
    pub force_system_version: Vec<String>,
    /// `property` (R5): the properties to include on each concept.
    pub property: Vec<String>,
    /// `useSupplement` (R5): the supplements to apply.
    pub use_supplement: Vec<String>,
}

/// One echoed `expansion.parameter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionParameter {
    /// The parameter name.
    pub name: String,
    /// The value.
    pub value: ParameterValue,
}

/// The value of an echoed expansion parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterValue {
    /// A `string`.
    String(String),
    /// A `boolean`.
    Boolean(bool),
    /// An `integer`.
    Integer(i64),
    /// A `code`.
    Code(String),
    /// A `uri`.
    Uri(String),
}

/// One concept of the expansion page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contains {
    /// The code system URI.
    pub system: String,
    /// The code system version served.
    pub version: String,
    /// The code.
    pub code: String,
    /// The display in the requested language.
    pub display: Option<String>,
    /// Whether the concept is abstract (not selectable).
    pub abstract_concept: bool,
    /// Whether the concept is inactive.
    pub inactive: bool,
    /// The designations asked for.
    pub designations: Vec<Designation>,
}

/// The outcome of `$expand`.
#[derive(Debug, Clone)]
pub struct ExpansionOutcome {
    /// The value set expanded (its definition, for the resource).
    pub model: Arc<ValueSetModel>,
    /// Whether the compose is to be returned (`includeDefinition`).
    pub include_definition: bool,
    /// The expansion identifier (`urn:uuid:…`).
    pub identifier: String,
    /// The timestamp of the expansion.
    pub timestamp: String,
    /// The size of the whole expansion.
    pub total: u64,
    /// The offset, when the client paged.
    pub offset: Option<u64>,
    /// The parameters echoed, in order.
    pub parameters: Vec<ExpansionParameter>,
    /// The page.
    pub contains: Vec<Contains>,
}

/// Runs `$expand`.
///
/// # Errors
///
/// Returns [`OperationError`] for a value set that cannot be found or is
/// invalid, a parameter the operation does not support, a version pin the
/// value set contradicts, a negative page, an unpaged expansion beyond
/// [`EXPANSION_LIMIT`], or a provider failure.
pub fn expand(
    sources: &Sources<'_>,
    input: &ExpandInput,
) -> Result<ExpansionOutcome, OperationError> {
    refuse_unsupported(input)?;
    let model = sources.value_set(
        input.inline_value_set.clone(),
        input.url.as_deref(),
        input.value_set_version.as_deref(),
    )?;
    let compose = pinned_compose(&model.compose, input)?;
    let options = options(input)?;
    let resolver = Resolver::new(sources.registry, sources.value_sets);
    let expansion = resolver.expand_compose(&model.canonical(), &compose, &options)?;
    if options.count.is_none() && expansion.total > EXPANSION_LIMIT {
        return Err(OperationError::TooCostly(format!(
            "the expansion of `{}` has {} concepts; page it with `count` (and `offset`) to fetch it",
            model.canonical(),
            expansion.total
        )));
    }
    let contains = contains(sources, &expansion, input, options.language.as_deref())?;
    let offset = u64::try_from(expansion.offset)
        .map_err(|_| OperationError::Invalid(String::from("`offset` is too large")))?;
    Ok(ExpansionOutcome {
        include_definition: input.include_definition.unwrap_or(false),
        identifier: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
        timestamp: jiff::Timestamp::now().to_string(),
        total: expansion.total,
        offset: (input.offset.is_some() || input.count.is_some()).then_some(offset),
        parameters: parameters(input, &expansion),
        contains,
        model,
    })
}

fn refuse_unsupported(input: &ExpandInput) -> Result<(), OperationError> {
    if input.context {
        return Err(OperationError::NotSupported(String::from(
            "`context` and `contextDirection` are not supported; name the value set with `url` or `valueSet`",
        )));
    }
    if input.date {
        return Err(OperationError::NotSupported(String::from(
            "`date` is not supported: expansions are generated from the versions served now",
        )));
    }
    Ok(())
}

fn options(input: &ExpandInput) -> Result<Options, OperationError> {
    let non_negative = |value: Option<i64>, name: &str| -> Result<Option<usize>, OperationError> {
        value
            .map(|v| {
                usize::try_from(v)
                    .map_err(|_| OperationError::Invalid(format!("`{name}` must not be negative")))
            })
            .transpose()
    };
    Ok(Options {
        active_only: input.active_only.unwrap_or(false),
        text: input.filter.clone(),
        language: input.display_language.clone(),
        offset: non_negative(input.offset, "offset")?.unwrap_or(0),
        count: non_negative(input.count, "count")?,
    })
}

/// The canonicals of a version parameter as `(url, version)` pairs.
fn canonicals(list: &[String]) -> Vec<(String, Option<String>)> {
    list.iter()
        .map(|c| match c.split_once('|') {
            Some((url, version)) => (url.to_owned(), Some(version.to_owned())),
            None => (c.clone(), None),
        })
        .collect()
}

fn pinned_compose(compose: &Compose, input: &ExpandInput) -> Result<Compose, OperationError> {
    let excluded = canonicals(&input.exclude_system);
    let defaults = canonicals(&input.system_version);
    let checks = canonicals(&input.check_system_version);
    let forced = canonicals(&input.force_system_version);
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

/// Every parameter the client gave, echoed, then the code system versions
/// used (`used-codesystem`).
fn parameters(input: &ExpandInput, expansion: &Expansion) -> Vec<ExpansionParameter> {
    let mut out = Vec::new();
    let mut push = |name: &str, value: ParameterValue| {
        out.push(ExpansionParameter {
            name: name.to_owned(),
            value,
        });
    };
    if let Some(filter) = &input.filter {
        push("filter", ParameterValue::String(filter.clone()));
    }
    for (name, flag) in [
        ("activeOnly", input.active_only),
        ("excludeNested", input.exclude_nested),
        ("includeDesignations", input.include_designations),
        ("includeDefinition", input.include_definition),
        ("excludeNotForUI", input.exclude_not_for_ui),
        ("excludePostCoordinated", input.exclude_post_coordinated),
    ] {
        if let Some(flag) = flag {
            push(name, ParameterValue::Boolean(flag));
        }
    }
    for (name, number) in [("offset", input.offset), ("count", input.count)] {
        if let Some(number) = number {
            push(name, ParameterValue::Integer(number));
        }
    }
    if let Some(language) = &input.display_language {
        push("displayLanguage", ParameterValue::Code(language.clone()));
    }
    for designation in &input.designation {
        push("designation", ParameterValue::String(designation.clone()));
    }
    for property in &input.property {
        push("property", ParameterValue::Code(property.clone()));
    }
    for supplement in &input.use_supplement {
        push("useSupplement", ParameterValue::Uri(supplement.clone()));
    }
    for (name, list) in [
        ("exclude-system", &input.exclude_system),
        ("system-version", &input.system_version),
        ("check-system-version", &input.check_system_version),
        ("force-system-version", &input.force_system_version),
    ] {
        for value in list {
            push(name, ParameterValue::Uri(value.clone()));
        }
    }
    for used in &expansion.versions {
        push(
            "used-codesystem",
            ParameterValue::Uri(canonical(&used.url, &used.version)),
        );
    }
    out
}

fn contains(
    sources: &Sources<'_>,
    expansion: &Expansion,
    input: &ExpandInput,
    language: Option<&str>,
) -> Result<Vec<Contains>, OperationError> {
    let include_designations = input.include_designations.unwrap_or(false);
    let wanted: Vec<&str> = input.designation.iter().map(String::as_str).collect();
    let mut out = Vec::with_capacity(expansion.items.len());
    for item in &expansion.items {
        let designations = if include_designations {
            designations_of(sources, item, language, &wanted)?
        } else {
            Vec::new()
        };
        out.push(Contains {
            system: item.system.clone(),
            version: item.version.clone(),
            code: item.code.clone(),
            display: item.display.clone(),
            abstract_concept: item.abstract_concept,
            inactive: item.inactive,
            designations,
        });
    }
    Ok(out)
}

/// The designations of an item the client asked for: by language, or by
/// `system|code` use, or every one in the display language.
fn designations_of(
    sources: &Sources<'_>,
    item: &Item,
    language: Option<&str>,
    wanted: &[&str],
) -> Result<Vec<Designation>, OperationError> {
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
        .collect())
}

/// `url|version`, the canonical of a code system version.
fn canonical(url: &str, version: &str) -> String {
    format!("{url}|{version}")
}
