//! The index screen: what the selected FHIR root says about itself.

use leptos::prelude::*;
use leptos_meta::Title;
use thaw::Spinner;

use crate::components::failure::Failure;
use crate::components::shell::SelectedVersion;
use crate::fhir::FhirClient;

/// Shows the FHIR base in use and what its `CapabilityStatement` declares.
///
/// The read refetches whenever the version changes, so it is read under
/// `<Transition>` rather than `<Suspense>`.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn HomePage() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let SelectedVersion(version) = expect_context::<SelectedVersion>();

    let statement_client = client.clone();
    let statement = LocalResource::new(move || {
        let client = statement_client.clone();
        let version = version.get();
        async move { client.capability_statement(version).await }
    });

    let base_client = client.clone();
    let base = move || base_client.version_base(version.get());

    let heading = view! {
        <Title text="Overview" />
        <h1 class="text-2xl font-semibold">"This server"</h1>
        <p class="mt-1 text-sm text-slate-600 dark:text-slate-300">
            "Everything on this page came from the FHIR API below, over HTTP, from your browser."
        </p>
    }
    .into_any();

    let address = view! {
        <dl class="mt-6 grid gap-2 text-sm sm:grid-cols-[10rem_1fr]">
            <dt class="font-medium">"FHIR base"</dt>
            <dd class="font-mono break-all">{base}</dd>
        </dl>
    }
    .into_any();

    let declared = view! {
        <section class="mt-6" aria-labelledby="capability-heading">
            <h2 id="capability-heading" class="text-lg font-medium">
                "What the root declares"
            </h2>
            <Transition fallback=|| {
                view! { <Spinner label="Reading the capability statement" /> }
            }>
                {move || {
                    statement
                        .get()
                        .map(|result| match result.as_ref() {
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
                }}
            </Transition>
        </section>
    }
    .into_any();

    view! {
        {heading}
        {address}
        {declared}
    }
}
