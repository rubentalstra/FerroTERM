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
use crate::provider::{Designation, Property};
use crate::valueset::model::{ModelError, ValueSetModel};
use crate::valueset::negotiation::{Negotiation, canonicals};
use crate::valueset::store::Resolver;
use crate::versioned::Versioned;

/// The most concepts an unpaged expansion returns before it is `too-costly`.
///
/// No specification names the limit; the ecosystem's servers refuse whole
/// expansions of the large systems, and a client pages with `count`.
pub const EXPANSION_LIMIT: u64 = 1000;

/// The input of `$expand`: the union of the parameters the served versions
/// declare.
#[derive(Debug, Default, Clone)]
pub struct ExpandInput {
    /// The names of parameters the version declares that the server does not
    /// implement; a request naming one is refused, never absorbed.
    pub unsupported: Vec<String>,
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
    /// `default-valueset-version` canonicals: the version of a value set
    /// reference that names none (pre-adopted from R6).
    pub default_valueset_version: Vec<String>,
    /// `check-valueset-version` canonicals: a version a differing value set
    /// reference is refused against (pre-adopted from R6).
    pub check_valueset_version: Vec<String>,
    /// `force-valueset-version` canonicals: a version that overrides the value
    /// set reference's (pre-adopted from R6).
    pub force_valueset_version: Vec<String>,
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
    /// The properties asked for with `property`
    /// (<https://hl7.org/fhir/R5/valueset-operation-expand.html>).
    pub properties: Vec<Property>,
}

/// One property the expansion returns on its concepts, for
/// `expansion.property` (<https://hl7.org/fhir/R5/valueset.html>).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionProperty {
    /// The property code, as `contains.property.code` names it.
    pub code: String,
    /// The formal URI of the property, when the code system declares one.
    pub uri: Option<String>,
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
    /// The properties the concepts carry, each once, in order of appearance.
    pub properties: Vec<ExpansionProperty>,
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
    crate::language::check(input.display_language.as_deref())?;
    refuse_unsupported(input)?;
    let negotiation = negotiation(input);
    let (url, version) = match input.url.as_deref() {
        Some(url) => {
            let (url, version) = negotiation.value_set(url, input.value_set_version.as_deref())?;
            (Some(url), version)
        }
        None => (None, input.value_set_version.clone()),
    };
    let model = sources.value_set(
        input.inline_value_set.clone(),
        url.as_deref(),
        version.as_deref(),
    )?;
    let input = &with_defaults(input, &model);
    let compose = pinned_compose(&model.compose, input, &negotiation)?;
    let options = options(input)?;
    let mut wanted = input.use_supplement.clone();
    wanted.extend(model.supplements.iter().cloned());
    let registry = sources.with_supplements(&wanted)?;
    let sources = &Sources {
        registry: &registry,
        ..*sources
    };
    let resolver =
        Resolver::new(sources.registry, sources.value_sets).with_negotiation(&negotiation);
    let expansion = resolver.expand_compose(&model.canonical(), &compose, &options)?;
    let used_value_sets = resolver.used_value_sets();
    if options.count.is_none() && expansion.total > EXPANSION_LIMIT {
        return Err(OperationError::TooCostly(format!(
            "the expansion of `{}` has {} concepts; page it with `count` (and `offset`) to fetch it",
            model.canonical(),
            expansion.total
        )));
    }
    let contains = contains(sources, &expansion, input)?;
    let properties = expansion_properties(sources, &expansion, &contains)?;
    // The message says what is wrong; the conversion error adds nothing to it.
    let Ok(offset) = u64::try_from(expansion.offset) else {
        return Err(OperationError::Invalid(String::from(
            "`offset` is too large",
        )));
    };
    Ok(ExpansionOutcome {
        include_definition: input.include_definition.unwrap_or(false),
        identifier: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
        timestamp: jiff::Timestamp::now().to_string(),
        total: expansion.total,
        offset: (input.offset.is_some() || input.count.is_some()).then_some(offset),
        parameters: parameters(input, &expansion, &used_value_sets),
        contains,
        properties,
        model,
    })
}

/// The request with the value set's default expansion parameters filled in
/// where the client named none (`valueset-expansion-parameter`,
/// <https://hl7.org/fhir/R4B/extension-valueset-expansion-parameter.html>).
fn with_defaults(input: &ExpandInput, model: &ValueSetModel) -> ExpandInput {
    let mut input = input.clone();
    for default in &model.expansion_parameters {
        match default.name.as_str() {
            "displayLanguage" if input.display_language.is_none() => {
                input.display_language = Some(default.value.clone());
            }
            "activeOnly" if input.active_only.is_none() => {
                input.active_only = Some(default.value == "true");
            }
            "excludeNested" if input.exclude_nested.is_none() => {
                input.exclude_nested = Some(default.value == "true");
            }
            "includeDesignations" if input.include_designations.is_none() => {
                input.include_designations = Some(default.value == "true");
            }
            _ => {}
        }
    }
    input
}

fn refuse_unsupported(input: &ExpandInput) -> Result<(), OperationError> {
    if let Some(name) = input.unsupported.first() {
        return Err(OperationError::NotSupported(format!(
            "`{name}` is not supported by this server"
        )));
    }
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
                let Ok(value) = usize::try_from(v) else {
                    return Err(OperationError::Invalid(format!(
                        "`{name}` must not be negative"
                    )));
                };
                Ok(value)
            })
            .transpose()
    };
    Ok(Options {
        active_only: input.active_only.unwrap_or(false),
        exclude_not_for_ui: input.exclude_not_for_ui.unwrap_or(false),
        exclude_post_coordinated: input.exclude_post_coordinated.unwrap_or(false),
        text: input.filter.clone(),
        language: input.display_language.clone(),
        offset: non_negative(input.offset, "offset")?.unwrap_or(0),
        count: non_negative(input.count, "count")?,
    })
}

/// The version negotiation the request asks for.
fn negotiation(input: &ExpandInput) -> Negotiation {
    Negotiation::new(
        &input.system_version,
        &input.check_system_version,
        &input.force_system_version,
        &input.default_valueset_version,
        &input.check_valueset_version,
        &input.force_valueset_version,
    )
}

/// `compose` less the `exclude-system` includes, with its systems at their
/// negotiated versions.
fn pinned_compose(
    compose: &Compose,
    input: &ExpandInput,
    negotiation: &Negotiation,
) -> Result<Compose, OperationError> {
    let excluded = canonicals(&input.exclude_system);
    let keep = |include: &Include| -> bool {
        let Some(system) = &include.system else {
            return true;
        };
        !excluded.iter().any(|(url, version)| {
            *url == system.url
                && version
                    .as_ref()
                    .is_none_or(|v| Some(v) == system.version.as_ref())
        })
    };
    let kept = Compose {
        include: compose
            .include
            .iter()
            .filter(|i| keep(i))
            .cloned()
            .collect(),
        exclude: compose
            .exclude
            .iter()
            .filter(|i| keep(i))
            .cloned()
            .collect(),
        inactive: compose.inactive,
    };
    Ok(negotiation.pin(&kept)?)
}

/// Every parameter the client gave, echoed, then the code system versions
/// used (`used-codesystem`).
fn parameters(
    input: &ExpandInput,
    expansion: &Expansion,
    used_value_sets: &[String],
) -> Vec<ExpansionParameter> {
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
    for supplement in &input.use_supplement {
        push("useSupplement", ParameterValue::Uri(supplement.clone()));
    }
    for (name, list) in [
        ("exclude-system", &input.exclude_system),
        ("system-version", &input.system_version),
        ("check-system-version", &input.check_system_version),
        ("force-system-version", &input.force_system_version),
        ("default-valueset-version", &input.default_valueset_version),
        ("check-valueset-version", &input.check_valueset_version),
        ("force-valueset-version", &input.force_valueset_version),
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
    // NOTE: the value sets an expansion drew on, the ecosystem's `used-valueset`
    // (<https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>, `$expand parameters`).
    for used in used_value_sets {
        push("used-valueset", ParameterValue::Uri(used.clone()));
    }
    out
}

fn contains(
    sources: &Sources<'_>,
    expansion: &Expansion,
    input: &ExpandInput,
) -> Result<Vec<Contains>, OperationError> {
    let include_designations = input.include_designations.unwrap_or(false);
    let wanted: Vec<&str> = input.designation.iter().map(String::as_str).collect();
    let mut out = Vec::with_capacity(expansion.items.len());
    for item in &expansion.items {
        let designations = if include_designations {
            designations_of(sources, item, &wanted)?
        } else {
            Vec::new()
        };
        // NOTE: an inactive concept carries its status property unasked, the
        // ecosystem's shape ("in expansions, the status property SHALL be populated",
        // <https://hl7.org/fhir/uv/tx-ecosystem/1.9.3/requirements.html>).
        let mut asked = input.property.clone();
        if item.inactive && !asked.iter().any(|p| p == "status" || p == "*") {
            asked.push(String::from("status"));
        }
        let properties = if asked.is_empty() {
            Vec::new()
        } else {
            properties_of(sources, item, &asked)?
        };
        out.push(Contains {
            system: item.system.clone(),
            version: item.version.clone(),
            code: item.code.clone(),
            display: item.display.clone(),
            abstract_concept: item.abstract_concept,
            inactive: item.inactive,
            designations,
            properties,
        });
    }
    Ok(out)
}

/// The designations of an item the client asked for: by language, or by
/// `system|code` use, or every one when none is named.
fn designations_of(
    sources: &Sources<'_>,
    item: &Item,
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
            return true;
        }
        wanted.iter().any(|w| match w.split_once('|') {
            // NOTE: `urn:ietf:bcp:47|<lang>` names a language, as the parameter's
            // definition spells it (<https://hl7.org/fhir/R5/valueset-operation-expand.html>).
            Some(("urn:ietf:bcp:47", language)) => d.language.as_deref() == Some(language),
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

/// `url|version`, the canonical of a code system version; `url` alone for a
/// versionless system.
fn canonical(url: &str, version: &str) -> String {
    if version.is_empty() {
        url.to_owned()
    } else {
        format!("{url}|{version}")
    }
}

/// The properties of an item the client asked for: every one for `*`, else
/// those whose code or declared URI is named.
fn properties_of(
    sources: &Sources<'_>,
    item: &Item,
    wanted: &[String],
) -> Result<Vec<Property>, OperationError> {
    let resolved = sources
        .registry
        .resolve(&item.system, Some(&item.version))?;
    let Some(located) = resolved.provider.locate(&item.code)? else {
        return Ok(Vec::new());
    };
    let all = wanted.iter().any(|w| w == "*");
    let declaration = resolved.provider.declaration();
    let asked = |code: &str| {
        all || wanted.iter().any(|w| {
            w == code
                || declaration
                    .properties
                    .iter()
                    .any(|p| p.code == code && p.uri.as_deref() == Some(w.as_str()))
        })
    };
    Ok(resolved
        .provider
        .properties(located.concept)?
        .into_iter()
        .filter(|p| asked(&p.code))
        .collect())
}

/// The distinct properties the page carries, with the URI the code system
/// declares for each, for `expansion.property`.
fn expansion_properties(
    sources: &Sources<'_>,
    expansion: &Expansion,
    contains: &[Contains],
) -> Result<Vec<ExpansionProperty>, OperationError> {
    let mut out: Vec<ExpansionProperty> = Vec::new();
    for (item, entry) in expansion.items.iter().zip(contains) {
        for property in &entry.properties {
            if out.iter().any(|p| p.code == property.code) {
                continue;
            }
            let resolved = sources
                .registry
                .resolve(&item.system, Some(&item.version))?;
            let uri = resolved
                .provider
                .declaration()
                .properties
                .iter()
                .find(|p| p.code == property.code)
                .and_then(|p| p.uri.clone());
            out.push(ExpansionProperty {
                code: property.code.clone(),
                uri,
            });
        }
    }
    Ok(out)
}
