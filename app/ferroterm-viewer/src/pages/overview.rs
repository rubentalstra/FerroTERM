//! The index screen: what this server loaded, and what it serves it as.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::failure::Failure;
use crate::components::reading::Reading;
use crate::components::request_disclosure::RequestDisclosure;
use crate::components::shell::SelectedVersion;
use crate::components::system_table::SystemTable;
use crate::fhir::FhirClient;
use crate::fhir::terminology::SystemCard;
use crate::fhir::version::FhirVersion;
use crate::styles;

/// Shows the FHIR base in use and the code systems the root has loaded.
///
/// Both reads refetch when the version switcher moves, so both are read under
/// a transition boundary rather than a `<Suspense>`
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

    let header = header_view(&client, version);
    let systems = systems_section(&client, version);

    view! {
        <Title text="Overview" />
        {header}
        {systems}
    }
}

/// The screen's own header: what it is, and which root it read.
///
/// The two facts about the root sit in the header rather than in a section of
/// their own. A heading and a lead sentence over one address cost more of the
/// screen than the address does, and this screen's subject is the inventory
/// below.
fn header_view(client: &FhirClient, version: Signal<FhirVersion>) -> AnyView {
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
        <header>
            <h1 class=styles::PAGE_TITLE>"This server"</h1>
            <p class=styles::LEAD>
                "Everything on this page came from the FHIR API below, over HTTP, from your browser."
            </p>
            <dl class="mt-default grid gap-x-loose gap-y-tight sm:grid-cols-[8rem_1fr]">
                <dt class=styles::MUTED>"FHIR base"</dt>
                <dd class=styles::CODE>{base}</dd>
            </dl>
            <Reading label="Reading the capability statement">
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
                                        view! {
                                            <p class=format!(
                                                "mt-default {}",
                                                styles::MUTED,
                                            )>{summary}</p>
                                        }
                                            .into_any()
                                    }
                                    Err(error) => {
                                        let error = error.clone();
                                        view! {
                                            <div class="mt-default">
                                                <Failure error=Signal::stored(error) />
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        </header>
    }
    .into_any()
}

/// The code systems the selected root declares, as one table.
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
        <section class="mt-section" aria-labelledby="systems-heading">
            <h2 id="systems-heading" class=styles::SECTION_TITLE>
                "The code systems this server loaded"
            </h2>
            <p aria-live="polite" class=styles::LEAD>
                {announcement}
            </p>
            <Reading label="Reading the terminology capabilities">
                {move || {
                    capabilities
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(capabilities) => systems_view(capabilities.cards()),
                                    Err(error) => {
                                        let error = error.clone();
                                        view! {
                                            <div class="mt-default">
                                                <Failure error=Signal::stored(error) />
                                            </div>
                                        }
                                            .into_any()
                                    }
                                })
                        })
                }}
            </Reading>
            <RequestDisclosure url />
        </section>
    }
    .into_any()
}

/// The table itself, or the statement that the root declared none.
///
/// The list is a whole-document replacement with no per-card state, so it is a
/// plain `Vec`, which rebuilds every position when the read settles again. A
/// `<For>` would be wrong here: a key it retains is moved rather than
/// re-rendered, so switching FHIR version would keep every card's old body
/// while its canonical stayed the same (verified in `leptos` 0.8.20
/// `for_loop.rs` and `tachys` 0.2.18 `view/keyed.rs`).
fn systems_view(cards: Vec<SystemCard>) -> AnyView {
    if cards.is_empty() {
        return view! {
            <p class=format!(
                "mt-default {}",
                styles::MUTED,
            )>
                "This root declares no code system. A deployment loads one with the offline build."
            </p>
        }
        .into_any();
    }
    view! { <SystemTable cards /> }.into_any()
}

/// How many code systems the root declared, as a sentence.
///
/// The sentence carries what the table is as well as how much of it there is,
/// because a lead paragraph saying the same thing costs a line of every visit
/// and a reader reads it once.
fn count_sentence(count: usize) -> String {
    if count == 1 {
        "1 code system, one row per served version, from this root's terminology capabilities."
            .to_owned()
    } else {
        format!(
            "{count} code systems, one row per served version, from this root's terminology capabilities."
        )
    }
}
