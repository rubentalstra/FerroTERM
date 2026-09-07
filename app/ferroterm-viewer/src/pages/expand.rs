//! The expansion runner: `ValueSet/$expand`, one page at a time.

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
use crate::fhir::error::FhirError;
use crate::fhir::expansion::ACTIVE_ONLY_PARAMETER;
use crate::fhir::expansion::COUNT_PARAMETER;
use crate::fhir::expansion::ConceptRow;
use crate::fhir::expansion::DISPLAY_LANGUAGE_PARAMETER;
use crate::fhir::expansion::DesignationRow;
use crate::fhir::expansion::ExpandRequest;
use crate::fhir::expansion::ExpandedValueSet;
use crate::fhir::expansion::Expansion;
use crate::fhir::expansion::FILTER_PARAMETER;
use crate::fhir::expansion::INCLUDE_DESIGNATIONS_PARAMETER;
use crate::fhir::expansion::OFFSET_PARAMETER;
use crate::fhir::expansion::ParameterLine;
use crate::fhir::expansion::URL_PARAMETER;
use crate::fhir::expansion::Unclosed;
use crate::fhir::version::FhirVersion;
use crate::paging::MAX_COUNT;
use crate::paging::Page;
use crate::routes::UI_BASE;
use crate::routes::VERSION_PARAM;
use crate::settings::Settings;
use crate::settings::parse_page_size;
use crate::url::RequestUrl;

/// The runner's own path below the router base.
const RUNNER_PATH: &str = "expand";

/// The `IssueType` code a server refuses an oversized selection with.
///
/// "The operation was refused because the value set is too costly to expand"
/// (<https://hl7.org/fhir/R4B/valueset-issue-type.html>).
const TOO_COSTLY: &str = "too-costly";

/// The classes every text control on the form shares.
const CONTROL: &str = "w-full rounded border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-700 dark:bg-slate-900";

/// The classes a page control carries.
const PAGE_LINK: &str = "rounded border border-slate-300 px-2 py-1 text-sm text-brand-700 hover:underline dark:border-slate-700 dark:text-brand-300";

/// The classes a page control that leads nowhere carries.
const PAGE_END: &str = "rounded border border-slate-200 px-2 py-1 text-sm text-slate-500 dark:border-slate-800 dark:text-slate-400";

/// Runs `ValueSet/$expand` and walks the pages of its answer.
///
/// Every parameter is read from the address rather than from a private signal,
/// so a page is shareable and the back button walks the run. The read is under
/// `<Transition>`, because it refetches on every parameter and every page
/// (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn ExpandPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let settings = expect_context::<Settings>();

    // A submit is a navigation onto this same path, which `leptos_router`
    // answers by updating the query without re-running this body, so the
    // parameters are read reactively and never once at setup. Both reads are
    // memos: a resource refetches on a notification rather than on a change,
    // so an unrelated write to the stored page size would otherwise re-issue
    // an identical `$expand`.
    let query = use_query_map();
    let params: Signal<RunnerParams> = Memo::new(move |_| {
        query.with(|map| RunnerParams::read(&|name| map.get(name), settings.page_size.get()))
    })
    .into();
    let request: Signal<Option<ExpandRequest>> =
        Memo::new(move |_| params.with(RunnerParams::request)).into();

    let heading = view! {
        <Title text="Expansion runner" />
        <h1 class="text-2xl font-semibold">"Expansion runner"</h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "Expand a value set by its canonical, then walk the answer a page at a time. Every run below is one GET this server answers to any client."
        </p>
    }
    .into_any();

    let form = form_section(params, version);
    let results = result_section(&client, version, params, request);

    view! {
        {heading}
        {form}
        {results}
    }
}

/// The parameters the address carries, which are the ones the runner sends.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RunnerParams {
    /// The value set canonical, empty when the reader has typed none yet.
    url: String,
    /// The text filter.
    filter: String,
    /// The BCP 47 tag displays are asked for in.
    display_language: String,
    /// The page size, absent for a run that asks for no page at all.
    count: Option<u32>,
    /// A page size the address named and the viewer cannot ask for, kept as
    /// the reader wrote it so the screen can say it was not used.
    refused_count: Option<String>,
    /// Where the page starts.
    offset: u32,
    /// Whether inactive concepts are left out.
    active_only: bool,
    /// Whether every concept carries its designations.
    include_designations: bool,
}

/// One page of a result, and what the reader is told about it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pager {
    /// The page the address asked for.
    page: Page,
    /// `expansion.total`, absent when the server declared none.
    total: Option<u32>,
    /// How many concepts this page actually holds.
    rows: u32,
}

/// The chrome of one labelled control on the form.
#[derive(Clone, Copy, Debug)]
struct Field {
    /// The `id` the label points at.
    id: &'static str,
    /// The `name` the control carries.
    name: &'static str,
    /// The label a reader reads.
    label: &'static str,
    /// The sentence below the control.
    hint: &'static str,
}

impl RunnerParams {
    /// Reads the parameters out of the address.
    ///
    /// `query` looks one parameter up, which is what a `ParamsMap` does and
    /// what a test can do without a browser. An address naming no `count`
    /// takes the reader's stored page size; an empty one asks for the whole
    /// selection in a single answer; one the viewer cannot use falls back to
    /// the stored size and is kept, so the screen says it was not used rather
    /// than turning a typing mistake into an unpaged request.
    fn read(query: &dyn Fn(&str) -> Option<String>, stored_count: u32) -> Self {
        let text = |name: &str| query(name).unwrap_or_default().trim().to_owned();
        let flag = |name: &str| query(name).is_some_and(|value| value.trim() == "true");
        let stored = stored_count.clamp(1, MAX_COUNT);
        let (count, refused_count) = match query(COUNT_PARAMETER) {
            None => (Some(stored), None),
            Some(typed) if typed.trim().is_empty() => (None, None),
            Some(typed) => match parse_page_size(&typed) {
                Some(size) => (Some(size), None),
                None => (Some(stored), Some(typed)),
            },
        };
        Self {
            url: text(URL_PARAMETER),
            filter: text(FILTER_PARAMETER),
            display_language: text(DISPLAY_LANGUAGE_PARAMETER),
            count,
            refused_count,
            offset: query(OFFSET_PARAMETER)
                .and_then(|typed| typed.trim().parse::<u32>().ok())
                .unwrap_or_default(),
            active_only: flag(ACTIVE_ONLY_PARAMETER),
            include_designations: flag(INCLUDE_DESIGNATIONS_PARAMETER),
        }
    }

    /// The request these parameters make, or `None` when they name no value set.
    fn request(&self) -> Option<ExpandRequest> {
        if self.url.is_empty() {
            return None;
        }
        Some(ExpandRequest {
            url: self.url.clone(),
            filter: (!self.filter.is_empty()).then(|| self.filter.clone()),
            count: self.count,
            offset: (self.offset > 0).then_some(self.offset),
            display_language: (!self.display_language.is_empty())
                .then(|| self.display_language.clone()),
            active_only: self.active_only.then_some(true),
            include_designations: self.include_designations.then_some(true),
        })
    }

    /// The page on screen, or `None` for an unpaged run.
    ///
    /// `answered_offset` is `expansion.offset`, the offset the server says it
    /// applied (<https://hl7.org/fhir/R4B/valueset-operation-expand.html>). It
    /// wins over the address, because `<Transition>` keeps the previous rows
    /// while the next page loads and a summary that follows the address would
    /// describe rows that are not on screen yet.
    fn page(&self, answered_offset: Option<u32>) -> Option<Page> {
        let count = self.count?;
        Some(Page::at(answered_offset.unwrap_or(self.offset), count))
    }

    /// These parameters, moved onto `page`.
    fn on(&self, page: Page) -> Self {
        Self {
            count: Some(page.count()),
            refused_count: None,
            offset: page.offset(),
            ..self.clone()
        }
    }

    /// The viewer address these parameters are, for a link or a navigation.
    ///
    /// `count` is written even when it is empty, so a run the reader asked to
    /// go unpaged stays unpaged when the address is shared or revisited.
    // NOTE: every value stays in the query, which a click navigation carries
    // through untouched while it unescapes the path a second time
    // (`leptos_router` 0.8.15 `src/location/mod.rs`).
    fn address(&self, version: FhirVersion) -> String {
        let mut url = RequestUrl::new()
            .segment(UI_BASE.trim_start_matches('/'))
            .segment(RUNNER_PATH)
            .query(VERSION_PARAM, version.segment());
        if self.url.is_empty() {
            return url.render("");
        }
        url = url.query(URL_PARAMETER, &self.url);
        if !self.filter.is_empty() {
            url = url.query(FILTER_PARAMETER, &self.filter);
        }
        url = url.query(
            COUNT_PARAMETER,
            &match (&self.refused_count, self.count) {
                (Some(refused), _) => refused.clone(),
                (None, Some(count)) => count.to_string(),
                (None, None) => String::new(),
            },
        );
        if self.offset > 0 {
            url = url.query(OFFSET_PARAMETER, &self.offset.to_string());
        }
        if !self.display_language.is_empty() {
            url = url.query(DISPLAY_LANGUAGE_PARAMETER, &self.display_language);
        }
        if self.active_only {
            url = url.query(ACTIVE_ONLY_PARAMETER, "true");
        }
        if self.include_designations {
            url = url.query(INCLUDE_DESIGNATIONS_PARAMETER, "true");
        }
        url.render("")
    }
}

impl Pager {
    /// The first page, or `None` when this page is already it.
    fn first(self) -> Option<Page> {
        (self.page.offset() > 0).then(|| Page::first(self.page.count()))
    }

    /// The previous page, or `None` when this page starts the result.
    fn previous(self) -> Option<Page> {
        self.page.previous()
    }

    /// The next page, or `None` when there is nothing after this one.
    ///
    /// A server that declares no `total` says nothing about what follows, so a
    /// full page is the only evidence that more may exist and a short one ends
    /// the walk.
    fn next(self) -> Option<Page> {
        match self.total {
            Some(total) => self.page.next(total),
            None => (self.rows >= self.page.count()).then(|| {
                Page::at(
                    self.page.offset().saturating_add(self.page.count()),
                    self.page.count(),
                )
            }),
        }
    }

    /// The last page, or `None` when the end is not known or already here.
    fn last(self) -> Option<Page> {
        let last = self.page.last(self.total?);
        (last.offset() > self.page.offset()).then_some(last)
    }

    /// Which rows of the whole selection this page holds.
    fn summary(self) -> String {
        if self.rows == 0 {
            return match self.total {
                Some(total) => format!("No concepts on this page, of {total} in the selection."),
                None => "No concepts on this page.".to_owned(),
            };
        }
        let first = self.page.offset().saturating_add(1);
        let last = self.page.offset().saturating_add(self.rows);
        match self.total {
            Some(total) => format!("Concepts {first} to {last} of {total}."),
            None => format!("Concepts {first} to {last}. The server declared no total."),
        }
    }

    /// Where this page sits in the walk.
    fn position(self) -> String {
        match self.total {
            Some(total) => format!(
                "Page {} of {}",
                self.page.number(),
                self.page.total_pages(total)
            ),
            None => format!("Page {}", self.page.number()),
        }
    }
}

/// The parameters, as a form that navigates rather than reloading the page.
///
/// The router installs no `submit` listener, so the submit is handled here and
/// turned into a navigation. Each control is seeded from the address through
/// `prop:value`, which updates on a back navigation and leaves what the reader
/// is typing alone, and read back from the document when they submit.
fn form_section(params: Signal<RunnerParams>, version: Signal<FhirVersion>) -> AnyView {
    let canonical: NodeRef<Input> = NodeRef::new();
    let filter: NodeRef<Input> = NodeRef::new();
    let language: NodeRef<Input> = NodeRef::new();
    let count: NodeRef<Input> = NodeRef::new();
    let active_only: NodeRef<Input> = NodeRef::new();
    let designations: NodeRef<Input> = NodeRef::new();

    let navigate = use_navigate();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let (page_size, refused_count) = typed_page_size(count);
        let typed = RunnerParams {
            url: typed_value(canonical),
            filter: typed_value(filter),
            display_language: typed_value(language),
            count: page_size,
            refused_count,
            // A changed parameter selects a different result, so the walk
            // starts again at its first page rather than at an offset into
            // something else.
            offset: 0,
            active_only: typed_flag(active_only),
            include_designations: typed_flag(designations),
        };
        // NOTE: the router resolves a navigation against its base, so an
        // address that already carries the base is passed unresolved
        // (`leptos_router` 0.8.15 `matching/resolve_path.rs`).
        navigate(
            &typed.address(version.get()),
            NavigateOptions {
                resolve: false,
                ..NavigateOptions::default()
            },
        );
    };

    view! {
        <form class="mt-6 grid gap-4" on:submit=submit>
            {text_field(
                Field {
                    id: "expand-url",
                    name: "url",
                    label: "Value set canonical",
                    hint: "The url parameter of $expand. An implicit canonical carrying its own query string works: the runner encodes the whole value.",
                },
                canonical,
                Signal::derive(move || params.with(|params| params.url.clone())),
            )}
            <div class="grid gap-4 sm:grid-cols-2">
                {text_field(
                    Field {
                        id: "expand-filter",
                        name: "filter",
                        label: "Filter",
                        hint: "Text the server matches against the designations it holds.",
                    },
                    filter,
                    Signal::derive(move || params.with(|params| params.filter.clone())),
                )}
                {text_field(
                    Field {
                        id: "expand-display-language",
                        name: "displayLanguage",
                        label: "Display language",
                        hint: "A BCP 47 tag. Left empty, the server picks its own display.",
                    },
                    language,
                    Signal::derive(move || params.with(|params| params.display_language.clone())),
                )}
            </div>
            {count_field(count, params)}
            {check_field(
                Field {
                    id: "expand-active-only",
                    name: "activeOnly",
                    label: "Active concepts only",
                    hint: "Sends activeOnly=true, which asks the server to leave inactive concepts out of the selection.",
                },
                active_only,
                Signal::derive(move || params.with(|params| params.active_only)),
            )}
            {check_field(
                Field {
                    id: "expand-include-designations",
                    name: "includeDesignations",
                    label: "Include designations",
                    hint: "Sends includeDesignations=true, which asks for every designation the server holds for a listed concept.",
                },
                designations,
                Signal::derive(move || params.with(|params| params.include_designations)),
            )}
            <div>
                <button
                    type="submit"
                    class="rounded bg-brand-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-brand-600 dark:bg-brand-600 dark:hover:bg-brand-500"
                >
                    "Run the expansion"
                </button>
                <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
                    "Running puts these parameters in the address. The page controls below walk the run that is showing, so an edit you have not run yet is left behind."
                </p>
            </div>
        </form>
    }
    .into_any()
}

/// One labelled text control, seeded from the address.
fn text_field(field: Field, node: NodeRef<Input>, value: Signal<String>) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-1">
            <label for=field.id class="text-sm font-medium">
                {field.label}
            </label>
            <input
                id=field.id
                name=field.name
                type="text"
                class=CONTROL
                aria-describedby=described_by.clone()
                node_ref=node
                prop:value=move || value.get()
            />
            <p id=described_by class="text-xs text-slate-500 dark:text-slate-400">
                {field.hint}
            </p>
        </div>
    }
    .into_any()
}

/// The page size, which is what makes the answer walkable.
fn count_field(node: NodeRef<Input>, params: Signal<RunnerParams>) -> AnyView {
    let value = move || {
        params.with(|params| {
            params
                .count
                .map(|count| count.to_string())
                .unwrap_or_default()
        })
    };
    view! {
        <div class="grid gap-1 sm:max-w-xs">
            <label for="expand-count" class="text-sm font-medium">
                "Page size"
            </label>
            <input
                id="expand-count"
                name="count"
                type="number"
                min="1"
                max=MAX_COUNT.to_string()
                class=CONTROL
                aria-describedby="expand-count-note"
                node_ref=node
                prop:value=value
            />
            <p id="expand-count-note" class="text-xs text-slate-500 dark:text-slate-400">
                {move || {
                    params
                        .with(|params| match (&params.refused_count, params.count) {
                            (Some(refused), Some(used)) => {
                                format!(
                                    "Not used: the address asked for a page size of `{refused}`, which is not a whole number from 1 to {MAX_COUNT}. This run asked for {used}.",
                                )
                            }
                            _ => {
                                format!(
                                    "The count parameter, from 1 to {MAX_COUNT}. Left empty, the run asks for no page and the server may refuse a selection it considers too costly.",
                                )
                            }
                        })
                }}
            </p>
        </div>
    }
    .into_any()
}

/// One labelled checkbox, seeded from the address.
fn check_field(field: Field, node: NodeRef<Input>, checked: Signal<bool>) -> AnyView {
    let described_by = format!("{}-note", field.id);
    view! {
        <div class="grid gap-1">
            <div class="flex items-center gap-2">
                <input
                    id=field.id
                    name=field.name
                    type="checkbox"
                    class="size-4"
                    aria-describedby=described_by.clone()
                    node_ref=node
                    prop:checked=move || checked.get()
                />
                <label for=field.id class="text-sm font-medium">
                    {field.label}
                </label>
            </div>
            <p id=described_by class="text-xs text-slate-500 dark:text-slate-400">
                {field.hint}
            </p>
        </div>
    }
    .into_any()
}

/// The answer: the page, what it leaves out, and the request that fetched it.
fn result_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<RunnerParams>,
    request: Signal<Option<ExpandRequest>>,
) -> AnyView {
    let read_client = client.clone();
    let expansion = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let request = request.get();
        async move {
            match request {
                Some(request) => Some(client.expand(version, &request).await),
                None => None,
            }
        }
    });

    let url_client = client.clone();
    let url = Signal::derive(move || {
        request.with(|request| {
            request
                .as_ref()
                .map(|request| url_client.expand_url(version.get(), request))
        })
    });

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives.
    let announcement = Memo::new(move |_| {
        let params = params.get();
        expansion.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .and_then(|result| result.as_ref().ok())
                .and_then(ExpandedValueSet::expansion)
                .map(|expansion| count_sentence(&expansion, params.page(expansion.offset)))
                .unwrap_or_default()
        })
    });

    view! {
        <section class="mt-8" aria-labelledby="expansion-heading">
            <h2 id="expansion-heading" class="text-lg font-medium">
                "The expansion"
            </h2>
            <p aria-live="polite" class="mt-2 text-sm text-slate-600 dark:text-slate-300">
                {announcement}
            </p>
            <Show when=move || request.with(Option::is_none) fallback=|| ()>
                {invitation()}
            </Show>
            <Show when=move || request.with(Option::is_some) fallback=|| ()>
                <Transition fallback=|| {
                    view! { <Spinner label="Running the expansion" /> }
                }>
                    {move || {
                        let params = params.get();
                        let version = version.get();
                        expansion
                            .with(|answered| {
                                answered
                                    .as_ref()
                                    .map(|answer| match answer {
                                        None => ().into_any(),
                                        Some(Ok(value)) => expansion_view(value, &params, version),
                                        Some(Err(error)) => refusal_view(error),
                                    })
                            })
                    }}
                </Transition>
            </Show>
            <Show when=move || url.with(Option::is_some) fallback=|| ()>
                <RequestDisclosure url=Signal::derive(move || { url.get().unwrap_or_default() }) />
            </Show>
        </section>
    }
    .into_any()
}

/// What the screen says before a canonical has been typed.
fn invitation() -> AnyView {
    view! {
        <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
            "Name a value set above and run it. The canonical is sent exactly as you type it, so an implicit form a code system defines works here too."
        </p>
    }
    .into_any()
}

/// The page, the mark saying what it leaves out, and the echoed parameters.
fn expansion_view(
    value: &ExpandedValueSet,
    params: &RunnerParams,
    version: FhirVersion,
) -> AnyView {
    let Some(expansion) = value.expansion() else {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "The server answered a ValueSet carrying no expansion, so there is nothing to page through."
            </p>
        }
        .into_any();
    };
    let unclosed = expansion
        .unclosed
        .as_ref()
        .map_or_else(|| ().into_any(), unclosed_view);
    let table = concepts_table(&expansion.concepts);
    let pager = params.page(expansion.offset).map_or_else(
        || ().into_any(),
        |page| {
            pager_view(
                Pager {
                    page,
                    total: expansion.total,
                    rows: expansion.listed(),
                },
                params,
                version,
            )
        },
    );
    let echoed = parameters_view(&expansion.parameters);
    view! {
        {unclosed}
        {table}
        {pager}
        {echoed}
    }
    .into_any()
}

/// The statement that the value set admits codes this list does not carry.
fn unclosed_view(unclosed: &Unclosed) -> AnyView {
    let reasons: Vec<AnyView> = unclosed
        .reasons
        .iter()
        .map(|reason| view! { <li>{reason.clone()}</li> }.into_any())
        .collect();
    let stated = if reasons.is_empty() {
        view! { <p class="mt-1">"The server stated no reason for it."</p> }.into_any()
    } else {
        view! { <ul class="mt-1 ml-4 list-disc">{reasons}</ul> }.into_any()
    };
    view! {
        <div
            role="note"
            class="mt-3 rounded-md border border-amber-400 bg-amber-50 p-3 text-sm dark:border-amber-700 dark:bg-amber-950"
        >
            <p class="font-medium">"Unclosed expansion"</p>
            <p class="mt-1">
                "This value set admits codes the expansion does not list, so a code missing from the table below is not a code this server rejects."
            </p>
            {stated}
        </div>
    }
    .into_any()
}

/// The concepts of this page, as a table.
///
/// The rows are a whole-page replacement with no per-row state, so they are a
/// plain `Vec`, which rebuilds every position when the page changes. A `<For>`
/// key it retained would be moved rather than re-rendered, and a concept whose
/// code repeats on the next page would keep the row it had.
fn concepts_table(rows: &[ConceptRow]) -> AnyView {
    if rows.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This page holds no concepts. A page past the end of a selection is empty, and so is a filter nothing matches."
            </p>
        }
        .into_any();
    }
    let body: Vec<AnyView> = rows.iter().map(concept_row).collect();
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left text-sm">
                <thead>
                    <tr class="border-b border-slate-300 dark:border-slate-700">
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "Code"
                        </th>
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "Display"
                        </th>
                        <th scope="col" class="py-2 pr-3 font-medium">
                            "System"
                        </th>
                        <th scope="col" class="py-2 font-medium">
                            "Flags"
                        </th>
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// One concept, with its designations under the display it was listed by.
fn concept_row(row: &ConceptRow) -> AnyView {
    let nesting =
        (row.depth > 0).then(|| format!("nested at level {}", row.depth.saturating_add(1)));
    let system = match (row.system.as_str(), &row.version) {
        ("", _) => NOT_DECLARED.to_owned(),
        (system, Some(version)) => format!("{system}|{version}"),
        (system, None) => system.to_owned(),
    };
    view! {
        <tr class="border-b border-slate-200 align-top dark:border-slate-800">
            <td
                class="py-2 pr-3 font-mono break-all"
                style=format!("padding-left:{}rem", row.depth)
            >
                <span class="sr-only">{nesting}</span>
                {row.code.clone()}
            </td>
            <td class="py-2 pr-3">
                {row.display.clone().unwrap_or_else(|| NOT_DECLARED.to_owned())}
                {designation_list(&row.designations)}
            </td>
            <td class="py-2 pr-3 font-mono text-xs break-all">{system}</td>
            <td class="py-2">{flags(row)}</td>
        </tr>
    }
    .into_any()
}

/// The designations a concept was listed with.
fn designation_list(designations: &[DesignationRow]) -> AnyView {
    if designations.is_empty() {
        return ().into_any();
    }
    let lines: Vec<AnyView> = designations
        .iter()
        .map(|designation| view! { <li>{designation_line(designation)}</li> }.into_any())
        .collect();
    view! { <ul class="mt-1 ml-4 list-disc text-xs text-slate-600 dark:text-slate-300">{lines}</ul> }
    .into_any()
}

/// One designation, as the term and what the server said about it.
fn designation_line(designation: &DesignationRow) -> String {
    let about: Vec<String> = designation
        .language
        .iter()
        .chain(designation.usage.iter())
        .cloned()
        .collect();
    if about.is_empty() {
        designation.value.clone()
    } else {
        format!("{} ({})", designation.value, about.join(", "))
    }
}

/// The flags the server set on a concept, as words rather than as a colour.
fn flags(row: &ConceptRow) -> String {
    let mut set: Vec<&str> = Vec::new();
    if row.inactive {
        set.push("inactive");
    }
    if row.abstract_concept {
        set.push("abstract");
    }
    set.join(", ")
}

/// The page controls, which are the address of another page.
///
/// Each control is a link, so a page is shareable, the browser walks the run
/// with its back button, and the router turns the click into a navigation of
/// its own.
fn pager_view(pager: Pager, params: &RunnerParams, version: FhirVersion) -> AnyView {
    let step = |target: Option<Page>, label: &'static str| -> AnyView {
        match target {
            Some(page) => {
                let href = params.on(page).address(version);
                view! {
                    <a href=href class=PAGE_LINK>
                        {label}
                    </a>
                }
                .into_any()
            }
            // An unavailable control says so in words: a tint alone carries
            // no meaning (<https://www.w3.org/TR/WCAG22/#use-of-color>), and
            // a `<span>` has no role for `aria-disabled` to qualify.
            None => view! { <span class=PAGE_END>{label} <span class="sr-only">", unavailable"</span></span> }
            .into_any(),
        }
    };
    view! {
        <nav aria-label="Expansion pages" class="mt-3 flex flex-wrap items-center gap-2">
            {step(pager.first(), "First page")}
            {step(pager.previous(), "Previous page")}
            <p class="text-sm font-medium">{pager.position()}</p>
            {step(pager.next(), "Next page")}
            {step(pager.last(), "Last page")}
        </nav>
    }
    .into_any()
}

/// The parameters the server says it applied, echoed back verbatim.
fn parameters_view(lines: &[ParameterLine]) -> AnyView {
    if lines.is_empty() {
        return view! {
            <p class="mt-3 text-xs text-slate-500 dark:text-slate-400">
                "The server echoed no parameters with this expansion."
            </p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = lines
        .iter()
        .map(|line| {
            view! {
                <dt class="font-mono">{line.name.clone()}</dt>
                <dd class="break-all">{line.value.clone()}</dd>
            }
            .into_any()
        })
        .collect();
    view! {
        <details class="mt-4 rounded border border-slate-200 text-xs dark:border-slate-800">
            <summary class="cursor-pointer px-3 py-2 font-medium text-slate-700 dark:text-slate-200">
                "The parameters the server says it applied"
            </summary>
            <dl class="grid gap-1 border-t border-slate-200 px-3 py-2 sm:grid-cols-[14rem_1fr] dark:border-slate-800">
                {rows}
            </dl>
        </details>
    }
    .into_any()
}

/// A refusal, in the server's own words, with what to do about a costly one.
fn refusal_view(error: &FhirError) -> AnyView {
    let costly = error
        .outcome()
        .is_some_and(|outcome| outcome.carries_code(TOO_COSTLY));
    let advice = if costly {
        view! {
            <p class="mt-2 text-sm">
                "The server refused to expand a selection this large in one answer. Ask for a page size and the runner walks it a page at a time."
            </p>
        }
        .into_any()
    } else {
        ().into_any()
    };
    let error = error.clone();
    view! {
        <div class="mt-3">
            <Failure error=Signal::stored(error) />
            {advice}
        </div>
    }
    .into_any()
}

/// How much of the selection this answer holds, as a sentence.
fn count_sentence(expansion: &Expansion, page: Option<Page>) -> String {
    let rows = expansion.listed();
    match page {
        Some(page) => {
            let pager = Pager {
                page,
                total: expansion.total,
                rows,
            };
            format!("{} {}.", pager.summary(), pager.position())
        }
        None => match expansion.total {
            Some(total) if total != rows => {
                format!("{rows} concepts, of {total} in the selection.")
            }
            Some(_) | None => format!("{rows} concepts."),
        },
    }
}

/// The page size a control holds now, and the text it holds instead.
///
/// A typed size the viewer cannot ask for travels into the address as it was
/// typed, so the run is refused there and the screen says which value was not
/// used. Dropping it here would turn a typing mistake into an unpaged request.
fn typed_page_size(node: NodeRef<Input>) -> (Option<u32>, Option<String>) {
    let typed = typed_value(node);
    if typed.is_empty() {
        return (None, None);
    }
    match parse_page_size(&typed) {
        Some(size) => (Some(size), None),
        None => (None, Some(typed)),
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

/// Whether a checkbox is ticked now.
fn typed_flag(node: NodeRef<Input>) -> bool {
    node.get().is_some_and(|input| input.checked())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stored page size a reader who has changed nothing carries.
    const STORED: u32 = 50;

    /// A stand-in for the address, which reads the same way a `ParamsMap` does.
    fn map<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + use<'a> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn pager(offset: u32, count: u32, total: Option<u32>, rows: u32) -> Pager {
        Pager {
            page: Page::at(offset, count),
            total,
            rows,
        }
    }

    #[test]
    fn an_address_with_no_count_takes_the_stored_page_size() {
        let params = RunnerParams::read(&map(&[("url", "https://terminology.example/vs")]), STORED);
        assert_eq!(params.count, Some(STORED));
        assert_eq!(params.offset, 0, "a run starts at the first page");
    }

    #[test]
    fn an_empty_count_asks_for_no_page_at_all() {
        let params = RunnerParams::read(
            &map(&[("url", "https://terminology.example/vs"), ("count", "")]),
            STORED,
        );
        assert_eq!(
            params.count, None,
            "the reader asked for the whole selection, and the server answers or refuses"
        );
        assert_eq!(
            params.page(None),
            None,
            "an unpaged run has no page to walk"
        );
    }

    #[test]
    fn a_count_the_viewer_cannot_use_is_kept_and_said_rather_than_obeyed() {
        for typed in ["0", "1001", "twenty", "-5"] {
            let params = RunnerParams::read(&map(&[("count", typed)]), STORED);
            assert_eq!(
                (params.count, params.refused_count.as_deref()),
                (Some(STORED), Some(typed)),
                "`{typed}` names no page size the viewer asks for, so the run keeps paging and the screen says what was not used"
            );
        }
    }

    #[test]
    fn a_page_the_reader_walks_to_leaves_no_refusal_behind() {
        let params = RunnerParams::read(&map(&[("url", "x"), ("count", "twenty")]), STORED);
        let walked = params.on(Page::at(50, 25));
        assert_eq!(
            (walked.count, walked.refused_count),
            (Some(25), None),
            "the address the control points at carries a page size the viewer asked for"
        );
    }

    #[test]
    fn the_page_on_screen_is_the_one_the_server_answered() {
        let params = RunnerParams::read(
            &map(&[("url", "x"), ("count", "20"), ("offset", "40")]),
            STORED,
        );
        assert_eq!(
            params.page(Some(20)).map(Page::offset),
            Some(20),
            "the rows on screen are the answered page, which the address has already moved past"
        );
        assert_eq!(
            params.page(None).map(Page::offset),
            Some(40),
            "a server that declares no offset leaves the address as the only evidence"
        );
    }

    #[test]
    fn every_parameter_is_read_from_the_address() {
        let params = RunnerParams::read(
            &map(&[
                ("url", "https://terminology.example/vs"),
                ("filter", " fever "),
                ("count", "20"),
                ("offset", "40"),
                ("displayLanguage", "nl-NL"),
                ("activeOnly", "true"),
                ("includeDesignations", "true"),
            ]),
            STORED,
        );
        assert_eq!(
            params,
            RunnerParams {
                url: "https://terminology.example/vs".to_owned(),
                filter: "fever".to_owned(),
                display_language: "nl-NL".to_owned(),
                count: Some(20),
                refused_count: None,
                offset: 40,
                active_only: true,
                include_designations: true,
            },
            "the address is the whole state of the run"
        );
    }

    #[test]
    fn a_flag_is_set_only_by_the_value_that_names_it() {
        for typed in ["false", "1", "yes", ""] {
            assert!(
                !RunnerParams::read(&map(&[("activeOnly", typed)]), STORED).active_only,
                "`{typed}` does not turn a flag on"
            );
        }
    }

    #[test]
    fn an_unreadable_offset_starts_at_the_beginning() {
        assert_eq!(
            RunnerParams::read(&map(&[("offset", "halfway")]), STORED).offset,
            0,
            "an address a reader typed cannot wedge the walk"
        );
    }

    #[test]
    fn an_address_naming_no_value_set_makes_no_request() {
        assert_eq!(
            RunnerParams::read(&map(&[("filter", "fever")]), STORED).request(),
            None,
            "the screen invites a canonical rather than asking the server for nothing"
        );
    }

    #[test]
    fn only_the_parameters_the_reader_set_reach_the_request() {
        let params = RunnerParams::read(
            &map(&[("url", "https://terminology.example/vs"), ("count", "20")]),
            STORED,
        );
        assert_eq!(
            params.request(),
            Some(ExpandRequest {
                url: "https://terminology.example/vs".to_owned(),
                count: Some(20),
                ..ExpandRequest::default()
            }),
            "an unticked box sends nothing, so the server's own default applies"
        );
    }

    #[test]
    fn an_address_round_trips_through_the_reader() {
        let params = RunnerParams {
            url: "https://terminology.example/vs?all=true".to_owned(),
            filter: "a&b".to_owned(),
            display_language: "cy".to_owned(),
            count: Some(25),
            refused_count: None,
            offset: 50,
            active_only: true,
            include_designations: true,
        };
        let address = params.address(FhirVersion::R5);
        assert_eq!(
            address,
            "/ui/expand?fhir=r5&url=https%3A%2F%2Fterminology.example%2Fvs%3Fall%3Dtrue\
             &filter=a%26b&count=25&offset=50&displayLanguage=cy&activeOnly=true\
             &includeDesignations=true",
            "every value the reader typed is encoded into the parameter it belongs to"
        );
        // The router percent-decodes a query on read, so the parameters come
        // back as they were typed.
        assert_eq!(
            RunnerParams::read(
                &map(&[
                    ("url", "https://terminology.example/vs?all=true"),
                    ("filter", "a&b"),
                    ("count", "25"),
                    ("offset", "50"),
                    ("displayLanguage", "cy"),
                    ("activeOnly", "true"),
                    ("includeDesignations", "true"),
                ]),
                STORED,
            ),
            params,
            "the address the runner writes is the address it reads"
        );
    }

    #[test]
    fn an_unpaged_run_stays_unpaged_when_its_address_is_shared() {
        let params = RunnerParams {
            url: "https://terminology.example/vs".to_owned(),
            count: None,
            ..RunnerParams::default()
        };
        assert_eq!(
            params.address(FhirVersion::R4B),
            "/ui/expand?fhir=r4b&url=https%3A%2F%2Fterminology.example%2Fvs&count=",
            "the empty count is written, or a revisit would page a run that asked not to be"
        );
        assert_eq!(
            RunnerParams::read(&map(&[("url", "x"), ("count", "")]), STORED).count,
            None
        );
    }

    #[test]
    fn a_refused_page_size_reaches_the_address_and_is_reported_from_there() {
        let typed = RunnerParams {
            url: "https://terminology.example/vs".to_owned(),
            count: None,
            refused_count: Some("twenty".to_owned()),
            ..RunnerParams::default()
        };
        assert_eq!(
            typed.address(FhirVersion::R4B),
            "/ui/expand?fhir=r4b&url=https%3A%2F%2Fterminology.example%2Fvs&count=twenty",
            "the value the reader typed travels rather than being dropped on the way"
        );
        let read = RunnerParams::read(
            &map(&[
                ("url", "https://terminology.example/vs"),
                ("count", "twenty"),
            ]),
            STORED,
        );
        assert_eq!(
            (read.count, read.refused_count.as_deref()),
            (Some(STORED), Some("twenty")),
            "the run keeps paging, and the screen has what it needs to say why"
        );
    }

    #[test]
    fn an_address_with_no_value_set_carries_only_the_version() {
        assert_eq!(
            RunnerParams::default().address(FhirVersion::R4),
            "/ui/expand?fhir=r4",
            "an empty runner is a link worth sharing and nothing more"
        );
    }

    #[test]
    fn walking_a_page_keeps_every_other_parameter() {
        let params = RunnerParams {
            url: "https://terminology.example/vs".to_owned(),
            filter: "fever".to_owned(),
            count: Some(20),
            offset: 0,
            active_only: true,
            ..RunnerParams::default()
        };
        let walked = params.on(Page::at(20, 20));
        assert_eq!(walked.offset, 20);
        assert_eq!(
            walked.filter, "fever",
            "the page moved and nothing else did"
        );
        assert!(walked.active_only);
    }

    #[test]
    fn the_walk_stops_at_both_ends_of_a_counted_result() {
        let first = pager(0, 20, Some(45), 20);
        assert_eq!(first.first(), None, "this page is the first one");
        assert_eq!(first.previous(), None);
        assert_eq!(first.next().map(Page::offset), Some(20));
        assert_eq!(first.last().map(Page::offset), Some(40));

        let last = pager(40, 20, Some(45), 5);
        assert_eq!(last.next(), None, "45 rows end on the third page of 20");
        assert_eq!(last.last(), None, "this page is the last one");
        assert_eq!(last.previous().map(Page::offset), Some(20));
        assert_eq!(last.first().map(Page::offset), Some(0));
    }

    #[test]
    fn a_server_that_declares_no_total_walks_while_the_pages_are_full() {
        assert_eq!(
            pager(0, 20, None, 20).next().map(Page::offset),
            Some(20),
            "a full page may be followed by another"
        );
        assert_eq!(
            pager(20, 20, None, 7).next(),
            None,
            "a short page is the end of what there is"
        );
        assert_eq!(
            pager(20, 20, None, 20).last(),
            None,
            "the end is unknown, so no control claims to jump to it"
        );
    }

    #[test]
    fn the_summary_names_the_rows_of_the_whole_selection() {
        assert_eq!(
            pager(20, 20, Some(45), 20).summary(),
            "Concepts 21 to 40 of 45."
        );
        assert_eq!(
            pager(20, 20, None, 20).summary(),
            "Concepts 21 to 40. The server declared no total.",
            "an absent total is stated rather than invented"
        );
        assert_eq!(
            pager(60, 20, Some(45), 0).summary(),
            "No concepts on this page, of 45 in the selection.",
            "a page past the end says so"
        );
    }

    #[test]
    fn the_position_is_a_word_rather_than_a_highlight() {
        assert_eq!(pager(20, 20, Some(45), 20).position(), "Page 2 of 3");
        assert_eq!(
            pager(20, 20, None, 20).position(),
            "Page 2",
            "without a total there is no last page to count towards"
        );
    }

    #[test]
    fn the_announced_sentence_covers_a_paged_and_an_unpaged_run() {
        let expansion = Expansion {
            total: Some(45),
            concepts: vec![ConceptRow::default(); 20],
            ..Expansion::default()
        };
        assert_eq!(
            count_sentence(&expansion, Some(Page::at(20, 20))),
            "Concepts 21 to 40 of 45. Page 2 of 3."
        );
        assert_eq!(
            count_sentence(&expansion, None),
            "20 concepts, of 45 in the selection.",
            "an unpaged answer that is short of the total says both numbers"
        );
    }

    #[test]
    fn a_designation_states_what_the_server_said_about_it() {
        assert_eq!(
            designation_line(&DesignationRow {
                language: Some("nl".to_owned()),
                usage: Some("Preferred".to_owned()),
                value: "Koorts".to_owned(),
            }),
            "Koorts (nl, Preferred)"
        );
        assert_eq!(
            designation_line(&DesignationRow {
                value: "Fever".to_owned(),
                ..DesignationRow::default()
            }),
            "Fever",
            "a bare term is rendered bare rather than with empty brackets"
        );
    }

    #[test]
    fn the_flags_are_words() {
        assert_eq!(
            flags(&ConceptRow {
                inactive: true,
                abstract_concept: true,
                ..ConceptRow::default()
            }),
            "inactive, abstract",
            "no meaning on this screen is carried by colour alone"
        );
        assert_eq!(flags(&ConceptRow::default()), "");
    }
}
