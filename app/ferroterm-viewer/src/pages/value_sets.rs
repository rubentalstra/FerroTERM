//! The value sets this root holds: the search, and one resource read by id.
//!
//! The screen reads and never writes. The RESTful API defines create, update,
//! and delete beside read and search
//! (<https://hl7.org/fhir/R4B/http.html>), and this server answers all of
//! them; the viewer calls none, so nothing here offers one.

use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query_map;

use crate::components::NOT_DECLARED;
use crate::components::failure::Failure;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::components::spinner::Spinner;
use crate::fhir::FhirClient;
use crate::fhir::VALUE_SET;
use crate::fhir::error::FhirError;
use crate::fhir::searchset::SearchFilter;
use crate::fhir::searchset::SearchSet;
use crate::fhir::value_set::ClauseRow;
use crate::fhir::value_set::PublishedValueSet;
use crate::fhir::version::FhirVersion;
use crate::listing::ListParams;
use crate::listing::canonical_cell;
use crate::listing::count_sentence;
use crate::listing::filter_form;
use crate::listing::pager_view;
use crate::listing::window;
use crate::routes::VALUE_SETS_PATH;
use crate::routes::expansion_link;
use crate::settings::Settings;

/// Lists the `ValueSet` resources this root holds, and reads one of them.
///
/// The filter, the page, and the resource being read all live in the address,
/// so every one of them is shareable and the back button walks them. Each is
/// read reactively: a click on a row is a navigation onto this same route, and
/// `leptos_router` 0.8.15 then updates the params without re-running this body
/// (`src/nested_router.rs`, the same-route-id branch).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn ValueSetsPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let settings = expect_context::<Settings>();

    let query = use_query_map();
    let params: Signal<ListParams> = Memo::new(move |_| {
        query.with(|map| ListParams::read(&|name| map.get(name), settings.page_size.get()))
    })
    .into();

    let heading = view! {
        <Title text="Value sets" />
        <h1 class="text-2xl font-semibold">"Value sets"</h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "The ValueSet resources this root holds, as it publishes them. Open one to see its definition, then run it in the expansion runner. This screen only reads."
        </p>
    }
    .into_any();

    let form = search_form(params, version);
    let list = list_section(&client, version, params);
    let detail = detail_section(&client, version, params);

    view! {
        {heading}
        {form}
        {list}
        {detail}
    }
}

/// The search filter, as a form that navigates rather than reloading.
fn search_form(params: Signal<ListParams>, version: Signal<FhirVersion>) -> AnyView {
    let canonical: NodeRef<Input> = NodeRef::new();
    let resource_version: NodeRef<Input> = NodeRef::new();
    let navigate = use_navigate();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let typed = params.with(|params| {
            params.searching(SearchFilter {
                url: typed_value(canonical),
                version: typed_value(resource_version),
            })
        });
        // NOTE: the router resolves a navigation against its base, so an
        // address that already carries the base is passed unresolved
        // (`leptos_router` 0.8.15 `matching/resolve_path.rs`).
        navigate(
            &typed.address(VALUE_SETS_PATH, version.get()),
            NavigateOptions {
                resolve: false,
                ..NavigateOptions::default()
            },
        );
    };
    filter_form(
        "valuesets",
        "The url search parameter, matched against ValueSet.url.",
        canonical,
        resource_version,
        params,
        Box::new(submit),
    )
}

/// The resources the search answered, a page at a time.
fn list_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<ListParams>,
) -> AnyView {
    let read_client = client.clone();
    let filter = Memo::new(move |_| params.with(|params| params.filter.clone()));
    let found = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let filter = filter.get();
        async move { client.value_set_search(version, &filter).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        filter.with(|filter| url_client.search_url(version.get(), VALUE_SET, filter))
    });

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives. Every
    // number in it comes from the answer rather than from the address, because
    // `<Transition>` keeps the previous rows on screen while the next read
    // runs and a sentence built from the address would describe rows that are
    // not there yet.
    let announcement = Memo::new(move |_| {
        let params = params.get();
        found.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|found| {
                    let view = window(params.page(), params.size(), found.matched());
                    count_sentence(view, "value sets", found.total())
                })
                .unwrap_or_default()
        })
    });

    view! {
        <section class="mt-8" aria-labelledby="valuesets-heading">
            <h2 id="valuesets-heading" class="text-lg font-medium">
                "What this root holds"
            </h2>
            <p aria-live="polite" class="mt-2 text-sm text-slate-600 dark:text-slate-300">
                {announcement}
            </p>
            <Transition fallback=|| {
                view! { <Spinner label="Reading the value sets" /> }
            }>
                {move || {
                    let params = params.get();
                    let version = version.get();
                    found
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(found) => list_view(found, &params, version),
                                    Err(error) => failure_view(error),
                                })
                        })
                }}
            </Transition>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The rows of one page, with the controls that walk the rest.
fn list_view(
    found: &SearchSet<PublishedValueSet>,
    params: &ListParams,
    version: FhirVersion,
) -> AnyView {
    let resources = found.found();
    if resources.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This root holds no ValueSet resource matching the filter above. A value set a code system defines implicitly is not published as a resource, and the expansion runner takes its canonical directly."
            </p>
        }
        .into_any();
    }
    let view = window(params.page(), params.size(), resources.len());
    let rows: Vec<AnyView> = view
        .indexes()
        .filter_map(|index| resources.get(index))
        .map(|resource| row_view(resource, params, version))
        .collect();
    let pager = pager_view(
        "Value set pages",
        view,
        params,
        VALUE_SETS_PATH,
        version,
        &[],
    );
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left text-sm">
                <thead>
                    <tr class="border-b border-slate-300 dark:border-slate-700">
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "Canonical"
                        </th>
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "Version"
                        </th>
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "Title"
                        </th>
                        <th scope="col" class="py-2 font-medium">
                            "Status"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
        {pager}
    }
    .into_any()
}

/// One published value set, as a row that opens it.
fn row_view(resource: &PublishedValueSet, params: &ListParams, version: FhirVersion) -> AnyView {
    let canonical = resource.url().unwrap_or(NOT_DECLARED).to_owned();
    let heading = canonical_cell(
        &canonical,
        resource
            .id()
            .map(|id| params.reading(id).address(VALUE_SETS_PATH, version)),
    );
    view! {
        <tr class="border-b border-slate-200 align-top dark:border-slate-800">
            <th scope="row" class="py-2 pr-3 font-mono text-xs font-normal break-all">
                {heading}
            </th>
            <td class="py-2 pr-3 font-mono text-xs">
                {resource.version().unwrap_or(NOT_DECLARED).to_owned()}
            </td>
            <td class="py-2 pr-3">{resource.label().unwrap_or(NOT_DECLARED).to_owned()}</td>
            <td class="py-2">{resource.status().unwrap_or(NOT_DECLARED).to_owned()}</td>
        </tr>
    }
    .into_any()
}

/// One value set read by its id, with what its definition selects.
fn detail_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<ListParams>,
) -> AnyView {
    let read_client = client.clone();
    let id = Memo::new(move |_| params.with(|params| params.id.clone()));
    let resource = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let id = id.get();
        async move {
            if id.is_empty() {
                None
            } else {
                Some(client.value_set_read(version, &id).await)
            }
        }
    });
    let url_client = client.clone();
    let url =
        Signal::derive(move || id.with(|id| url_client.resource_url(version.get(), VALUE_SET, id)));

    view! {
        <Show when=move || id.with(|id| !id.is_empty()) fallback=|| ()>
            <section class="mt-8" aria-labelledby="valueset-detail-heading">
                <h2 id="valueset-detail-heading" class="text-lg font-medium">
                    "The value set you opened"
                </h2>
                <Transition fallback=|| {
                    view! { <Spinner label="Reading the value set" /> }
                }>
                    {move || {
                        let version = version.get();
                        resource
                            .with(|answered| {
                                answered
                                    .as_ref()
                                    .map(|answer| match answer {
                                        None => ().into_any(),
                                        Some(Ok(resource)) => resource_view(resource, version),
                                        Some(Err(error)) => failure_view(error),
                                    })
                            })
                    }}
                </Transition>
                <RequestDisclosure url />
            </section>
        </Show>
    }
    .into_any()
}

/// One resource: its facts, its definition, and the runner that expands it.
fn resource_view(resource: &PublishedValueSet, version: FhirVersion) -> AnyView {
    let rows: Vec<AnyView> = resource
        .facts()
        .into_iter()
        .map(|fact| {
            let value = fact.value.unwrap_or_else(|| NOT_DECLARED.to_owned());
            view! {
                <div class="grid gap-1 border-b border-slate-100 py-1 last:border-0 sm:grid-cols-[16rem_1fr] dark:border-slate-800">
                    <dt class="font-medium">{fact.label}</dt>
                    <dd class="break-words">{value}</dd>
                </div>
            }
            .into_any()
        })
        .collect();
    let canonical = resource.url().map(str::to_owned);
    let heading = canonical
        .clone()
        .unwrap_or_else(|| format!("Canonical {NOT_DECLARED}"));
    let run = canonical.map_or_else(
        || {
            view! {
                <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                    "This resource declares no canonical, so there is no url for the expansion runner to expand."
                </p>
            }
            .into_any()
        },
        |canonical| {
            let href = expansion_link(&canonical, version);
            view! {
                <p class="mt-3">
                    <a href=href class="text-brand-700 underline dark:text-brand-300">
                        "Run this value set in the expansion runner"
                    </a>
                </p>
            }
            .into_any()
        },
    );
    let clauses = clauses_view(&resource.clauses());
    view! {
        <article class="mt-3 rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900">
            <h3 class="font-mono text-sm font-semibold break-all">{heading}</h3>
            {run}
            <dl class="mt-3 text-sm">{rows}</dl>
            {clauses}
        </article>
    }
    .into_any()
}

/// What the definition selects, one clause at a time.
///
/// The clauses are a plain `Vec`, which rebuilds every position when another
/// resource is read. A `<For>` key it retained would be moved rather than
/// re-rendered, so opening a second value set would keep the first one's rows.
fn clauses_view(clauses: &[ClauseRow]) -> AnyView {
    if clauses.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This resource carries no compose element, so its content is whatever the server holds for it rather than a selection written here."
            </p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = clauses.iter().map(clause_row).collect();
    view! {
        <div class="mt-4 overflow-x-auto">
            <table class="w-full border-collapse text-left text-sm">
                <caption class="pb-1 text-left text-xs font-medium tracking-wide uppercase">
                    "What the definition selects"
                </caption>
                <thead>
                    <tr class="border-b border-slate-200 dark:border-slate-700">
                        <th scope="col" class="py-1 pr-3 text-xs font-medium">
                            "Clause"
                        </th>
                        <th scope="col" class="py-1 pr-3 text-xs font-medium">
                            "Code system"
                        </th>
                        <th scope="col" class="py-1 text-xs font-medium">
                            "Selects"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// One clause, with the word that says whether it draws codes in or out.
fn clause_row(clause: &ClauseRow) -> AnyView {
    let kind = if clause.included {
        "include"
    } else {
        "exclude"
    };
    let system = match (clause.system.as_deref(), clause.version.as_deref()) {
        (Some(system), Some(version)) => format!("{system}|{version}"),
        (Some(system), None) => system.to_owned(),
        (None, _) => NOT_DECLARED.to_owned(),
    };
    view! {
        <tr class="border-b border-slate-100 align-top last:border-0 dark:border-slate-800">
            <th scope="row" class="py-1 pr-3 text-xs font-normal">
                {kind}
            </th>
            <td class="py-1 pr-3 font-mono text-xs break-all">{system}</td>
            <td class="py-1 text-xs">{selection_view(clause)}</td>
        </tr>
    }
    .into_any()
}

/// What one clause selects, as the lines the publisher wrote.
fn selection_view(clause: &ClauseRow) -> AnyView {
    let mut lines: Vec<String> = Vec::new();
    if clause.codes > 0 {
        lines.push(match clause.codes {
            1 => "1 code named one by one".to_owned(),
            codes => format!("{codes} codes named one by one"),
        });
    }
    lines.extend(clause.filters.iter().cloned());
    lines.extend(
        clause
            .value_sets
            .iter()
            .map(|url| format!("every code in {url}")),
    );
    if lines.is_empty() {
        return view! { "the whole code system" }.into_any();
    }
    let items: Vec<AnyView> = lines
        .into_iter()
        .map(|line| view! { <li class="break-all">{line}</li> }.into_any())
        .collect();
    view! { <ul class="ml-4 list-disc">{items}</ul> }.into_any()
}

/// A refused read, in the server's own words.
fn failure_view(error: &FhirError) -> AnyView {
    let error = error.clone();
    view! {
        <div class="mt-3">
            <Failure error=Signal::stored(error) />
        </div>
    }
    .into_any()
}

/// What a control holds now, trimmed, or the empty string when it is gone.
fn typed_value(node: NodeRef<Input>) -> String {
    node.get()
        .map(|input| input.value())
        .unwrap_or_default()
        .trim()
        .to_owned()
}
