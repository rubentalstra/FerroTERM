//! One code system: what this server can do with it, and what it says it is.
//!
//! Two documents describe a code system and they answer different questions.
//! `TerminologyCapabilities` says what this server can do with it; the
//! published `CodeSystem` resource says what the code system is. Each pane
//! below names the document behind it, so a claim can be traced to the request
//! that produced it rather than blended into one table.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use thaw::Spinner;

use crate::components::NOT_DECLARED;
use crate::components::failure::Failure;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::code_system::PublishedCodeSystem;
use crate::fhir::error::FhirError;
use crate::fhir::terminology::SystemCard;
use crate::fhir::terminology::VersionRow;
use crate::fhir::version::FhirVersion;
use crate::routes::BROWSE_PATH;
use crate::routes::EXPAND_PATH;
use crate::routes::system_tool_link;

/// The path parameter this screen is addressed by.
#[derive(Clone, Debug, Params, PartialEq)]
struct SystemParams {
    /// The code system canonical, which the router percent-decodes on read.
    url: Option<String>,
}

/// Shows one code system, from the two documents that describe it.
///
/// The canonical is read reactively. A link from one system to another matches
/// this same `<Route>`, and `leptos_router` then updates the params signal
/// without re-running this body, so a read taken untracked at setup would go
/// stale (`.claude/rules/leptos-ui.md` §8).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn CodeSystemPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let params = use_params::<SystemParams>();
    // NOTE: an address carrying no canonical is a link a reader typed, so it
    // reads as the empty system and every section states what it found.
    let system = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|params| params.url.clone())
            .unwrap_or_default()
    });

    let title = move || system.with(|system| title_of(system));
    let heading = view! {
        <Title text=title />
        <h1 class="font-mono text-xl font-semibold break-all">
            {move || system.with(|system| heading_of(system))}
        </h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "One code system, from the two documents that describe it. Each pane below says which request it read."
        </p>
    }
    .into_any();

    let capability = capability_section(&client, version, system);
    let published = published_section(&client, version, system);

    view! {
        {heading}
        {capability}
        {published}
    }
}

/// The document title for one canonical.
fn title_of(system: &str) -> String {
    if system.is_empty() {
        "Code system".to_owned()
    } else {
        system.to_owned()
    }
}

/// The heading for one canonical.
fn heading_of(system: &str) -> String {
    if system.is_empty() {
        "This address names no code system".to_owned()
    } else {
        system.to_owned()
    }
}

/// What this server declares it can do with the system.
fn capability_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    system: Signal<String>,
) -> AnyView {
    let read_client = client.clone();
    let capabilities = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        async move { client.terminology_capabilities(version).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.terminology_metadata_url(version.get()));

    view! {
        <section class="mt-6" aria-labelledby="system-capability-heading">
            <h2 id="system-capability-heading" class="text-lg font-medium">
                "What this server can do with it"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "Read from this root's terminology capabilities. Every affordance on the other screens is gated on what this pane shows."
            </p>
            <Transition fallback=|| {
                view! { <Spinner label="Reading the terminology capabilities" /> }
            }>
                {move || {
                    capabilities
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(document) => {
                                        let found = system.with(|system| document.card(system));
                                        match found {
                                            Some(card) => support_view(card, version.get()),
                                            None => undeclared_view(),
                                        }
                                    }
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

/// The statement that this root's capabilities do not name the system.
fn undeclared_view() -> AnyView {
    view! {
        <p class="mt-3 rounded-md border border-amber-300 bg-amber-50 p-3 text-sm dark:border-amber-800 dark:bg-amber-950">
            "This root's terminology capabilities do not name this code system. Another FHIR version may serve it, so try the version switcher above; otherwise this deployment has not loaded it."
        </p>
    }
    .into_any()
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

/// The system-level facts, then one block per served version.
///
/// The blocks are a plain `Vec`, which rebuilds every position when the read
/// settles again. A `<For>` would be wrong: a key it retains is moved rather
/// than re-rendered, so switching FHIR version or code system would keep every
/// block's old body (verified in `leptos` 0.8.20 `for_loop.rs` and `tachys`
/// 0.2.18 `view/keyed.rs`).
fn support_view(card: SystemCard, version: FhirVersion) -> AnyView {
    let content = card.content.map_or_else(
        || format!("Content {NOT_DECLARED} at this FHIR version"),
        |mode| format!("Content: {mode}"),
    );
    let subsumption = match card.subsumption {
        Some(true) => "Subsumption supported".to_owned(),
        Some(false) => "Subsumption not supported".to_owned(),
        None => format!("Subsumption {NOT_DECLARED}"),
    };
    let badges = view! {
        <ul class="mt-3 flex flex-wrap gap-2 text-xs">
            <li class="rounded bg-slate-100 px-2 py-1 text-slate-800 dark:bg-slate-800 dark:text-slate-100">
                {content}
            </li>
            <li class="rounded bg-slate-100 px-2 py-1 text-slate-800 dark:bg-slate-800 dark:text-slate-100">
                {subsumption}
            </li>
        </ul>
    }
    .into_any();

    if card.versions.is_empty() {
        let links = tools_view(&card.url, None, version);
        return view! {
            {badges}
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This server declares no version for this code system."
            </p>
            {links}
        }
        .into_any();
    }
    let blocks: Vec<AnyView> = card
        .versions
        .iter()
        .map(|row| version_view(&card.url, row, version))
        .collect();
    view! {
        {badges}
        <div class="mt-3 grid gap-4">{blocks}</div>
    }
    .into_any()
}

/// One served version: what it holds, what it filters on, what it answers.
fn version_view(system: &str, row: &VersionRow, version: FhirVersion) -> AnyView {
    let code = row
        .code
        .clone()
        .unwrap_or_else(|| format!("Version {NOT_DECLARED}"));
    let default = if row.is_default {
        "An unversioned request resolves to this version"
    } else {
        "An unversioned request resolves to another version"
    };
    let compositional = match row.compositional {
        Some(true) => "The server reads this system's compositional grammar",
        Some(false) => "The server does not read a compositional grammar here",
        None => "Compositional grammar not declared",
    };
    let languages = list_view(
        "Designation languages",
        &row.languages,
        "This version declares no designation language.",
    );
    let properties = list_view(
        "Properties $lookup answers",
        &row.properties,
        "This version declares no $lookup property.",
    );
    let filters = filter_view(row);
    let links = tools_view(system, row.code.as_deref(), version);
    view! {
        <article class="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900">
            <h3 class="font-mono text-sm font-semibold break-all">{code}</h3>
            <p class="mt-1 text-xs text-slate-600 dark:text-slate-300">{default}</p>
            <p class="text-xs text-slate-600 dark:text-slate-300">{compositional}</p>
            {languages}
            {properties}
            {filters}
            {links}
        </article>
    }
    .into_any()
}

/// One declared list, or the statement that the version declared none.
fn list_view(label: &'static str, values: &[String], absent: &'static str) -> AnyView {
    if values.is_empty() {
        return view! {
            <h4 class="mt-3 text-xs font-medium tracking-wide uppercase">{label}</h4>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">{absent}</p>
        }
        .into_any();
    }
    let items: Vec<AnyView> = values
        .iter()
        .map(|value| {
            view! {
                <li class="rounded bg-slate-100 px-2 py-1 font-mono dark:bg-slate-800">
                    {value.clone()}
                </li>
            }
            .into_any()
        })
        .collect();
    view! {
        <h4 class="mt-3 text-xs font-medium tracking-wide uppercase">{label}</h4>
        <ul class="mt-1 flex flex-wrap gap-1 text-xs">{items}</ul>
    }
    .into_any()
}

/// The filters `$expand` accepts for one version, with their operators.
fn filter_view(row: &VersionRow) -> AnyView {
    if row.filters.is_empty() {
        return view! {
            <h4 class="mt-3 text-xs font-medium tracking-wide uppercase">
                "Filters $expand accepts"
            </h4>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "This version declares no filter."
            </p>
        }
        .into_any();
    }
    let rows: Vec<AnyView> = row
        .filters
        .iter()
        .map(|filter| {
            let code = if filter.code.is_empty() {
                format!("Filter {NOT_DECLARED}")
            } else {
                filter.code.clone()
            };
            let operators = if filter.operators.is_empty() {
                "No operator declared".to_owned()
            } else {
                filter.operators.join(", ")
            };
            view! {
                <tr class="border-b border-slate-100 align-top last:border-0 dark:border-slate-800">
                    <th scope="row" class="py-1 pr-3 font-mono text-xs font-normal break-all">
                        {code}
                    </th>
                    <td class="py-1 font-mono text-xs break-all">{operators}</td>
                </tr>
            }
            .into_any()
        })
        .collect();
    view! {
        <div class="mt-3 overflow-x-auto">
            <table class="w-full border-collapse text-left text-sm">
                <caption class="pb-1 text-left text-xs font-medium tracking-wide uppercase">
                    "Filters $expand accepts"
                </caption>
                <thead>
                    <tr class="border-b border-slate-200 dark:border-slate-700">
                        <th scope="col" class="py-1 pr-3 text-xs font-medium">
                            "Property"
                        </th>
                        <th scope="col" class="py-1 text-xs font-medium">
                            "Operators"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
    .into_any()
}

/// The screens that work over this system, carrying it and its version.
fn tools_view(system: &str, code: Option<&str>, version: FhirVersion) -> AnyView {
    let browse = system_tool_link(BROWSE_PATH, system, code, version);
    let expand = system_tool_link(EXPAND_PATH, system, code, version);
    let named = code.map_or_else(
        || "this code system".to_owned(),
        |code| format!("version {code}"),
    );
    view! {
        <nav aria-label=format!("Screens for {named}") class="mt-3 flex flex-wrap gap-3 text-sm">
            <a href=browse class="text-brand-700 underline dark:text-brand-300">
                "Browse the concepts"
            </a>
            <a href=expand class="text-brand-700 underline dark:text-brand-300">
                "Run an expansion"
            </a>
        </nav>
    }
    .into_any()
}

/// What the code system itself says it is, from the published resource.
fn published_section(
    client: &FhirClient,
    version: Signal<FhirVersion>,
    system: Signal<String>,
) -> AnyView {
    let read_client = client.clone();
    let search = LocalResource::new(move || {
        let client = read_client.clone();
        let version = version.get();
        let system = system.get();
        async move { client.code_system_search(version, &system).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || {
        system.with(|system| url_client.code_system_search_url(version.get(), system))
    });

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives. It stays
    // silent on a refusal, which the failure's own `role="alert"` carries.
    let announcement = Memo::new(move |_| {
        search.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|search| match_sentence(search.matched(), search.total()))
                .unwrap_or_default()
        })
    });

    view! {
        <section class="mt-8" aria-labelledby="system-published-heading">
            <h2 id="system-published-heading" class="text-lg font-medium">
                "What the code system says it is"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "Read from the published CodeSystem resources this root holds for the canonical above. This pane describes the code system; the pane above describes this server."
            </p>
            <p aria-live="polite" class="mt-2 text-sm text-slate-600 dark:text-slate-300">
                {announcement}
            </p>
            <Transition fallback=|| {
                view! { <Spinner label="Reading the published CodeSystem" /> }
            }>
                {move || {
                    search
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(search) => resources_view(&search.published()),
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

/// How many published resources answered, as a sentence.
///
/// The count the server declared is shown beside the number of resources the
/// answer actually carried, because a `Bundle.total` larger than the entries
/// means the search was paged.
fn match_sentence(drawn: usize, total: Option<u32>) -> String {
    let counted = total.map_or_else(
        || format!("this root counted {NOT_DECLARED}"),
        |total| format!("this root counted {total}"),
    );
    if drawn == 1 {
        format!("1 published resource, and {counted}")
    } else {
        format!("{drawn} published resources, and {counted}")
    }
}

/// One block per published resource, or the statement that there is none.
fn resources_view(published: &[PublishedCodeSystem]) -> AnyView {
    if published.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This root publishes no CodeSystem resource for this canonical. A system served from a built index is declared in the capabilities above and need not be published as a resource."
            </p>
        }
        .into_any();
    }
    let blocks: Vec<AnyView> = published.iter().map(resource_view).collect();
    view! { <div class="mt-3 grid gap-4">{blocks}</div> }.into_any()
}

/// One published resource, as a definition list of its declared facts.
fn resource_view(resource: &PublishedCodeSystem) -> AnyView {
    let heading = resource.version().map_or_else(
        || format!("Version {NOT_DECLARED}"),
        |version| format!("Version {version}"),
    );
    let canonical = resource
        .url()
        .map_or_else(|| format!("Canonical {NOT_DECLARED}"), str::to_owned);
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
    view! {
        <article class="rounded-lg border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900">
            <h3 class="font-mono text-sm font-semibold break-all">{heading}</h3>
            <p class="mt-1 font-mono text-xs break-all text-slate-600 dark:text-slate-300">
                {canonical}
            </p>
            <dl class="mt-2 text-sm">{rows}</dl>
        </article>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_that_names_no_system_still_titles_the_page() {
        assert_eq!(title_of(""), "Code system");
        assert_eq!(
            heading_of(""),
            "This address names no code system",
            "the screen states what it found rather than rendering an empty heading"
        );
    }

    #[test]
    fn a_named_system_titles_the_page_with_its_canonical() {
        assert_eq!(
            title_of("https://terminology.example/x"),
            "https://terminology.example/x"
        );
        assert_eq!(
            heading_of("https://terminology.example/x"),
            "https://terminology.example/x"
        );
    }

    #[test]
    fn the_match_sentence_states_the_drawn_count_and_the_declared_one() {
        assert_eq!(
            match_sentence(1, Some(1)),
            "1 published resource, and this root counted 1"
        );
        assert_eq!(
            match_sentence(0, Some(0)),
            "0 published resources, and this root counted 0"
        );
    }

    #[test]
    fn a_search_that_counted_nothing_says_so_rather_than_inventing_a_number() {
        assert_eq!(
            match_sentence(1, None),
            "1 published resource, and this root counted not declared"
        );
    }
}
