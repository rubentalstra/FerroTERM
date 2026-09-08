//! The concept browser: search one code system, read a concept, walk its tree.
//!
//! What the screen offers is what the served version declares. The hierarchy
//! pane appears for a version whose capability statement declares a filter
//! operator that selects direct children, and a version that declares none
//! gets the flat searchable list with the reason stated. No code system is
//! named anywhere below.
//!
//! Every section is gated by a `Memo<bool>` and erased with `.into_any()`
//! rather than wrapped in a `<Show>`, because the component is generic over
//! its children and each use of it monomorphizes the whole subtree
//! (`.claude/rules/leptos-ui.md` §1, the bundle bar).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use leptos::ev::Event;
use leptos::ev::FocusEvent;
use leptos::ev::KeyboardEvent;
use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query_map;
use wasm_bindgen::JsCast;

use crate::components::NOT_DECLARED;
use crate::components::failure::Failure;
use crate::components::icon;
use crate::components::icon::Icon;
use crate::components::reading::Reading;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::components::spinner::Spinner;
use crate::fhir::FhirClient;
use crate::fhir::concept::CHILD_OF_OPERATOR;
use crate::fhir::concept::CODE_PARAMETER;
use crate::fhir::concept::ChildOf;
use crate::fhir::concept::Concept;
use crate::fhir::concept::ConceptQuery;
use crate::fhir::concept::Hierarchy;
use crate::fhir::concept::LookupRequest;
use crate::fhir::concept::chosen_version;
use crate::fhir::error::FhirError;
use crate::fhir::expansion::ConceptRow;
use crate::fhir::expansion::DISPLAY_LANGUAGE_PARAMETER;
use crate::fhir::expansion::ExpandedValueSet;
use crate::fhir::expansion::FILTER_PARAMETER;
use crate::fhir::terminology::TerminologyCapabilities;
use crate::fhir::terminology::VersionRow;
use crate::fhir::version::FhirVersion;
use crate::paging::MAX_COUNT;
use crate::routes::BROWSE_PATH;
use crate::routes::SYSTEM_PARAM;
use crate::routes::SYSTEM_VERSION_PARAM;
use crate::routes::UI_BASE;
use crate::routes::VERSION_PARAM;
use crate::routes::system_link;
use crate::routes::ui_link;
use crate::settings::Settings;
use crate::styles;
use crate::tree::TreeAction;
use crate::tree::TreeConcept;
use crate::tree::TreeRow;
use crate::tree::action;
use crate::tree::decode_open;
use crate::tree::encode_open;
use crate::tree::rows;
use crate::url::RequestUrl;

/// The address parameter carrying the concept the tree hangs from.
const ROOT_PARAM: &str = "root";

/// The address parameter carrying the open nodes of the tree.
const OPEN_PARAM: &str = "open";

/// The prefix of the element id every tree row carries.
const ROW_ID: &str = "browse-tree-";

/// The classes a sentence that states an absence carries.
const NOTE: &str = "mt-3 text-body text-muted";

/// The classes the twist that opens and closes a tree row carries.
///
/// The box is 24 by 24 CSS pixels around a smaller chevron, which is the
/// minimum target WCAG 2.2 SC 2.5.8 sets
/// (<https://www.w3.org/TR/WCAG22/#target-size-minimum>).
const TWIST: &str = "state-change mr-1 inline-flex h-6 w-6 items-center justify-center \
                     rounded text-accent hover:bg-inset";

/// The classes a table cell carries.
const CELL: &str = "py-1 pr-3 text-left text-small font-normal wrap-break-word";

/// How far one level of the tree is indented, in pixels.
///
/// The indent is written in whole pixels rather than in `rem`, because a
/// fractional value would pull the whole floating-point formatter into a
/// bundle that has no other use for it.
const INDENT: u32 = 20;

/// Browses one code system: search, one concept, and its hierarchy.
///
/// Every parameter is read from the address rather than from a private signal,
/// so a concept is shareable by link and the back button walks the reading.
/// The parameters are a memo, because a resource refetches on a notification
/// rather than on a change.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn BrowsePage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let settings = expect_context::<Settings>();
    let query = use_query_map();
    let params: Signal<BrowseParams> =
        Memo::new(move |_| query.with(|map| BrowseParams::read(&|name| map.get(name)))).into();
    let count: Memo<u32> = Memo::new(move |_| settings.page_size.get().clamp(1, MAX_COUNT));
    let named: Memo<bool> = Memo::new(move |_| params.with(|params| !params.system.is_empty()));

    let capability_client = client.clone();
    let capabilities = LocalResource::new(move || {
        let client = capability_client.clone();
        let version = version.get();
        async move { client.terminology_capabilities(version).await }
    });
    let declared: Memo<Declared> = Memo::new(move |_| {
        let row = capabilities.with(|answered| {
            let document = answered.as_ref()?.as_ref().ok()?;
            params.with(|params| {
                let card = document.card(&params.system)?;
                chosen_version(&card, Some(&params.system_version))
            })
        });
        Declared {
            hierarchy: row.as_ref().map(Hierarchy::of).unwrap_or_default(),
            languages: row
                .as_ref()
                .map(|row| row.languages.clone())
                .unwrap_or_default(),
            row,
        }
    });

    let heading = view! {
        <Title text="Concept browser" />
        <h1 class=styles::PAGE_TITLE>"Concept browser"</h1>
        <p class=styles::LEAD>
            "Search one code system, read a concept, and walk what the server declares of its hierarchy. Every pane below says which request it made."
        </p>
        {move || {
            (!named.get())
                .then(|| {
                    view! {
                        <p class=format!(
                            "mt-6 rounded-md p-3 {}",
                            styles::NOTICE,
                        )>
                            "This address names no code system. "
                            <a href=move || ui_link("", version.get()) class=styles::LINK>
                                "Pick one from the overview"
                            </a> ", open it, and follow its browse link."
                        </p>
                    }
                })
        }}
    }
    .into_any();

    let system = system_section(&client, version, params, capabilities, declared, named);
    let search = search_section(&client, version, params, count, named);
    let concept = concept_section(&client, version, params, declared, named);
    let tree = tree_section(&client, version, params, declared, count);

    view! {
        {heading}
        {system}
        {search}
        {concept}
        {tree}
    }
}

/// What the served version declares, as every section below reads it.
///
/// The three facts travel in one memo rather than three, because each memo
/// type instantiates its own reactive machinery in the bundle.
#[derive(Clone, Debug, Default, PartialEq)]
struct Declared {
    /// The version of the system this screen is reading.
    row: Option<VersionRow>,
    /// What it declares about walking its hierarchy.
    hierarchy: Hierarchy,
    /// The designation languages it holds.
    languages: Vec<String>,
}

/// The parameters the address carries, which are the ones the screen sends.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BrowseParams {
    /// The code system canonical, empty when the address names none.
    system: String,
    /// The code system version, empty for the server's own default.
    system_version: String,
    /// The text the search sends as `filter`.
    term: String,
    /// The BCP 47 tag the operations send as `displayLanguage`.
    language: String,
    /// The concept the detail pane reads.
    code: String,
    /// The concept the tree hangs from, which the detail pane's code seeds.
    root: String,
    /// The codes whose children are showing.
    open: BTreeSet<String>,
}

impl BrowseParams {
    /// Reads the parameters out of the address.
    ///
    /// `query` looks one parameter up, which is what a `ParamsMap` does and
    /// what a test can do without a browser.
    fn read(query: &dyn Fn(&str) -> Option<String>) -> Self {
        let text = |name: &str| query(name).unwrap_or_default().trim().to_owned();
        let code = text(CODE_PARAMETER);
        let named = text(ROOT_PARAM);
        Self {
            system: text(SYSTEM_PARAM),
            system_version: text(SYSTEM_VERSION_PARAM),
            term: text(FILTER_PARAMETER),
            language: text(DISPLAY_LANGUAGE_PARAMETER),
            root: if named.is_empty() {
                code.clone()
            } else {
                named
            },
            code,
            open: decode_open(&text(OPEN_PARAM)),
        }
    }

    /// The viewer address these parameters are, for a link or a navigation.
    // NOTE: every value stays in the query, which a click navigation carries
    // through untouched while it unescapes the path a second time
    // (`leptos_router` 0.8.15 `src/location/mod.rs`).
    fn address(&self, version: FhirVersion) -> String {
        let mut url = RequestUrl::new()
            .segment(UI_BASE.trim_start_matches('/'))
            .segment(BROWSE_PATH)
            .query(VERSION_PARAM, version.segment());
        if self.system.is_empty() {
            return url.render("");
        }
        url = url.query(SYSTEM_PARAM, &self.system);
        for (name, value) in [
            (SYSTEM_VERSION_PARAM, &self.system_version),
            (FILTER_PARAMETER, &self.term),
            (DISPLAY_LANGUAGE_PARAMETER, &self.language),
            (CODE_PARAMETER, &self.code),
        ] {
            if !value.is_empty() {
                url = url.query(name, value);
            }
        }
        if !self.root.is_empty() && self.root != self.code {
            url = url.query(ROOT_PARAM, &self.root);
        }
        if !self.open.is_empty() {
            url = url.query(OPEN_PARAM, &encode_open(&self.open));
        }
        url.render("")
    }

    /// These parameters, searching for `term`.
    ///
    /// The selection is kept: a new search does not un-read the concept the
    /// reader is looking at.
    fn searching(&self, term: &str) -> Self {
        Self {
            term: term.trim().to_owned(),
            ..self.clone()
        }
    }

    /// These parameters, asking the operations for displays in `language`.
    fn in_language(&self, language: &str) -> Self {
        Self {
            language: language.trim().to_owned(),
            ..self.clone()
        }
    }

    /// These parameters, reading `code` and hanging the tree from it.
    fn rooted_at(&self, code: &str) -> Self {
        Self {
            code: code.to_owned(),
            root: code.to_owned(),
            open: BTreeSet::new(),
            ..self.clone()
        }
    }

    /// These parameters, reading `code` without moving the tree.
    fn selecting(&self, code: &str) -> Self {
        Self {
            code: code.to_owned(),
            ..self.clone()
        }
    }

    /// These parameters, with the children of `code` showing or hidden.
    fn toggling(&self, code: &str, open: bool) -> Self {
        let mut showing = self.open.clone();
        if open {
            showing.insert(code.to_owned());
        } else {
            showing.remove(code);
        }
        Self {
            open: showing,
            ..self.clone()
        }
    }

    /// The version to pin the operations to, when the address names one.
    fn pinned(&self) -> Option<String> {
        (!self.system_version.is_empty()).then(|| self.system_version.clone())
    }

    /// The display language to ask for, when the address names one.
    fn wanted_language(&self) -> Option<String> {
        (!self.language.is_empty()).then(|| self.language.clone())
    }

    /// The search this address asks for, or `None` when it names no system.
    fn search(&self, count: u32) -> Option<ConceptQuery> {
        (!self.system.is_empty()).then(|| ConceptQuery {
            system: self.system.clone(),
            system_version: self.pinned(),
            filter: (!self.term.is_empty()).then(|| self.term.clone()),
            child_of: None,
            display_language: self.wanted_language(),
            count,
        })
    }

    /// The concept read this address asks for, or `None` when it names none.
    fn lookup(&self) -> Option<LookupRequest> {
        (!self.system.is_empty() && !self.code.is_empty()).then(|| LookupRequest {
            system: self.system.clone(),
            system_version: self.pinned(),
            code: self.code.clone(),
            display_language: self.wanted_language(),
        })
    }
}

/// The walk one drawing of the tree performs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ChildWalk {
    /// The canonical of the system being walked.
    system: String,
    /// The code system version, when the address names one.
    system_version: Option<String>,
    /// The BCP 47 tag displays are asked for in.
    language: Option<String>,
    /// How many children one level may hold.
    count: u32,
    /// The filter property the version declares the child operator on.
    property: String,
    /// The concept the tree hangs from.
    root: String,
    /// The codes whose children are showing.
    open: BTreeSet<String>,
}

impl ChildWalk {
    /// The selection that reads the direct children of `code`.
    fn query(&self, code: &str) -> ConceptQuery {
        ConceptQuery {
            system: self.system.clone(),
            system_version: self.system_version.clone(),
            filter: None,
            child_of: Some(ChildOf {
                property: self.property.clone(),
                code: code.to_owned(),
            }),
            display_language: self.language.clone(),
            count: self.count,
        }
    }
}

/// What this server declares it can do with the system being browsed.
fn system_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<BrowseParams>,
    capabilities: LocalResource<Result<TerminologyCapabilities, FhirError>>,
    declared: Memo<Declared>,
    named: Memo<bool>,
) -> AnyView {
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.terminology_metadata_url(version.get()));

    let body = move || {
        view! {
            <section class="mt-6" aria-labelledby="browse-system-heading">
                <h2 id="browse-system-heading" class=styles::SECTION_TITLE>
                    "The code system being browsed"
                </h2>
                <p class="mt-1 text-body break-all">
                    {move || {
                        params
                            .with(|params| {
                                let target = system_link(&params.system, version.get());
                                view! {
                                    <a href=target class=styles::LINK>
                                        {params.system.clone()}
                                    </a>
                                }
                            })
                    }}
                </p>
                <Reading label="Reading the terminology capabilities">
                    {move || {
                        capabilities
                            .with(|answered| {
                                answered
                                    .as_ref()
                                    .map(|result| match result {
                                        Ok(_) => {
                                            declared
                                                .with(|declared| match &declared.row {
                                                    Some(row) => version_view(row, &declared.hierarchy),
                                                    None => {
                                                        note(
                                                            "This root's terminology capabilities declare no such code system, or no such version of it. Another FHIR version may serve it, so try the version switcher above.",
                                                        )
                                                    }
                                                })
                                        }
                                        Err(error) => failure_view(error),
                                    })
                            })
                    }}
                </Reading>
                {language_view(params, declared, version)}
                <RequestDisclosure url />
            </section>
        }
        .into_any()
    };
    view! { {move || named.get().then(body)} }.into_any()
}

/// What the served version holds, and whether its hierarchy can be walked.
fn version_view(row: &VersionRow, hierarchy: &Hierarchy) -> AnyView {
    let code = row
        .code
        .clone()
        .unwrap_or_else(|| format!("Version {NOT_DECLARED}"));
    let walk = match hierarchy {
        Hierarchy::Walkable { property } => format!(
            "This version declares the {CHILD_OF_OPERATOR} operator on the {property} filter, so the tree below walks one level at a time."
        ),
        Hierarchy::WithoutChildren { operators } => format!(
            "This version declares the hierarchy operators {}, and none of them selects the direct children of a code, so the concepts read as a flat list.",
            operators.join(", ")
        ),
        Hierarchy::Absent => String::from(
            "This version declares no hierarchy filter operator, so its concepts are a flat list and there is no tree to walk.",
        ),
    };
    view! {
        <p class="mt-2 font-mono text-body break-all">{code}</p>
        <p class=styles::LEAD>{walk}</p>
    }
    .into_any()
}

/// The display language, offered as the version declares it.
fn language_view(
    params: Signal<BrowseParams>,
    declared: Memo<Declared>,
    version: Signal<FhirVersion>,
) -> AnyView {
    let navigate = StoredValue::new(use_navigate());
    let change = move |event: Event| {
        let chosen = event_target_value(&event);
        let target = params.with(|params| params.in_language(&chosen).address(version.get()));
        navigate.with_value(|navigate| go(navigate, &target));
    };
    let offered: Memo<Vec<String>> = Memo::new(move |_| {
        let mut offered = declared.with(|declared| declared.languages.clone());
        params.with(|params| {
            if !params.language.is_empty() && !offered.contains(&params.language) {
                offered.push(params.language.clone());
            }
        });
        offered
    });
    let options = move || {
        offered
            .get()
            .into_iter()
            .map(|tag| {
                let value = tag.clone();
                view! { <option value=value>{tag}</option> }.into_any()
            })
            .collect::<Vec<AnyView>>()
    };
    let picker = move || {
        view! {
            <div class="mt-3 grid max-w-xs gap-1">
                <label for="browse-language" class="text-body font-medium">
                    "Display language"
                </label>
                <select
                    id="browse-language"
                    name=DISPLAY_LANGUAGE_PARAMETER
                    class=styles::INPUT
                    aria-describedby="browse-language-note"
                    prop:value=move || params.with(|params| params.language.clone())
                    on:change=change
                >
                    <option value="">"The server's own choice"</option>
                    {options}
                </select>
                <p id="browse-language-note" class="text-small text-faint">
                    "The languages this version declares. The choice is sent as displayLanguage on every operation this screen calls."
                </p>
            </div>
        }
        .into_any()
    };
    view! {
        {move || {
            if offered.with(Vec::is_empty) {
                note(
                    "This version declares no designation language, so the server chooses the display and no language is sent.",
                )
            } else {
                picker()
            }
        }}
    }
    .into_any()
}

/// The search over the system, and the concepts it answers.
fn search_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<BrowseParams>,
    count: Memo<u32>,
    named: Memo<bool>,
) -> AnyView {
    let query: Memo<Option<ConceptQuery>> =
        Memo::new(move |_| params.with(|params| params.search(count.get())));
    let read_client = client.clone();
    let matches = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let query = query.get();
        async move {
            match query {
                Some(query) => Some(client.expand_inline(version, &query).await),
                None => None,
            }
        }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.expand_post_url(version.get()));
    let sent = Signal::derive(move || {
        query.with(|query| query.as_ref().map(ConceptQuery::body).unwrap_or_default())
    });

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives. It is
    // derived from the answer, never from the address, because `<Transition>`
    // keeps the previous list on screen while the next one loads.
    let announcement = Memo::new(move |_| {
        matches.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .and_then(|result| result.as_ref().ok())
                .map(|value| found_sentence(&concepts_of(value), count.get()))
                .unwrap_or_default()
        })
    });

    // The field is seeded from a memo over the term alone, because
    // `prop:value` writes the property on every notification with no equality
    // check (`tachys` 0.2.18 `html/property.rs`), so a memo over the whole
    // address would discard what the reader is typing when any other parameter
    // moves. A memo does not notify while its own value is unchanged.
    let term: Memo<String> = Memo::new(move |_| params.with(|params| params.term.clone()));
    let field: NodeRef<Input> = NodeRef::new();
    let navigate = StoredValue::new(use_navigate());
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let typed = field.get().map(|input| input.value()).unwrap_or_default();
        let target = params.with(|params| params.searching(&typed).address(version.get()));
        navigate.with_value(|navigate| go(navigate, &target));
    };

    let body = move || {
        view! {
            <section class="mt-8" aria-labelledby="browse-search-heading">
                <h2 id="browse-search-heading" class=styles::SECTION_TITLE>
                    "Search"
                </h2>
                <form class="mt-2 grid max-w-xl gap-1" on:submit=submit>
                    <label for="browse-filter" class="text-body font-medium">
                        "Filter"
                    </label>
                    <div class="flex gap-2">
                        <input
                            id="browse-filter"
                            name=FILTER_PARAMETER
                            type="search"
                            class=styles::INPUT
                            aria-describedby="browse-filter-note"
                            node_ref=field
                            prop:value=move || term.get()
                        />
                        <button type="submit" class=styles::SUBMIT>
                            <Icon glyph=icon::SEARCH />
                            "Search"
                        </button>
                    </div>
                    <p id="browse-filter-note" class="text-small text-faint">
                        "Sent as the filter parameter of $expand over a value set that selects this code system. An empty filter lists the first concepts the server answers."
                    </p>
                </form>
                <p aria-live="polite" class="mt-2 text-body text-muted">
                    {announcement}
                </p>
                <Reading label="Searching the code system">
                    {move || {
                        let params = params.get();
                        let version = version.get();
                        matches
                            .with(|answered| {
                                answered
                                    .as_ref()
                                    .map(|answer| match answer {
                                        None => ().into_any(),
                                        Some(Ok(value)) => {
                                            matches_view(&concepts_of(value), &params, version)
                                        }
                                        Some(Err(error)) => failure_view(error),
                                    })
                            })
                    }}
                </Reading>
                <RequestDisclosure url body=sent />
            </section>
        }
        .into_any()
    };
    view! { {move || named.get().then(body)} }.into_any()
}

/// How many concepts the answer carried, as a sentence.
fn found_sentence(concepts: &[ConceptRow], count: u32) -> String {
    let found = concepts.len();
    let whole = u32::try_from(found).unwrap_or(u32::MAX) >= count;
    match (found, whole) {
        (0, _) => String::from("No concept matched."),
        (1, _) => String::from("1 concept matched."),
        (found, false) => format!("{found} concepts matched."),
        (found, true) => format!(
            "{found} concepts, which is the whole page this screen asked for. Narrow the filter to see fewer."
        ),
    }
}

/// The concepts a search answered, as a list that reads each one.
fn matches_view(concepts: &[ConceptRow], params: &BrowseParams, version: FhirVersion) -> AnyView {
    if concepts.is_empty() {
        return note("No concept matched this filter in this code system.");
    }
    let items: Vec<AnyView> = concepts
        .iter()
        .map(|concept| {
            let target = params.rooted_at(&concept.code).address(version);
            let display = concept
                .display
                .clone()
                .unwrap_or_else(|| format!("Display {NOT_DECLARED}"));
            let code = concept.code.clone();
            let inactive = concept.inactive.then(|| String::from(" (inactive)"));
            view! {
                <li class="border-b border-line py-1 last:border-0">
                    <a href=target class=styles::LINK>
                        {display}
                    </a>
                    <span class="ml-2 font-mono text-small break-all text-faint">
                        {code} {inactive}
                    </span>
                </li>
            }
            .into_any()
        })
        .collect();
    view! { <ul class="mt-2 text-body">{items}</ul> }.into_any()
}

/// The concept the address names, as `$lookup` answers it.
fn concept_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<BrowseParams>,
    declared: Memo<Declared>,
    named: Memo<bool>,
) -> AnyView {
    let request: Memo<Option<LookupRequest>> =
        Memo::new(move |_| params.with(BrowseParams::lookup));
    let read_client = client.clone();
    let looked_up = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let request = request.get();
        async move {
            // The request travels back with its answer, because a resource
            // keeps its previous value while it refetches and `<Transition>`
            // keeps the previous view mounted, so a closure reading both the
            // address and the answer would pair the new code with the concept
            // that was read before it
            // (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
            match request {
                Some(request) => {
                    let answer = client.lookup(version, &request).await;
                    Some((request, answer))
                }
                None => None,
            }
        }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        request.with(|request| {
            request
                .as_ref()
                .map(|request| url_client.lookup_url(version.get(), request))
                .unwrap_or_default()
        })
    });
    let asked: Memo<bool> = Memo::new(move |_| request.with(Option::is_some));

    let answered = move || {
        view! {
            <Reading label="Reading the concept">
                {move || {
                    let version = version.get();
                    let walked = declared.with(|declared| declared.hierarchy.walk().is_some());
                    looked_up
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|answer| match answer {
                                    None => {
                                        // The pane is only rendered while the
                                        // address names a concept, so an answer
                                        // holding no request is one the reader has
                                        // already navigated away from.
                                        view! { <Spinner label="Reading the concept" /> }
                                            .into_any()
                                    }
                                    Some((asked, Ok(value))) => {
                                        params
                                            .with(|params| {
                                                concept_view(
                                                    &value.concept(),
                                                    &asked.code,
                                                    params,
                                                    version,
                                                    walked,
                                                )
                                            })
                                    }
                                    Some((_, Err(error))) => failure_view(error),
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        }
        .into_any()
    };
    let body = move || {
        view! {
            <section class="mt-8" aria-labelledby="browse-concept-heading">
                <h2 id="browse-concept-heading" class=styles::SECTION_TITLE>
                    "The concept"
                </h2>
                {move || {
                    if asked.get() {
                        answered()
                    } else {
                        note(
                            "Choose a concept from the search results to read its designations and properties.",
                        )
                    }
                }}
            </section>
        }
        .into_any()
    };
    view! { {move || named.get().then(body)} }.into_any()
}

/// One concept: its display, where it sits, and everything declared of it.
///
/// `code` is the code the answer was asked for rather than the one the address
/// carries now, so the code on screen always names the concept beside it.
/// `walked` says whether the tree below draws this concept's children. Where
/// it does not, and the server still answers the `child` property, the links
/// are drawn here, because a hierarchy the server declares is worth following
/// even when no filter operator can select a level of it.
fn concept_view(
    concept: &Concept,
    code: &str,
    params: &BrowseParams,
    version: FhirVersion,
    walked: bool,
) -> AnyView {
    let display = concept
        .display
        .clone()
        .unwrap_or_else(|| format!("Display {NOT_DECLARED}"));
    let code = code.to_owned();
    let held = concept.version.clone().map_or_else(
        || format!("Read from a version this server did not name, of the system {NOT_DECLARED}"),
        |version| format!("Read from version {version}"),
    );
    let parents = links_view(
        "Parents",
        "This server declares no parent for this concept.",
        &concept.parents(),
        params,
        version,
    );
    let children = (!walked).then(|| {
        links_view(
            "Children",
            "This server declares no child for this concept.",
            &concept.children(),
            params,
            version,
        )
    });
    let designations = table_view(
        "Designations",
        ["Term", "Language", "Use"],
        concept
            .designations
            .iter()
            .map(|designation| {
                [
                    designation.value.clone(),
                    designation.language.clone().unwrap_or_default(),
                    designation.usage.clone().unwrap_or_default(),
                ]
            })
            .collect(),
        "This server holds no designation for this concept.",
    );
    let properties = table_view(
        "Properties",
        ["Property", "Value", "Meaning"],
        concept
            .properties
            .iter()
            .map(|property| {
                [
                    property.code.clone(),
                    property.value.clone().unwrap_or_default(),
                    property.description.clone().unwrap_or_default(),
                ]
            })
            .collect(),
        "This server answers no property for this concept.",
    );
    view! {
        <h3 class="mt-2 text-body font-semibold">{display}</h3>
        <p class="font-mono text-small break-all text-faint">{code}</p>
        <p class="text-small text-faint">{held}</p>
        {parents}
        {children}
        {designations}
        {properties}
    }
    .into_any()
}

/// A set of related codes, as links that move the tree onto each one.
fn links_view(
    label: &'static str,
    absent: &'static str,
    codes: &[String],
    params: &BrowseParams,
    version: FhirVersion,
) -> AnyView {
    if codes.is_empty() {
        return note(absent);
    }
    let links: Vec<AnyView> = codes
        .iter()
        .map(|related| {
            let target = params.rooted_at(related).address(version);
            let code = related.clone();
            view! {
                <li>
                    <a href=target class="font-mono text-small ".to_owned() + styles::LINK>
                        {code}
                    </a>
                </li>
            }
            .into_any()
        })
        .collect();
    view! {
        <nav aria-label=format!("{label} of this concept") class="mt-3">
            <h4 class="text-small font-medium tracking-wide uppercase">{label}</h4>
            <ul class="mt-1 flex flex-wrap gap-2">{links}</ul>
        </nav>
    }
    .into_any()
}

/// One table of three columns, whose first column heads each row.
///
/// A cell the server left empty states its absence rather than reading as a
/// gap the screen failed to fill.
fn table_view(
    caption: &'static str,
    headers: [&'static str; 3],
    rows: Vec<[String; 3]>,
    absent: &'static str,
) -> AnyView {
    if rows.is_empty() {
        return note(absent);
    }
    let [first, second, third] = headers;
    let body: Vec<AnyView> = rows
        .into_iter()
        .map(|cells| {
            let [head, middle, tail] = cells.map(|cell| {
                if cell.is_empty() {
                    NOT_DECLARED.to_owned()
                } else {
                    cell
                }
            });
            view! {
                <tr class="border-b border-line align-top last:border-0">
                    <th scope="row" class=CELL>
                        {head}
                    </th>
                    <td class=CELL>{middle}</td>
                    <td class="py-1 text-small wrap-break-word text-faint">{tail}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left">
                <caption class="pb-1 text-left text-small font-medium tracking-wide uppercase">
                    {caption}
                </caption>
                <thead>
                    <tr class="border-b border-line">
                        <th scope="col" class=CELL>
                            {first}
                        </th>
                        <th scope="col" class=CELL>
                            {second}
                        </th>
                        <th scope="col" class=CELL>
                            {third}
                        </th>
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// The walk the address asks for, or `None` when this version declares none.
fn child_walk(
    params: Signal<BrowseParams>,
    declared: Memo<Declared>,
    count: Memo<u32>,
) -> Memo<Option<ChildWalk>> {
    Memo::new(move |_| {
        let property = declared.with(|declared| declared.hierarchy.walk().map(str::to_owned))?;
        params.with(|params| {
            (!params.system.is_empty() && !params.root.is_empty()).then(|| ChildWalk {
                system: params.system.clone(),
                system_version: params.pinned(),
                language: params.wanted_language(),
                count: count.get(),
                property,
                root: params.root.clone(),
                open: params.open.clone(),
            })
        })
    })
}

/// One level of the tree, and whether the server listed all of it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Level {
    /// The children this level drew.
    children: Vec<TreeConcept>,
    /// Whether the answer held every child the server counted.
    whole: bool,
}

/// One walk and the children it read, by the code they hang below.
///
/// The walk travels back with its answers, because a resource keeps its
/// previous value while it refetches and `<Transition>` keeps the previous
/// view mounted, so rows built from the address over the last answer would
/// claim the new anchor has no children before its level has been read
/// (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
type Levels = LocalResource<Option<(ChildWalk, Vec<(String, Result<Level, FhirError>)>)>>;

/// Issues the reads one drawing of the tree needs.
///
/// The walk reads the anchor's children first and then follows the open nodes
/// it finds, so a code left open in the address that no longer hangs below the
/// anchor costs no request.
fn child_levels(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    walk: Memo<Option<ChildWalk>>,
) -> Levels {
    let read_client = client.clone();
    LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let walk = walk.get();
        async move {
            let walk = walk?;
            let mut answers: Vec<(String, Result<Level, FhirError>)> = Vec::new();
            let mut frontier = vec![walk.root.clone()];
            let mut asked: BTreeSet<String> = BTreeSet::new();
            while let Some(code) = frontier.pop() {
                if !asked.insert(code.clone()) {
                    continue;
                }
                let answer = client
                    .expand_inline(version, &walk.query(&code))
                    .await
                    .map(|value| children_of(&value));
                if let Ok(level) = &answer {
                    frontier.extend(
                        level
                            .children
                            .iter()
                            .filter(|child| walk.open.contains(&child.code))
                            .map(|child| child.code.clone()),
                    );
                }
                answers.push((code, answer));
            }
            Some((walk, answers))
        }
    })
}

/// The hierarchy below the concept, one declared level at a time.
fn tree_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    params: Signal<BrowseParams>,
    declared: Memo<Declared>,
    count: Memo<u32>,
) -> AnyView {
    let walk = child_walk(params, declared, count);
    let levels = child_levels(client, version, walk);
    let walkable: Memo<bool> = Memo::new(move |_| walk.with(Option::is_some));

    // The rows come from the walk the answer belongs to, never from the
    // address, so a level that has not been read yet is never drawn as a
    // concept the server said has no children. `None` is that not-yet case:
    // the section is only rendered while a walk exists, so an answer holding
    // no walk belongs to an address the reader has already left.
    let drawn = move || {
        levels.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .map(|(walk, answers)| rows(&walk.root, &walk.open, &known_children(answers)))
        })
    };
    let refusals = move || {
        levels.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .map(|(_, answers)| {
                    answers
                        .iter()
                        .filter_map(|(_, answer)| answer.as_ref().err().cloned())
                        .collect::<Vec<FhirError>>()
                })
                .unwrap_or_default()
        })
    };
    // A level the server counted higher than it listed is a partial answer,
    // and the tree says so on the node it hangs below rather than drawing a
    // short list as if it were the whole one.
    let partial = move || {
        levels.with(|answered| {
            answered
                .as_ref()
                .and_then(Option::as_ref)
                .map(|(_, answers)| partial_levels(answers))
                .unwrap_or_default()
        })
    };

    let focused: RwSignal<String> = RwSignal::new(String::new());
    // The pattern's invariant is that the row carrying `tabindex="0"` is the
    // focused row, and a row is click-focusable at `tabindex="-1"`, so the tab
    // stop follows the browser's own focus rather than only the arrow keys
    // (<https://www.w3.org/WAI/ARIA/apg/patterns/treeview/>).
    let entered = move |event: FocusEvent| {
        let Some(key) = focused_row_key(&event) else {
            return;
        };
        if focused.with(|current| current != &key) {
            focused.set(key);
        }
    };
    let pressed = tree_keys(version, params, focused, drawn);

    let url_client = client.clone();
    let url = Signal::derive(move || url_client.expand_post_url(version.get()));
    let sent = Signal::derive(move || {
        walk.with(|walk| {
            walk.as_ref()
                .map(|walk| walk.query(&walk.root).body())
                .unwrap_or_default()
        })
    });

    let body = move || {
        view! {
            <section class="mt-8" aria-labelledby="browse-tree-heading">
                <h2 id="browse-tree-heading" class=styles::SECTION_TITLE>
                    "Below this concept"
                </h2>
                <p class=styles::LEAD>
                    "One level at a time, each level read with the child operator the version declares. The arrow keys walk it: right opens a concept, left closes it, and Enter reads the concept the tree is on."
                </p>
                // The rows are the only focusable part of the tree, so both a
                // key press and a focus move arrive here by bubbling from the
                // row that has focus.
                <div on:keydown=pressed on:focusin=entered>
                    <Reading label="Reading the children">
                        {move || {
                            let version = version.get();
                            let Some(rows) = drawn() else {
                                return view! { <Spinner label="Reading the children" /> }
                                    .into_any();
                            };
                            let stop = tab_stop(&rows, &focused.get());
                            let partial = partial();
                            params
                                .with(|params| {
                                    let selected = selected_row(&rows, &stop, &params.code);
                                    tree_view(
                                        &rows,
                                        &TreeChrome {
                                            stop: &stop,
                                            selected: &selected,
                                            partial: &partial,
                                            params,
                                            version,
                                        },
                                    )
                                })
                        }}
                        {move || { refusals().iter().map(failure_view).collect::<Vec<AnyView>>() }}
                    </Reading>
                </div>
                <RequestDisclosure url body=sent />
            </section>
        }
        .into_any()
    };
    view! { {move || walkable.get().then(body)} }.into_any()
}

/// What one drawing of the tree needs beyond the rows themselves.
///
/// The two keys travel in a struct rather than as adjacent string parameters,
/// because they are interchangeable at a call site and a swap would be silent.
struct TreeChrome<'a> {
    /// The key of the row holding the tree's one tab stop.
    stop: &'a str,
    /// The key of the row the tree announces as selected.
    selected: &'a str,
    /// The codes whose children the server did not list in full.
    partial: &'a BTreeSet<String>,
    /// The address every link on a row is built from.
    params: &'a BrowseParams,
    /// The FHIR version those links carry.
    version: FhirVersion,
}

/// The tree, as the ARIA tree view pattern draws it.
fn tree_view(rows: &[TreeRow], chrome: &TreeChrome) -> AnyView {
    if rows.is_empty() {
        return view! {
            <p id="browse-tree-empty" class=NOTE>
                "This server answers no child for this concept, so nothing hangs below it."
            </p>
        }
        .into_any();
    }
    let items: Vec<AnyView> = rows.iter().map(|row| row_view(row, chrome)).collect();
    view! {
        <ul role="tree" aria-label="The concepts below this one" class="mt-2 text-body">
            {items}
        </ul>
    }
    .into_any()
}

/// One row of the tree, with the state the pattern announces.
///
/// The rows are one flat list, so each carries its own level and set position,
/// which is what WAI-ARIA defines those attributes for
/// (<https://www.w3.org/TR/wai-aria-1.2/#aria-level>).
fn row_view(row: &TreeRow, chrome: &TreeChrome) -> AnyView {
    let params = chrome.params;
    let version = chrome.version;
    let selected = !chrome.selected.is_empty() && row.key == chrome.selected;
    let toggle = params
        .toggling(&row.concept.code, !row.open)
        .address(version);
    let select = params.selecting(&row.concept.code).address(version);
    let display = row
        .concept
        .display
        .clone()
        .unwrap_or_else(|| format!("Display {NOT_DECLARED}"));
    let code = row.concept.code.clone();
    let level = row.depth.saturating_add(1).to_string();
    // The row names itself, so the twist link and the truncation note stay out
    // of what a screen reader reads for it
    // (<https://www.w3.org/TR/accname-1.2/#computation-steps>).
    let name = format!("{display}, {code}");
    let short = (row.open && chrome.partial.contains(&row.concept.code)).then(|| {
        view! {
            <span class="ml-2 inline-flex items-center gap-1 text-small text-muted">
                <Icon glyph=icon::NOTICE class="h-3.5 w-3.5" />
                "This server counts more children here than this page size asked for."
            </span>
        }
        .into_any()
    });
    // The chevron is hidden and the word it replaced stays as `sr-only` text,
    // so the control keeps the accessible name it had and the row's own
    // `aria-expanded` remains the only announcement of the state.
    let twist = row.expandable.then(|| {
        let (glyph, mark) = if row.open {
            (icon::TWIST_OPEN, "Close")
        } else {
            (icon::TWIST_CLOSED, "Open")
        };
        view! {
            <a href=toggle tabindex="-1" class=TWIST>
                <Icon glyph=glyph class="h-3.5 w-3.5" />
                <span class="sr-only">{mark}</span>
            </a>
        }
        .into_any()
    });
    view! {
        <li
            role="treeitem"
            id=format!("{ROW_ID}{}", row.key)
            tabindex=if row.key == chrome.stop { "0" } else { "-1" }
            aria-label=name
            aria-posinset=row.position.to_string()
            aria-setsize=row.siblings.to_string()
            aria-expanded=row.expandable.then_some(if row.open { "true" } else { "false" })
            aria-selected=if selected { "true" } else { "false" }
            class="py-0.5"
            // The weight carries the selection as well as the tint, because a
            // reader who cannot tell the two backgrounds apart would otherwise
            // have no cue at all (WCAG 2.2 SC 1.4.1).
            class:font-semibold=selected
            class:bg-inset=selected
            style=format!("padding-left: {}px", row.depth.saturating_mul(INDENT))
        >
            {twist}
            <a href=select tabindex="-1" class=styles::LINK>
                {display}
            </a>
            <span class="ml-2 font-mono text-small break-all text-faint">{code}</span>
            {short}
        </li>
    }
    .attr("aria-level", level)
    .into_any()
}

/// The row that holds the tree's one tab stop.
///
/// A tab stop on a row that is no longer drawn would leave the tree
/// unreachable by keyboard, so it falls back to the first row.
fn tab_stop(rows: &[TreeRow], focused: &str) -> String {
    rows.iter()
        .find(|row| row.key == focused)
        .or_else(|| rows.first())
        .map(|row| row.key.clone())
        .unwrap_or_default()
}

/// The tree's key handler: what each key the pattern names does to the tree.
///
/// Opening, closing, and selecting are navigations, so a walk the reader made
/// is a link they can share; only moving the tab stop stays in the component,
/// because focus is not shareable state.
fn tree_keys(
    version: Signal<FhirVersion>,
    params: Signal<BrowseParams>,
    focused: RwSignal<String>,
    drawn: impl Fn() -> Option<Vec<TreeRow>> + Copy + 'static,
) -> impl Fn(KeyboardEvent) + Copy + 'static {
    let navigate = StoredValue::new(use_navigate());
    move |event: KeyboardEvent| {
        let rows = drawn().unwrap_or_default();
        let stop = tab_stop(&rows, &focused.get());
        let Some(action) = action(&rows, &stop, &event.key()) else {
            return;
        };
        event.prevent_default();
        let version = version.get();
        let walk_to = |target: String| navigate.with_value(|navigate| go(navigate, &target));
        match action {
            TreeAction::Focus(key) => {
                focused.set(key.clone());
                focus_row(&key);
            }
            TreeAction::Open(code) => {
                walk_to(params.with(|params| params.toggling(&code, true).address(version)));
            }
            TreeAction::Close(code) => {
                walk_to(params.with(|params| params.toggling(&code, false).address(version)));
            }
            TreeAction::Select(code) => {
                walk_to(params.with(|params| params.selecting(&code).address(version)));
            }
        }
    }
}

/// The one row the tree announces as selected.
///
/// A hierarchy may be a graph, so one code can name more than one row, and a
/// tree that sets no `aria-multiselectable` announces exactly one selected row
/// (<https://www.w3.org/TR/wai-aria-1.2/#aria-selected>). The row the reader
/// acted on holds the tab stop, so it wins; a link that arrives with no focus
/// yet takes the first row drawing the code, because the address carries the
/// concept rather than the path that reached it.
fn selected_row(rows: &[TreeRow], stop: &str, code: &str) -> String {
    if code.is_empty() {
        return String::new();
    }
    rows.iter()
        .find(|row| row.key == stop && row.concept.code == code)
        .or_else(|| rows.iter().find(|row| row.concept.code == code))
        .map(|row| row.key.clone())
        .unwrap_or_default()
}

/// The row a focus event landed on, when it landed on one.
fn focused_row_key(event: &FocusEvent) -> Option<String> {
    let id = event
        .target()?
        .dyn_into::<web_sys::Element>()
        .ok()
        .map(|element| element.id())?;
    id.strip_prefix(ROW_ID).map(str::to_owned)
}

/// Moves the browser's focus onto a tree row.
///
/// The tree keeps one tab stop, so walking it with the arrow keys has to move
/// the focus itself (<https://www.w3.org/WAI/ARIA/apg/patterns/treeview/>).
fn focus_row(key: &str) {
    let found = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&format!("{ROW_ID}{key}")));
    let Some(element) = found
        .as_ref()
        .and_then(|found| found.dyn_ref::<web_sys::HtmlElement>())
    else {
        return;
    };
    if element.focus().is_err() {
        leptos::logging::warn!("this browser refused to focus a tree row");
    }
}

/// Navigates to an address that already carries the router's base.
// NOTE: the router resolves a navigation against its base, so an address that
// already carries it is passed unresolved (`leptos_router` 0.8.15
// `matching/resolve_path.rs`).
fn go(navigate: &dyn Fn(&str, NavigateOptions), target: &str) {
    navigate(
        target,
        NavigateOptions {
            resolve: false,
            ..NavigateOptions::default()
        },
    );
}

/// A sentence stating what the server did not declare.
fn note(text: &'static str) -> AnyView {
    view! { <p class=NOTE>{text}</p> }.into_any()
}

/// The concepts an expansion listed, at every depth it answered at.
///
/// The request asks for a flat answer, so the depths are normally all zero.
/// A server that nests anyway still had every concept it listed match the
/// selection, and dropping the nested ones would hide a match from the reader.
fn concepts_of(value: &ExpandedValueSet) -> Vec<ConceptRow> {
    value
        .expansion()
        .map(|expansion| expansion.concepts)
        .unwrap_or_default()
}

/// The children an expansion listed, as the tree draws them.
fn children_of(value: &ExpandedValueSet) -> Level {
    let listed: Vec<TreeConcept> = concepts_of(value)
        .into_iter()
        .map(|concept| TreeConcept {
            code: concept.code,
            display: concept.display,
        })
        .collect();
    // `expansion.total` is "the total number of concepts in the expansion"
    // (<https://hl7.org/fhir/R4B/valueset-definitions.html#ValueSet.expansion.total>),
    // so a total above what arrived is the server saying this level holds more
    // than the page asked for. A server that sends no total has said nothing,
    // and the level is drawn as it arrived.
    let held = u32::try_from(listed.len()).unwrap_or(u32::MAX);
    let whole = value
        .expansion()
        .and_then(|expansion| expansion.total)
        .is_none_or(|total| total <= held);
    Level {
        children: listed,
        whole,
    }
}

/// The children that were read, by the code they hang below.
fn known_children(
    answers: &[(String, Result<Level, FhirError>)],
) -> BTreeMap<String, Vec<TreeConcept>> {
    answers
        .iter()
        .filter_map(|(code, answer)| {
            answer
                .as_ref()
                .ok()
                .map(|level| (code.clone(), level.children.clone()))
        })
        .collect()
}

/// The codes whose children the server said it did not list in full.
fn partial_levels(answers: &[(String, Result<Level, FhirError>)]) -> BTreeSet<String> {
    answers
        .iter()
        .filter(|(_, answer)| answer.as_ref().is_ok_and(|level| !level.whole))
        .map(|(code, _)| code.clone())
        .collect()
}

/// A refused read, in the server's own words.
fn failure_view(error: &FhirError) -> AnyView {
    let error = error.clone();
    view! {
        <div class="mt-2">
            <Failure error=Signal::stored(error) />
        </div>
    }
    .into_any()
}
#[cfg(test)]
mod tests {
    use super::*;

    fn read(pairs: &[(&str, &str)]) -> BrowseParams {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        BrowseParams::read(&|name| {
            owned
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        })
    }

    #[test]
    fn an_address_that_names_nothing_reads_as_no_system() {
        let params = read(&[]);
        assert_eq!(params, BrowseParams::default());
        assert_eq!(
            params.search(20),
            None,
            "with no system there is nothing to search"
        );
        assert_eq!(params.lookup(), None);
    }

    #[test]
    fn the_tree_hangs_from_the_concept_when_the_address_names_no_anchor() {
        let params = read(&[("system", "https://terminology.example/x"), ("code", "a")]);
        assert_eq!(
            params.root, "a",
            "a link that names only a concept still draws its tree"
        );
    }

    #[test]
    fn the_address_carries_every_piece_of_state_the_screen_holds() {
        let params = read(&[
            ("system", "https://terminology.example/x?edition=2031"),
            ("version", "2.0"),
            ("filter", "fever"),
            ("displayLanguage", "nl-NL"),
            ("code", "a"),
            ("root", "b"),
            ("open", "b,c"),
        ]);
        assert_eq!(
            params.address(FhirVersion::R4B),
            "/ui/browse?fhir=r4b&system=https%3A%2F%2Fterminology.example%2Fx%3Fedition%3D2031\
             &version=2.0&filter=fever&displayLanguage=nl-NL&code=a&root=b&open=b%2Cc",
            "a reader who copies the address gets what they were reading"
        );
        assert_eq!(
            read(&[
                ("system", "https://terminology.example/x?edition=2031"),
                ("version", "2.0"),
                ("filter", "fever"),
                ("displayLanguage", "nl-NL"),
                ("code", "a"),
                ("root", "b"),
                ("open", "b,c"),
            ]),
            params,
            "the address round trips"
        );
    }

    #[test]
    fn a_code_that_carries_the_list_separator_survives_the_address() {
        let params = read(&[("system", "https://terminology.example/x"), ("code", "a")])
            .toggling("m,s2", true)
            .toggling("kg", true);
        let address = params.address(FhirVersion::R4B);
        let written = address
            .split("open=")
            .nth(1)
            .expect("the address carries the open nodes");
        // The router decodes a query value once before a screen reads it, so
        // reading the address back models that one pass.
        let decoded = crate::url::decode_query_component(written);
        assert_eq!(
            read(&[
                ("system", "https://terminology.example/x"),
                ("code", "a"),
                ("open", &decoded),
            ])
            .open,
            params.open,
            "a code carrying a comma cannot split the list it travels in: {address}"
        );
    }

    #[test]
    fn an_anchor_that_is_the_concept_is_left_out_of_the_address() {
        let params = read(&[("system", "https://terminology.example/x"), ("code", "a")]);
        let address = params.address(FhirVersion::R5);
        assert!(
            !address.contains("root="),
            "the concept seeds the anchor, so naming it twice says nothing: {address}"
        );
    }

    #[test]
    fn choosing_a_concept_moves_the_tree_and_closes_what_was_open() {
        let params = read(&[
            ("system", "https://terminology.example/x"),
            ("code", "a"),
            ("open", "a,b"),
        ]);
        let chosen = params.rooted_at("c");
        assert_eq!((chosen.code.as_str(), chosen.root.as_str()), ("c", "c"));
        assert!(
            chosen.open.is_empty(),
            "the tree hangs from the new concept, so what was open below the old one is gone"
        );
    }

    #[test]
    fn choosing_inside_the_tree_leaves_the_tree_where_it_is() {
        let params = read(&[
            ("system", "https://terminology.example/x"),
            ("code", "a"),
            ("open", "a,b"),
        ]);
        let chosen = params.selecting("b");
        assert_eq!((chosen.code.as_str(), chosen.root.as_str()), ("b", "a"));
        assert_eq!(
            chosen.open, params.open,
            "reading a concept in the tree does not collapse the tree under the reader"
        );
    }

    #[test]
    fn opening_and_closing_a_node_moves_only_that_node() {
        let params = read(&[("system", "https://terminology.example/x"), ("code", "a")]);
        let opened = params.toggling("b", true);
        assert!(opened.open.contains("b"));
        assert_eq!(
            opened.toggling("b", false).open,
            params.open,
            "closing what was opened leaves the address as it was"
        );
    }

    #[test]
    fn a_search_sends_the_filter_and_the_pinned_version() {
        let params = read(&[
            ("system", "https://terminology.example/x"),
            ("version", "2.0"),
            ("filter", " fever "),
            ("displayLanguage", "nl"),
        ]);
        let query = params.search(25).expect("the address names a system");
        assert_eq!(
            (
                query.system.as_str(),
                query.system_version.as_deref(),
                query.filter.as_deref(),
                query.display_language.as_deref(),
                query.count
            ),
            (
                "https://terminology.example/x",
                Some("2.0"),
                Some("fever"),
                Some("nl"),
                25
            ),
            "surrounding space is trimmed off the term the reader typed"
        );
        assert_eq!(
            query.child_of, None,
            "a search selects the whole system, not one level of it"
        );
    }

    #[test]
    fn a_search_with_no_filter_still_asks_for_the_first_concepts() {
        let params = read(&[("system", "https://terminology.example/x")]);
        let query = params.search(10).expect("the address names a system");
        assert_eq!(
            query.filter, None,
            "an empty filter is left out, so the server's own default applies"
        );
    }

    #[test]
    fn a_concept_read_names_the_system_the_code_belongs_to() {
        let params = read(&[
            ("system", "https://terminology.example/x"),
            ("code", "a"),
            ("displayLanguage", "de"),
        ]);
        let request = params.lookup().expect("the address names a concept");
        assert_eq!(
            (
                request.system.as_str(),
                request.code.as_str(),
                request.display_language.as_deref()
            ),
            ("https://terminology.example/x", "a", Some("de"))
        );
    }

    #[test]
    fn a_level_of_the_tree_asks_for_the_children_of_one_code() {
        let walk = ChildWalk {
            system: String::from("https://terminology.example/x"),
            system_version: Some(String::from("2.0")),
            language: None,
            count: 50,
            property: String::from("concept"),
            root: String::from("a"),
            open: BTreeSet::new(),
        };
        let query = walk.query("b");
        assert_eq!(
            query.child_of,
            Some(ChildOf {
                property: String::from("concept"),
                code: String::from("b")
            }),
            "the walk names the property the version declared the operator on"
        );
        assert_eq!(query.filter, None, "a level is selected, not searched");
    }

    #[test]
    fn the_count_sentence_says_what_the_answer_held() {
        let concept = |code: &str| ConceptRow {
            code: code.to_owned(),
            ..ConceptRow::default()
        };
        assert_eq!(found_sentence(&[], 20), "No concept matched.");
        assert_eq!(found_sentence(&[concept("a")], 20), "1 concept matched.");
        assert_eq!(
            found_sentence(&[concept("a"), concept("b")], 20),
            "2 concepts matched."
        );
        assert!(
            found_sentence(&[concept("a"), concept("b")], 2).contains("Narrow the filter"),
            "a full page may be hiding matches, and the reader is told so"
        );
    }

    #[test]
    fn a_nested_match_stays_in_the_list_the_reader_sees() {
        let answer: ExpandedValueSet = serde_json::from_str(
            r#"{"resourceType":"ValueSet","expansion":{"contains":[
                {"code":"a","contains":[{"code":"b"}]}]}}"#,
        )
        .expect("the fixture is valid JSON");
        assert_eq!(
            concepts_of(&answer)
                .into_iter()
                .map(|concept| concept.code)
                .collect::<Vec<String>>(),
            vec![String::from("a"), String::from("b")],
            "every code a server listed matched the selection, however it nested them"
        );
    }

    #[test]
    fn a_level_the_server_counted_higher_than_it_listed_is_marked_partial() {
        let level = |json: &str| {
            let answer: ExpandedValueSet =
                serde_json::from_str(json).expect("the fixture is valid JSON");
            children_of(&answer)
        };
        assert!(
            !level(
                r#"{"resourceType":"ValueSet","expansion":{"total":5,"contains":[{"code":"a"}]}}"#
            )
            .whole,
            "a total above what arrived is the server saying the level holds more"
        );
        assert!(
            level(
                r#"{"resourceType":"ValueSet","expansion":{"total":1,"contains":[{"code":"a"}]}}"#
            )
            .whole,
            "a total the answer met is a whole level"
        );
        assert!(
            level(r#"{"resourceType":"ValueSet","expansion":{"contains":[{"code":"a"}]}}"#).whole,
            "a server that sent no total has said nothing, so the level is drawn as it arrived"
        );
    }

    #[test]
    fn one_concept_drawn_twice_is_announced_as_selected_once() {
        let row = |key: &str, code: &str| TreeRow {
            key: String::from(key),
            concept: TreeConcept {
                code: String::from(code),
                display: None,
            },
            ..TreeRow::default()
        };
        let rows = vec![row("a,c", "c"), row("b,c", "c"), row("b,d", "d")];
        assert_eq!(
            selected_row(&rows, "b,c", "c"),
            "b,c",
            "the row the reader acted on is the row the tree announces"
        );
        assert_eq!(
            selected_row(&rows, "b,d", "c"),
            "a,c",
            "a link that arrives with the focus elsewhere takes the first row drawing the code"
        );
        assert_eq!(
            selected_row(&rows, "a,c", ""),
            "",
            "an address that reads no concept selects no row"
        );
        assert_eq!(
            selected_row(&rows, "a,c", "gone"),
            "",
            "a concept no row draws selects no row"
        );
    }

    #[test]
    fn the_tab_stop_falls_back_to_the_first_row() {
        let rows = vec![
            TreeRow {
                key: String::from("a"),
                ..TreeRow::default()
            },
            TreeRow {
                key: String::from("b"),
                ..TreeRow::default()
            },
        ];
        assert_eq!(tab_stop(&rows, "b"), "b");
        assert_eq!(
            tab_stop(&rows, "gone"),
            "a",
            "a tree with no tab stop cannot be reached by keyboard"
        );
        assert_eq!(tab_stop(&[], "a"), "");
    }
}
