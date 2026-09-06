//! The index screen: what this server loaded, and what it serves it as.

use leptos::prelude::*;
use leptos_meta::Title;
use thaw::Spinner;

use crate::components::code_system_card::CodeSystemCard;
use crate::components::failure::Failure;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;
use crate::fhir::terminology::SystemCard;
use crate::fhir::version::FhirVersion;

/// Shows the FHIR base in use and the code systems the root has loaded.
///
/// Both reads refetch when the version switcher moves, so both are read under
/// `<Transition>` rather than `<Suspense>`
/// (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
/// Each section discloses the request it made and renders its own failure, so
/// one refused read never blanks the screen.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn OverviewPage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();

    let heading = view! {
        <Title text="Overview" />
        <h1 class="text-2xl font-semibold">"This server"</h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "Everything on this page came from the FHIR API below, over HTTP, from your browser."
        </p>
    }
    .into_any();

    let server = server_section(&client, version);
    let systems = systems_section(&client, version);

    view! {
        {heading}
        {server}
        {systems}
    }
}

/// What the selected root says about itself.
fn server_section(client: &FhirClient, version: Signal<FhirVersion>) -> AnyView {
    let statement_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = statement_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });

    let base_client = client.clone();
    let base = move || base_client.version_base(version.get());
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.metadata_url(version.get()));

    view! {
        <section class="mt-6" aria-labelledby="capability-heading">
            <h2 id="capability-heading" class="text-lg font-medium">
                "What the root declares"
            </h2>
            <dl class="mt-2 grid gap-2 text-sm sm:grid-cols-[10rem_1fr]">
                <dt class="font-medium">"FHIR base"</dt>
                <dd class="font-mono break-all">{base}</dd>
            </dl>
            <Transition fallback=|| {
                view! { <Spinner label="Reading the capability statement" /> }
            }>
                {move || {
                    statement
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(statement) => {
                                        let summary = statement
                                            .summary()
                                            .unwrap_or_else(|| {
                                                "the root answered, and declared neither a FHIR version nor its software"
                                                    .to_owned()
                                            });
                                        view! { <p class="mt-2 text-sm">{summary}</p> }.into_any()
                                    }
                                    Err(error) => {
                                        let error = error.clone();
                                        view! {
                                            <div class="mt-2">
                                                <Failure error=Signal::stored(error) />
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        })
                }}
            </Transition>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The code systems the selected root declares, one card each.
fn systems_section(client: &FhirClient, version: Signal<FhirVersion>) -> AnyView {
    let capabilities_client = client.clone();
    let capabilities = LocalResource::new(move || {
        let client = capabilities_client.clone();
        let version = version.get();
        async move { client.terminology_capabilities(version).await }
    });
    let url_client = client.clone();
    let url = Signal::derive(move || url_client.terminology_metadata_url(version.get()));

    // The live region is in the document before the read settles, which is
    // what lets a screen reader announce the count when it arrives. It stays
    // silent on a refusal, which the failure's own `role="alert"` carries.
    let announcement = Memo::new(move |_| {
        capabilities.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|capabilities| count_sentence(capabilities.cards().len()))
                .unwrap_or_default()
        })
    });

    view! {
        <section class="mt-8" aria-labelledby="systems-heading">
            <h2 id="systems-heading" class="text-lg font-medium">
                "The code systems this server loaded"
            </h2>
            <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
                "Every card below is this root's terminology capabilities, rendered. The viewer names no code system of its own."
            </p>
            <p aria-live="polite" class="mt-2 text-sm text-slate-600 dark:text-slate-300">
                {announcement}
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
                                    Ok(capabilities) => cards_view(capabilities.cards()),
                                    Err(error) => {
                                        let error = error.clone();
                                        view! {
                                            <div class="mt-2">
                                                <Failure error=Signal::stored(error) />
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        })
                }}
            </Transition>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The cards themselves, or the statement that the root declared none.
///
/// The list is a whole-document replacement with no per-card state, so it is a
/// plain `Vec`, which rebuilds every position when the read settles again. A
/// `<For>` would be wrong here: a key it retains is moved rather than
/// re-rendered, so switching FHIR version would keep every card's old body
/// while its canonical stayed the same (verified in `leptos` 0.8.20
/// `for_loop.rs` and `tachys` 0.2.18 `view/keyed.rs`).
fn cards_view(cards: Vec<SystemCard>) -> AnyView {
    if cards.is_empty() {
        return view! {
            <p class="mt-3 text-sm text-slate-600 dark:text-slate-300">
                "This root declares no code system. A deployment loads one with the offline build."
            </p>
        }
        .into_any();
    }
    let drawn: Vec<AnyView> = cards
        .into_iter()
        .map(|card| view! { <CodeSystemCard card /> }.into_any())
        .collect();
    view! { <div class="mt-3 grid gap-4">{drawn}</div> }.into_any()
}

/// How many code systems the root declared, as a sentence.
fn count_sentence(count: usize) -> String {
    if count == 1 {
        "1 code system".to_owned()
    } else {
        format!("{count} code systems")
    }
}
