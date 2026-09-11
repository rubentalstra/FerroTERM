//! The index screen: what this server loaded, and what it serves it as.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::failure::Failure;
use crate::components::reading::Reading;
use crate::components::shell::SelectedVersion;
use crate::components::system_table::SystemTable;
use crate::fhir::FhirClient;
use crate::fhir::code_system::CodeSystemSearch;
use crate::fhir::terminology::SystemCard;
use crate::fhir::version::FhirVersion;
use crate::styles;

/// Shows the FHIR base in use and the code systems the root has loaded.
///
/// Both reads refetch when the version switcher moves, so both are read under
/// a transition boundary rather than a `<Suspense>`
/// (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
/// Each section renders its own failure, so one refused read never blanks the
/// screen.
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
/// The root and what it answered sit on one line under the title. The screen's
/// subject is the inventory below it, and every line this header takes is a
/// row of that inventory a reader has to scroll for.
fn header_view(client: &FhirClient, version: Signal<FhirVersion>) -> AnyView {
    let statement_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = statement_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });

    let base_client = client.clone();
    let base = move || base_client.version_base(version.get());

    view! {
        <header>
            <h1 class=styles::PAGE_TITLE>"This server"</h1>
            <div class="mt-tight flex flex-wrap items-center gap-loose">
                <dl class="flex flex-wrap items-baseline gap-default">
                    <dt class=styles::MUTED>"FHIR base"</dt>
                    <dd class=styles::CODE>{base}</dd>
                </dl>
                <Reading label="Reading the capability statement" inline=true>
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
                                            view! { <span class=styles::MUTED>{summary}</span> }
                                                .into_any()
                                        }
                                        Err(error) => {
                                            let error = error.clone();
                                            view! { <Failure error=Signal::stored(error) /> }.into_any()
                                        }
                                    })
                            })
                    }}
                </Reading>
            </div>
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
    // A second read, narrowed to three elements, so a row can lead with the
    // name a person recognises rather than the canonical a request carries.
    // The capability statement carries no name for a code system, and the
    // published resource may carry its concepts inline, so `_elements` is what
    // makes this affordable at all
    // (<https://hl7.org/fhir/R5/search.html#elements>).
    let names_client = client.clone();
    let published = LocalResource::new(move || {
        let client = names_client.clone();
        let version = version.get();
        async move { client.code_system_names(version).await }
    });
    let names = Memo::new(move |_| {
        published.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(CodeSystemSearch::names)
                .unwrap_or_default()
        })
    });

    // The heading is the live region, so the count is announced when the read
    // settles without costing the screen a line that repeats it. Until then it
    // says what the section is, and it says that again on a refusal, which the
    // failure's own `role="alert"` carries.
    let announcement = Memo::new(move |_| {
        capabilities.with(|answered| {
            answered
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map_or_else(
                    || "Code systems".to_owned(),
                    |capabilities| count_sentence(capabilities.cards().len()),
                )
        })
    });

    view! {
        <section class="mt-section" aria-labelledby="systems-heading">
            <h2 id="systems-heading" aria-live="polite" class=styles::SECTION_TITLE>
                {announcement}
            </h2>
            <Reading label="Reading the terminology capabilities">
                {move || {
                    capabilities
                        .with(|answered| {
                            answered
                                .as_ref()
                                .map(|result| match result {
                                    Ok(capabilities) => {
                                        systems_view(capabilities.cards(), names.get())
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
fn systems_view(cards: Vec<SystemCard>, names: BTreeMap<String, String>) -> AnyView {
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
    view! { <SystemTable cards names /> }.into_any()
}

/// How many code systems the root declared, as the section's heading.
///
/// The count is the heading rather than a sentence under it: a lead paragraph
/// saying what the table is costs a row of the table on every visit, and the
/// table says it itself.
fn count_sentence(count: usize) -> String {
    if count == 1 {
        "1 code system".to_owned()
    } else {
        format!("{count} code systems")
    }
}
