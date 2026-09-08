//! The concept maps this root holds, as the resources it publishes.
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
use crate::components::field::Help;
use crate::components::reading::Reading;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::fhir::CONCEPT_MAP;
use crate::fhir::FhirClient;
use crate::fhir::concept_map::GroupRow;
use crate::fhir::concept_map::PublishedConceptMap;
use crate::fhir::error::FhirError;
use crate::fhir::searchset::SearchFilter;
use crate::fhir::searchset::SearchSet;
use crate::fhir::translate::TranslateRequest;
use crate::fhir::version::FhirVersion;
use crate::listing::Action;
use crate::listing::ListParams;
use crate::listing::Published;
use crate::listing::SortColumn;
use crate::listing::SortOrder;
use crate::listing::count_sentence;
use crate::listing::empty;
use crate::listing::filter_form;
use crate::listing::heading;
use crate::listing::pager_view;
use crate::listing::published_row;
use crate::listing::sortable;
use crate::listing::table;
use crate::listing::window;
use crate::pages::translate;
use crate::routes::CONCEPT_MAPS_PATH;
use crate::settings::Settings;
use crate::styles;

/// The address parameter carrying the concept map the runner translates with.
const MAP_PARAM: &str = "map";

/// The address parameter carrying that concept map's business version.
const MAP_VERSION_PARAM: &str = "mapVersion";

/// The address parameter carrying the code system the code belongs to.
const SYSTEM_PARAM: &str = "system";

/// The address parameter carrying that code system's version.
const SYSTEM_VERSION_PARAM: &str = "systemVersion";

/// The address parameter carrying the code to translate.
const CODE_PARAM: &str = "code";

/// The address parameter carrying the code system an answer is wanted in.
const TARGET_PARAM: &str = "target";

/// Lists the `ConceptMap` resources this root holds, and translates with them.
///
/// The filter, the page, the map being read, and every parameter of a run all
/// live in the address, so a translation is a link you can share and the back
/// button walks the runs. Each is read reactively: a submit is a navigation
/// onto this same route, and `leptos_router` 0.8.15 then updates the params
/// without re-running this body (`src/nested_router.rs`, the same-route-id
/// branch).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn ConceptMapsPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let settings = expect_context::<Settings>();
    provide_context(Help(RwSignal::new(false)));

    let query = use_query_map();
    let params: Signal<ListParams> = Memo::new(move |_| {
        query.with(|map| ListParams::read(&|name| map.get(name), settings.page_size.get()))
    })
    .into();
    // An address written before the runner had its own screen still names a
    // run, so it is sent to the screen that answers it rather than listing
    // maps and dropping what the link asked for.
    let carried = Memo::new(move |_| query.with(|map| read_run(&|name| map.get(name))));
    let navigate = StoredValue::new(use_navigate());
    Effect::new(move |_| {
        let run = carried.get();
        if run.runnable() {
            let target = translate::address(&run, version.get());
            navigate.with_value(|navigate| navigate(&target, unresolved()));
        }
    });

    let heading = view! {
        <Title text="Concept maps" />
        <h1 class=styles::PAGE_TITLE>"Concept maps"</h1>
        <p class=styles::LEAD>
            "The ConceptMap resources this root holds, as it publishes them. Open one to see the groups it maps, then run a code through it in the translate runner. This screen only reads."
        </p>
    }
    .into_any();

    let form = search_form(params, version);
    // The order lives in the address, so an ordered list is a link.
    let order = Memo::new(move |_| query.with(|map| SortOrder::read(&|name| map.get(name))));
    let list = list_section(&client, version, params, order);
    let detail = detail_section(&client, version, params);

    view! {
        {heading}
        {form}
        {list}
        {detail}
    }
}

/// The parameters of a run, as the address carries them.
fn read_run(query: &dyn Fn(&str) -> Option<String>) -> TranslateRequest {
    let text = |name: &str| query(name).unwrap_or_default().trim().to_owned();
    TranslateRequest {
        concept_map: text(MAP_PARAM),
        concept_map_version: text(MAP_VERSION_PARAM),
        system: text(SYSTEM_PARAM),
        system_version: text(SYSTEM_VERSION_PARAM),
        code: text(CODE_PARAM),
        target_system: text(TARGET_PARAM),
    }
}

/// The address the list is at, for a link or a navigation.
fn address(params: &ListParams, version: FhirVersion) -> String {
    params.address(CONCEPT_MAPS_PATH, version)
}

/// The search filter, as a form that navigates rather than reloading.
fn search_form(params: Signal<ListParams>, version: Signal<FhirVersion>) -> AnyView {
    let canonical: NodeRef<Input> = NodeRef::new();
    let resource_version: NodeRef<Input> = NodeRef::new();
    let navigate = use_navigate();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let searched = params.with(|params| {
            params.searching(SearchFilter {
                url: typed_value(canonical),
                version: typed_value(resource_version),
            })
        });
        navigate(&address(&searched, version.get()), unresolved());
    };
    filter_form(
        "The url search parameter, matched against ConceptMap.url.",
        canonical,
        resource_version,
        params,
        Box::new(submit),
    )
}

/// A refused read, in the server's own words.
fn failure_view(error: &FhirError) -> AnyView {
    let error = error.clone();
    view! {
        <div class="mt-default">
            <Failure error=Signal::stored(error) />
        </div>
    }
    .into_any()
}

/// The navigation options an address that already carries the base needs.
// NOTE: the router resolves a navigation against its base, so an address that
// already carries the base is passed unresolved (`leptos_router` 0.8.15
// `matching/resolve_path.rs`).
fn unresolved() -> NavigateOptions {
    NavigateOptions {
        resolve: false,
        ..NavigateOptions::default()
    }
}

/// The resources the search answered, a page at a time.
fn list_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<ListParams>,
    order: Memo<SortOrder>,
) -> AnyView {
    let read_client = client.clone();
    let filter = Memo::new(move |_| params.with(|params| params.filter.clone()));
    let found = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let filter = filter.get();
        async move { client.concept_map_search(version, &filter).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        filter.with(|filter| url_client.search_url(version.get(), CONCEPT_MAP, filter))
    });

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives. Every
    // number in it comes from the answer rather than from the address, because
    // the transition boundary keeps the previous rows on screen while the next
    // read runs and a sentence built from the address would describe rows that
    // are not there yet.
    let announcement = Memo::new(move |_| {
        let params = params.get();
        found.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|found| {
                    let view = window(params.page(), params.size(), found.matched());
                    count_sentence(view, "concept maps", found.total())
                })
                .unwrap_or_default()
        })
    });

    view! {
        <section class="mt-section" aria-labelledby="conceptmaps-heading">
            <h2 id="conceptmaps-heading" class=styles::SECTION_TITLE>
                "What this root holds"
            </h2>
            <p aria-live="polite" class="mt-default text-body text-muted">
                {announcement}
            </p>
            <Reading label="Reading the concept maps">
                {move || {
                    let params = params.get();
                    let version = version.get();
                    found
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(found) => list_view(order.get(), found, &params, version),
                                    Err(error) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The rows of one page, with the controls that walk the rest.
fn list_view(
    order: SortOrder,
    found: &SearchSet<PublishedConceptMap>,
    params: &ListParams,
    version: FhirVersion,
) -> AnyView {
    let resources = found.found();
    if resources.is_empty() {
        return empty(
            "This root holds no ConceptMap resource matching the filter above. The translate runner still works: a server may translate through a map it holds without publishing it as a resource.",
        );
    }
    // The whole answer is ordered before it is paged, so a walk through the
    // pages walks the order the reader asked for rather than the server's.
    let mut ordered: Vec<(Published<'_>, usize)> = resources
        .iter()
        .enumerate()
        .map(|(index, resource)| (facts(resource), index))
        .collect();
    order.ordering(&mut ordered);
    let view = window(params.page(), params.size(), resources.len());
    let rows: Vec<AnyView> = view
        .indexes()
        .filter_map(|index| ordered.get(index))
        .filter_map(|(_, index)| resources.get(*index))
        .map(|resource| row_view(resource, params, version))
        .collect();
    let pager = pager_view(
        "Concept map pages",
        view,
        params,
        CONCEPT_MAPS_PATH,
        version,
        &[],
    );
    view! {
        {table(
            vec![
                sortable(
                    SortColumn::Name,
                    "Concept map",
                    order,
                    CONCEPT_MAPS_PATH,
                    params,
                    version,
                ),
                sortable(SortColumn::Version, "Version", order, CONCEPT_MAPS_PATH, params, version),
                sortable(SortColumn::Status, "Status", order, CONCEPT_MAPS_PATH, params, version),
                heading("Open in"),
            ],
            rows,
        )}
        {pager}
    }
    .into_any()
}

/// The four facts a row draws about one published concept map.
fn facts(resource: &PublishedConceptMap) -> Published<'_> {
    Published {
        title: resource.label(),
        canonical: resource.url(),
        version: resource.version(),
        status: resource.status(),
    }
}

/// One published concept map, as a row that opens it and one that runs it.
fn row_view(resource: &PublishedConceptMap, params: &ListParams, version: FhirVersion) -> AnyView {
    // The runner is handed the map and the source system this resource
    // already names, so a reader never retypes a canonical the row shows them.
    let filled = resource.url().map(|canonical| TranslateRequest {
        concept_map: canonical.to_owned(),
        concept_map_version: resource.version().unwrap_or_default().to_owned(),
        system: resource.only_source_system().unwrap_or_default(),
        ..TranslateRequest::default()
    });
    published_row(
        &facts(resource),
        resource
            .id()
            .map(|id| address(&params.reading(id), version)),
        filled.map(|run| Action {
            label: "Translate",
            href: translate::address(&run, version),
        }),
    )
}

/// One concept map read by its id, with the groups it maps.
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
                Some(client.concept_map_read(version, &id).await)
            }
        }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        id.with(|id| url_client.resource_url(version.get(), CONCEPT_MAP, id))
    });

    view! {
        <Show when=move || id.with(|id| !id.is_empty()) fallback=|| ()>
            <section class="mt-section" aria-labelledby="conceptmap-detail-heading">
                <h2 id="conceptmap-detail-heading" class=styles::SECTION_TITLE>
                    "The concept map you opened"
                </h2>
                <Reading label="Reading the concept map">
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
                </Reading>
                <RequestDisclosure url />
            </section>
        </Show>
    }
    .into_any()
}

/// One resource: its facts, its groups, and the link that runs a code
/// through it.
fn resource_view(resource: &PublishedConceptMap, version: FhirVersion) -> AnyView {
    let rows: Vec<AnyView> = resource
        .facts()
        .into_iter()
        .map(|fact| {
            let value = fact.value.unwrap_or_else(|| NOT_DECLARED.to_owned());
            view! {
                <div class="grid gap-tight border-b border-line py-tight last:border-0 sm:grid-cols-[16rem_1fr]">
                    <dt class="font-medium">{fact.label}</dt>
                    <dd class="wrap-break-word">{value}</dd>
                </div>
            }
            .into_any()
        })
        .collect();
    let heading = resource
        .url()
        .map_or_else(|| format!("Canonical {NOT_DECLARED}"), str::to_owned);
    let prefill = resource.url().map_or_else(
        || {
            view! {
                <p class="mt-default text-body text-muted">
                    "This resource declares no canonical, so there is no url to name it by in a run."
                </p>
            }
            .into_any()
        },
        |canonical| {
            // The link hands the runner what this resource already says, so a
            // reader never retypes a canonical the screen is showing them.
            let filled = TranslateRequest {
                concept_map: canonical.to_owned(),
                concept_map_version: resource.version().unwrap_or_default().to_owned(),
                system: resource.only_source_system().unwrap_or_default(),
                ..TranslateRequest::default()
            };
            let href = translate::address(&filled, version);
            view! {
                <p class="mt-default">
                    <a href=href class="text-accent underline">
                        "Translate with this concept map"
                    </a>
                </p>
            }
            .into_any()
        },
    );
    let groups = groups_view(&resource.groups());
    view! {
        <article class=format!("mt-default panel-p {}", styles::PANEL)>
            <h3 class="font-mono text-body font-semibold break-all">{heading}</h3>
            {prefill}
            <dl class="mt-default text-body">{rows}</dl>
            {groups}
        </article>
    }
    .into_any()
}

/// What the map maps, one group at a time.
fn groups_view(groups: &[GroupRow]) -> AnyView {
    if groups.is_empty() {
        return view! {
            <p class="mt-default text-body text-muted">
                "This resource declares no group, so what it maps is whatever the server holds for it rather than a table written here."
            </p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = groups
        .iter()
        .map(|group| {
            view! {
                <tr class="border-b border-line align-top last:border-0">
                    <th
                        scope="row"
                        class="py-tight pr-default font-mono text-small font-normal break-all"
                    >
                        {group.source.clone().unwrap_or_else(|| NOT_DECLARED.to_owned())}
                    </th>
                    <td class="py-tight pr-default font-mono text-small break-all">
                        {group.target.clone().unwrap_or_else(|| NOT_DECLARED.to_owned())}
                    </td>
                    <td class="py-tight text-small">{group.elements.to_string()}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    view! {
        <div class="mt-loose overflow-x-auto">
            <table class="w-full border-collapse text-left text-body">
                <caption class="pb-tight text-left text-small font-medium tracking-wide uppercase">
                    "What the map maps"
                </caption>
                <thead>
                    <tr class="border-b border-line">
                        <th scope="col" class="py-tight pr-default text-small font-medium">
                            "From"
                        </th>
                        <th scope="col" class="py-tight pr-default text-small font-medium">
                            "To"
                        </th>
                        <th scope="col" class="py-tight text-small font-medium">
                            "Codes mapped"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the address, which reads the same way a `ParamsMap` does.
    fn query<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + use<'a> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn opening_a_map_keeps_the_page_the_reader_is_on() {
        let params = ListParams::read(&query(&[("page", "2")]), 25);
        let opened = address(&params.reading("m-tight"), FhirVersion::R5);
        assert!(opened.contains("id=m-tight"), "{opened}");
        assert!(opened.contains("page=2"), "{opened}");
    }

    #[test]
    fn an_address_written_before_the_runner_moved_still_names_a_run() {
        let run = read_run(&query(&[
            ("map", "https://terminology.example/ConceptMap/m"),
            ("system", "https://terminology.example/a"),
            ("code", "x"),
        ]));
        assert!(
            run.runnable(),
            "a link shared before the split still says what to translate"
        );
    }

    #[test]
    fn an_address_naming_no_code_is_a_listing_rather_than_a_run() {
        let run = read_run(&query(&[(
            "url",
            "https://terminology.example/ConceptMap/m",
        )]));
        assert!(
            !run.runnable(),
            "a search filter is not a run, so the screen lists rather than redirecting"
        );
    }
}
