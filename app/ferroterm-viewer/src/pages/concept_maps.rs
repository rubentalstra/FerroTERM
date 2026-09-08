//! The concept maps this root holds, and the `$translate` runner over them.
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
use crate::components::field::Field;
use crate::components::field::Help;
use crate::components::field::group;
use crate::components::field::help_toggle;
use crate::components::field::row;
use crate::components::field::text_field;
use crate::components::icon;
use crate::components::icon::Icon;
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
use crate::fhir::translate::Coding;
use crate::fhir::translate::NamedValue;
use crate::fhir::translate::TranslateAnswer;
use crate::fhir::translate::TranslateRequest;
use crate::fhir::translate::TranslationMatch;
use crate::fhir::version::FhirVersion;
use crate::listing::ListParams;
use crate::listing::canonical_cell;
use crate::listing::count_sentence;
use crate::listing::filter_form;
use crate::listing::pager_view;
use crate::listing::window;
use crate::routes::CONCEPT_MAPS_PATH;
use crate::settings::Settings;
use crate::styles;

/// The operation this screen gates its runner on.
const TRANSLATE: &str = "translate";

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
    let run: Signal<TranslateRequest> =
        Memo::new(move |_| query.with(|map| read_run(&|name| map.get(name)))).into();

    let heading = view! {
        <Title text="Concept maps" />
        <h1 class=styles::PAGE_TITLE>"Concept maps"</h1>
        <p class=styles::LEAD>
            "The ConceptMap resources this root holds, and the translation runner over them. Every run below is one GET this server answers to any client. This screen only reads."
        </p>
    }
    .into_any();

    let form = search_form(params, run, version);
    let list = list_section(&client, version, params, run);
    let detail = detail_section(&client, version, params, run);
    let runner = runner_section(&client, version, params, run);

    view! {
        {heading}
        {form}
        {list}
        {detail}
        {runner}
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

/// A run as the address parameters that reproduce it.
fn run_pairs(run: &TranslateRequest) -> Vec<(&'static str, String)> {
    vec![
        (MAP_PARAM, run.concept_map.clone()),
        (MAP_VERSION_PARAM, run.concept_map_version.clone()),
        (SYSTEM_PARAM, run.system.clone()),
        (SYSTEM_VERSION_PARAM, run.system_version.clone()),
        (CODE_PARAM, run.code.clone()),
        (TARGET_PARAM, run.target_system.clone()),
    ]
}

/// The whole address: the list half, and the run the reader has going.
fn address(params: &ListParams, run: &TranslateRequest, version: FhirVersion) -> String {
    let carried = run_pairs(run);
    let extra: Vec<(&str, &str)> = carried
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    params.address_with(CONCEPT_MAPS_PATH, version, &extra)
}

/// The search filter, as a form that navigates rather than reloading.
fn search_form(
    params: Signal<ListParams>,
    run: Signal<TranslateRequest>,
    version: Signal<FhirVersion>,
) -> AnyView {
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
        let target = run.with(|run| address(&searched, run, version.get()));
        // NOTE: the router resolves a navigation against its base, so an
        // address that already carries the base is passed unresolved
        // (`leptos_router` 0.8.15 `matching/resolve_path.rs`).
        navigate(&target, unresolved());
    };
    filter_form(
        "The url search parameter, matched against ConceptMap.url.",
        canonical,
        resource_version,
        params,
        Box::new(submit),
    )
}

/// The navigation options an address that already carries the base needs.
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
    run: Signal<TranslateRequest>,
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
        <section class="mt-8" aria-labelledby="conceptmaps-heading">
            <h2 id="conceptmaps-heading" class=styles::SECTION_TITLE>
                "What this root holds"
            </h2>
            <p aria-live="polite" class="mt-2 text-body text-muted">
                {announcement}
            </p>
            <Reading label="Reading the concept maps">
                {move || {
                    let params = params.get();
                    let run = run.get();
                    let version = version.get();
                    found
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(found) => list_view(found, &params, &run, version),
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
    found: &SearchSet<PublishedConceptMap>,
    params: &ListParams,
    run: &TranslateRequest,
    version: FhirVersion,
) -> AnyView {
    let resources = found.found();
    if resources.is_empty() {
        return view! {
            <p class="mt-3 text-body text-muted">
                "This root holds no ConceptMap resource matching the filter above. The runner below still works: a server may translate through a map it holds without publishing it as a resource."
            </p>
        }
        .into_any();
    }
    let view = window(params.page(), params.size(), resources.len());
    let rows: Vec<AnyView> = view
        .indexes()
        .filter_map(|index| resources.get(index))
        .map(|resource| row_view(resource, params, run, version))
        .collect();
    let carried = run_pairs(run);
    let extra: Vec<(&str, &str)> = carried
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let pager = pager_view(
        "Concept map pages",
        view,
        params,
        CONCEPT_MAPS_PATH,
        version,
        &extra,
    );
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left text-body">
                <thead>
                    <tr class="border-b border-line-strong">
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

/// One published concept map, as a row that opens it.
fn row_view(
    resource: &PublishedConceptMap,
    params: &ListParams,
    run: &TranslateRequest,
    version: FhirVersion,
) -> AnyView {
    let canonical = resource.url().unwrap_or(NOT_DECLARED).to_owned();
    let heading = canonical_cell(
        &canonical,
        resource
            .id()
            .map(|id| address(&params.reading(id), run, version)),
    );
    view! {
        <tr class="border-b border-line align-top">
            <th scope="row" class="py-2 pr-3 font-mono text-small font-normal break-all">
                {heading}
            </th>
            <td class="py-2 pr-3 font-mono text-small">
                {resource.version().unwrap_or(NOT_DECLARED).to_owned()}
            </td>
            <td class="py-2 pr-3">{resource.label().unwrap_or(NOT_DECLARED).to_owned()}</td>
            <td class="py-2">{resource.status().unwrap_or(NOT_DECLARED).to_owned()}</td>
        </tr>
    }
    .into_any()
}

/// One concept map read by its id, with the groups it maps.
fn detail_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<ListParams>,
    run: Signal<TranslateRequest>,
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
            <section class="mt-8" aria-labelledby="conceptmap-detail-heading">
                <h2 id="conceptmap-detail-heading" class=styles::SECTION_TITLE>
                    "The concept map you opened"
                </h2>
                <Reading label="Reading the concept map">
                    {move || {
                        let params = params.get();
                        let run = run.get();
                        let version = version.get();
                        resource
                            .with(|answered| {
                                answered
                                    .as_ref()
                                    .map(|answer| match answer {
                                        None => ().into_any(),
                                        Some(Ok(resource)) => {
                                            resource_view(resource, &params, &run, version)
                                        }
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

/// One resource: its facts, its groups, and the runner prefilled from it.
fn resource_view(
    resource: &PublishedConceptMap,
    params: &ListParams,
    run: &TranslateRequest,
    version: FhirVersion,
) -> AnyView {
    let rows: Vec<AnyView> = resource
        .facts()
        .into_iter()
        .map(|fact| {
            let value = fact.value.unwrap_or_else(|| NOT_DECLARED.to_owned());
            view! {
                <div class="grid gap-1 border-b border-line py-1 last:border-0 sm:grid-cols-[16rem_1fr]">
                    <dt class="font-medium">{fact.label}</dt>
                    <dd class="break-words">{value}</dd>
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
                <p class="mt-3 text-body text-muted">
                    "This resource declares no canonical, so there is no url to name it by in a run."
                </p>
            }
            .into_any()
        },
        |canonical| {
            let filled = TranslateRequest {
                concept_map: canonical.to_owned(),
                concept_map_version: resource.version().unwrap_or_default().to_owned(),
                system: resource.only_source_system().unwrap_or_default(),
                ..run.clone()
            };
            let href = address(params, &filled, version);
            view! {
                <p class="mt-3">
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
        <article class=format!("mt-3 panel-p {}", styles::PANEL)>
            <h3 class="font-mono text-body font-semibold break-all">{heading}</h3>
            {prefill}
            <dl class="mt-3 text-body">{rows}</dl>
            {groups}
        </article>
    }
    .into_any()
}

/// What the map maps, one group at a time.
fn groups_view(groups: &[GroupRow]) -> AnyView {
    if groups.is_empty() {
        return view! {
            <p class="mt-3 text-body text-muted">
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
                    <th scope="row" class="py-1 pr-3 font-mono text-small font-normal break-all">
                        {group.source.clone().unwrap_or_else(|| NOT_DECLARED.to_owned())}
                    </th>
                    <td class="py-1 pr-3 font-mono text-small break-all">
                        {group.target.clone().unwrap_or_else(|| NOT_DECLARED.to_owned())}
                    </td>
                    <td class="py-1 text-small">{group.elements.to_string()}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    view! {
        <div class="mt-4 overflow-x-auto">
            <table class="w-full border-collapse text-left text-body">
                <caption class="pb-1 text-left text-small font-medium tracking-wide uppercase">
                    "What the map maps"
                </caption>
                <thead>
                    <tr class="border-b border-line">
                        <th scope="col" class="py-1 pr-3 text-small font-medium">
                            "From"
                        </th>
                        <th scope="col" class="py-1 pr-3 text-small font-medium">
                            "To"
                        </th>
                        <th scope="col" class="py-1 text-small font-medium">
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

/// The `$translate` runner, gated on what this root declares it can do.
///
/// The affordance appears only where the `CapabilityStatement` declares
/// `$translate` on `ConceptMap`, which is where a server states the operations
/// it answers per resource type
/// (<https://hl7.org/fhir/R4B/capabilitystatement.html>), so the screen never
/// offers a run the root then refuses. The answer below it is drawn whenever a
/// run is in the address, because a link a reader was sent still has to say
/// what the server answered.
fn runner_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<ListParams>,
    run: Signal<TranslateRequest>,
) -> AnyView {
    let read_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.metadata_url(version.get()));
    let declared = Signal::derive(move || {
        statement.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .is_some_and(|statement| statement.declares_operation(CONCEPT_MAP, TRANSLATE))
        })
    });
    let answer = answer_section(client, version, run);

    view! {
        <section class="mt-8" aria-labelledby="translate-heading">
            <h2 id="translate-heading" class=styles::SECTION_TITLE>
                "Translate a code"
            </h2>
            <Reading label="Reading what this root can do">
                {move || {
                    statement
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(statement) => {
                                        if statement.declares_operation(CONCEPT_MAP, TRANSLATE) {
                                            ().into_any()
                                        } else {
                                            undeclared_view()
                                        }
                                    }
                                    Err(error) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <Show when=move || declared.get() fallback=|| ()>
                {runner_form(params, run, version)}
            </Show>
            <RequestDisclosure url />
            {answer}
        </section>
    }
    .into_any()
}

/// The statement that this root does not declare the operation.
fn undeclared_view() -> AnyView {
    view! {
        <p class=format!(
            "mt-3 rounded-md p-3 {}",
            styles::NOTICE,
        )>
            "This root's capability statement does not declare $translate on ConceptMap, so the runner is not offered here. Another FHIR version may declare it, so try the version switcher above."
        </p>
    }
    .into_any()
}

/// A control's own value, gated so an unrelated navigation cannot wipe an edit.
///
/// `prop:value` writes the DOM property on every notification with no equality
/// check (tachys 0.2.18, `html/property.rs`), so a control seeded from a derive
/// over the whole run loses what a reader is typing the moment any other
/// parameter moves. A `Memo` over the one field notifies only when that field
/// changes.
fn seeded(run: Signal<TranslateRequest>, read: fn(&TranslateRequest) -> String) -> Memo<String> {
    Memo::new(move |_| run.with(read))
}

/// The runner, as a form that navigates rather than reloading the page.
fn runner_form(
    params: Signal<ListParams>,
    run: Signal<TranslateRequest>,
    version: Signal<FhirVersion>,
) -> AnyView {
    let map: NodeRef<Input> = NodeRef::new();
    let map_version: NodeRef<Input> = NodeRef::new();
    let system: NodeRef<Input> = NodeRef::new();
    let system_version: NodeRef<Input> = NodeRef::new();
    let code: NodeRef<Input> = NodeRef::new();
    let target: NodeRef<Input> = NodeRef::new();
    let navigate = use_navigate();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let typed = TranslateRequest {
            concept_map: typed_value(map),
            concept_map_version: typed_value(map_version),
            system: typed_value(system),
            system_version: typed_value(system_version),
            code: typed_value(code),
            target_system: typed_value(target),
        };
        let target = params.with(|params| address(params, &typed, version.get()));
        navigate(&target, unresolved());
    };
    view! {
        <form class="mt-4 grid gap-5" on:submit=submit>
            {map_group(run, map, map_version)}
            {code_group(run, system, system_version, code)}
            {target_group(run, target)}
            <div class="flex flex-wrap items-center gap-2">
                <button type="submit" class=styles::SUBMIT>
                    <Icon glyph=icon::CONCEPT_MAPS />
                    "Translate"
                </button>
                {help_toggle()}
            </div>
        </form>
    }
    .into_any()
}

/// Which map the run reads, and which version of it.
fn map_group(
    run: Signal<TranslateRequest>,
    map: NodeRef<Input>,
    map_version: NodeRef<Input>,
) -> AnyView {
    group(
        "The map",
        vec![row(vec![
            text_field(
                Field {
                    id: "translate-map",
                    name: MAP_PARAM,
                    label: "Concept map canonical",
                    hint: "Sent as the url parameter. Left empty, the server picks the maps it holds for the code.",
                },
                map,
                seeded(run, |run| run.concept_map.clone()),
            ),
            text_field(
                Field {
                    id: "translate-map-version",
                    name: MAP_VERSION_PARAM,
                    label: "Concept map version",
                    hint: "Sent as conceptMapVersion.",
                },
                map_version,
                seeded(run, |run| run.concept_map_version.clone()),
            ),
        ])],
    )
}

/// The code being translated, and the system it is read in.
fn code_group(
    run: Signal<TranslateRequest>,
    system: NodeRef<Input>,
    system_version: NodeRef<Input>,
    code: NodeRef<Input>,
) -> AnyView {
    group(
        "The code",
        vec![
            row(vec![
                text_field(
                    Field {
                        id: "translate-system",
                        name: SYSTEM_PARAM,
                        label: "Code system",
                        hint: "The system the code belongs to. The operation requires one with a code.",
                    },
                    system,
                    seeded(run, |run| run.system.clone()),
                ),
                text_field(
                    Field {
                        id: "translate-system-version",
                        name: SYSTEM_VERSION_PARAM,
                        label: "Code system version",
                        hint: "Left empty, the server resolves the code against its default version.",
                    },
                    system_version,
                    seeded(run, |run| run.system_version.clone()),
                ),
            ]),
            text_field(
                Field {
                    id: "translate-code",
                    name: CODE_PARAM,
                    label: "Code",
                    hint: "The code to translate, sent exactly as you type it. The operation needs this and the system together.",
                },
                code,
                seeded(run, |run| run.code.clone()),
            ),
        ],
    )
}

/// Where the answer is allowed to land.
fn target_group(run: Signal<TranslateRequest>, target: NodeRef<Input>) -> AnyView {
    group(
        "The target",
        vec![text_field(
            Field {
                id: "translate-target",
                name: TARGET_PARAM,
                label: "Target code system",
                hint: "Narrows the answer to matches in this system. Left empty, every target the map reaches is returned.",
            },
            target,
            seeded(run, |run| run.target_system.clone()),
        )],
    )
}

/// The answer of one run: the matches, or the refusal that came instead.
///
/// The resource is created here, once, and the block below reads it. Nothing
/// is drawn until the address carries a code and its system, so a screen that
/// has not been asked to translate anything says nothing about a translation.
fn answer_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    run: Signal<TranslateRequest>,
) -> AnyView {
    let read_client = client.clone();
    let translated = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let request = run.get();
        async move {
            if request.runnable() {
                Some(client.translate(version, &request).await)
            } else {
                None
            }
        }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        run.with(|run| {
            if run.runnable() {
                url_client.translate_url(version.get(), run)
            } else {
                String::new()
            }
        })
    });

    // The live region is mounted the moment the address names a run, which is
    // before the read settles, so a screen reader hears the count when it
    // arrives rather than reading a sentence that was already there.
    let announcement = Memo::new(move |_| {
        translated.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .and_then(|result| result.as_ref().ok())
                .map(match_sentence)
                .unwrap_or_default()
        })
    });

    view! {
        <Show when=move || run.with(TranslateRequest::runnable) fallback=|| ()>
            {answer_block(translated, announcement, url)}
        </Show>
    }
    .into_any()
}

/// The heading, the live region, the answer, and the request that fetched it.
fn answer_block(
    translated: LocalResource<Option<Result<TranslateAnswer, FhirError>>>,
    announcement: Memo<String>,
    url: Signal<String>,
) -> AnyView {
    view! {
        <div class="mt-6">
            <h3 class="text-body font-medium">"The translation"</h3>
            <p aria-live="polite" class="mt-2 text-body text-muted">
                {move || announcement.get()}
            </p>
            <Reading label="Running the translation">
                {move || {
                    translated
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|answer| match answer {
                                    None => ().into_any(),
                                    Some(Ok(answer)) => answer_view(answer),
                                    Some(Err(error)) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        </div>
    }
    .into_any()
}

/// The result, the message, and one block per reported match.
fn answer_view(answer: &TranslateAnswer) -> AnyView {
    let result = match answer.result() {
        Some(true) => "The server translated the code.".to_owned(),
        Some(false) => "The server did not translate the code.".to_owned(),
        None => format!("Result {NOT_DECLARED}."),
    };
    let message = answer.message().map_or_else(
        || ().into_any(),
        |message| view! { <p class="mt-1 text-body">{message}</p> }.into_any(),
    );
    let used = used_view(&answer.used_maps());
    let matches = answer.matches();
    if matches.is_empty() {
        return view! {
            <p class="mt-3 text-body font-medium">{result}</p>
            {message}
            <p class="mt-2 text-body text-muted">"The server reported no match for this code."</p>
            {used}
        }
        .into_any();
    }
    // The blocks are a plain `Vec`, which rebuilds every position when the
    // next run settles. A `<For>` key it retained would be moved rather than
    // re-rendered, so a target code that repeats across two runs would keep
    // the block it had.
    let blocks: Vec<AnyView> = matches.iter().map(match_view).collect();
    view! {
        <p class="mt-3 text-body font-medium">{result}</p>
        {message}
        <div class="mt-3 grid gap-4">{blocks}</div>
        {used}
    }
    .into_any()
}

/// One match: how it relates, what it points at, and what came with it.
fn match_view(found: &TranslationMatch) -> AnyView {
    let target = found.target.as_ref().map_or_else(
        || {
            if found.no_map {
                "This code maps to nothing in the target".to_owned()
            } else {
                format!("Target {NOT_DECLARED}")
            }
        },
        Coding::rendered,
    );
    // The relation is labelled by the element that carried it: R4 and R4B
    // answer `equivalence` and R5 and the R6 ballot `relationship`, over two
    // different sets of codes.
    let relations: Vec<AnyView> = found
        .relations()
        .into_iter()
        .map(|relation| {
            view! {
                <li>
                    <span class="font-medium">{relation.label}</span>
                    ": "
                    <span class="font-mono">{relation.code}</span>
                </li>
            }
            .into_any()
        })
        .collect();
    let stated = if relations.is_empty() {
        view! {
            <p class=styles::LEAD>
                "The server stated no equivalence or relationship for this match."
            </p>
        }
        .into_any()
    } else {
        view! { <ul class="mt-1 text-body">{relations}</ul> }.into_any()
    };
    let facts: Vec<AnyView> = [
        (
            "From concept",
            found.source_concept.as_ref().map(Coding::rendered),
        ),
        ("From map", found.origin_map.clone()),
        ("Source comment", found.source_comment.clone()),
        ("Target comment", found.target_comment.clone()),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| (label, value)))
    .map(|(label, value)| {
        view! {
            <div class="grid gap-1 sm:grid-cols-[10rem_1fr]">
                <dt class="font-medium">{label}</dt>
                <dd class="font-mono break-all">{value}</dd>
            </div>
        }
        .into_any()
    })
    .collect();
    let products = values_view("Also produces", &found.products);
    let depends = values_view("Depends on", &found.depends_on);
    let properties = values_view("Properties", &found.properties);
    view! {
        <article class=format!("panel-p {}", styles::PANEL)>
            <h4 class="font-mono text-body font-semibold break-all">{target}</h4>
            {stated}
            <dl class="mt-2 text-small">{facts}</dl>
            {products}
            {depends}
            {properties}
        </article>
    }
    .into_any()
}

/// One list of named values a match carried, or nothing when it carried none.
fn values_view(label: &'static str, values: &[NamedValue]) -> AnyView {
    if values.is_empty() {
        return ().into_any();
    }
    let items: Vec<AnyView> = values
        .iter()
        .map(|value| {
            let rendered = value
                .value
                .clone()
                .unwrap_or_else(|| "a value shape this viewer draws no text for".to_owned());
            view! {
                <li class="break-all">
                    <span class="font-mono">{value.name.clone()}</span>
                    ": "
                    {rendered}
                </li>
            }
            .into_any()
        })
        .collect();
    view! {
        <h5 class="mt-3 text-small font-medium tracking-wide uppercase">{label}</h5>
        <ul class="mt-1 ml-4 list-disc text-small">{items}</ul>
    }
    .into_any()
}

/// The maps the server says it translated through.
fn used_view(used: &[String]) -> AnyView {
    if used.is_empty() {
        return ().into_any();
    }
    let items: Vec<AnyView> = used
        .iter()
        .map(|canonical| view! { <li class="break-all">{canonical.clone()}</li> }.into_any())
        .collect();
    view! {
        <h4 class="mt-4 text-small font-medium tracking-wide uppercase">
            "The maps the server used"
        </h4>
        <ul class="mt-1 ml-4 list-disc font-mono text-small">{items}</ul>
    }
    .into_any()
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

/// How many matches a translation reported, as a sentence.
fn match_sentence(answer: &TranslateAnswer) -> String {
    match answer.matches().len() {
        0 => "No matches.".to_owned(),
        1 => "1 match.".to_owned(),
        matches => format!("{matches} matches."),
    }
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
    fn a_run_reads_out_of_the_address_and_back_into_it() {
        let run = read_run(&query(&[
            ("map", "https://terminology.example/ConceptMap/m"),
            ("system", "https://terminology.example/a"),
            ("code", "x"),
        ]));
        assert!(run.runnable());
        let params = ListParams::read(&query(&[]), 25);
        assert_eq!(
            address(&params, &run, FhirVersion::R4B),
            "/ui/conceptmaps?fhir=r4b&map=https%3A%2F%2Fterminology.example%2FConceptMap%2Fm\
             &system=https%3A%2F%2Fterminology.example%2Fa&code=x",
            "a parameter the reader left empty is left out of the address"
        );
    }

    #[test]
    fn opening_a_map_keeps_the_run_the_reader_has_going() {
        let run = read_run(&query(&[("system", "https://x.example/a"), ("code", "x")]));
        let params = ListParams::read(&query(&[("page", "2")]), 25);
        let opened = address(&params.reading("m-1"), &run, FhirVersion::R5);
        assert!(opened.contains("id=m-1"), "{opened}");
        assert!(opened.contains("page=2"), "{opened}");
        assert!(opened.contains("code=x"), "{opened}");
    }

    #[test]
    fn a_run_naming_no_code_is_not_sent() {
        let run = read_run(&query(&[("system", "https://x.example/a")]));
        assert!(
            !run.runnable(),
            "the operation requires a code with its system"
        );
    }

    #[test]
    fn the_match_count_is_the_number_of_matches_the_server_reported() {
        let answer: TranslateAnswer = serde_json::from_str(
            r#"{"parameter":[{"name":"result","valueBoolean":true},
                {"name":"match","part":[{"name":"concept","valueCoding":{"code":"a"}}]},
                {"name":"match","part":[{"name":"concept","valueCoding":{"code":"b"}}]}]}"#,
        )
        .expect("the fixture is valid JSON");
        assert_eq!(match_sentence(&answer), "2 matches.");
    }
}
