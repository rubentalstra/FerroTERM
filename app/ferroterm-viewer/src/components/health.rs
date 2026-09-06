//! The health indicator, which polls `GET /health`.

use std::time::Duration;

use leptos::prelude::*;
use thaw::Spinner;
use thaw::SpinnerSize;

use crate::fhir::FhirClient;

/// How often the indicator asks the server whether it is still up.
const POLL: Duration = Duration::from_secs(15);

/// Shows whether the server that served this bundle is answering.
///
/// The resource refetches on every tick, so it is read under `<Transition>`:
/// a `<Suspense>` would flash its fallback on each poll
/// (<https://github.com/leptos-rs/book/blob/main/src/async/12_transition.md>).
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn HealthIndicator() -> impl IntoView {
    let client = expect_context::<FhirClient>();
    let health = LocalResource::new(move || {
        let client = client.clone();
        async move { client.health().await }
    });

    match set_interval_with_handle(move || health.refetch(), POLL) {
        Ok(handle) => on_cleanup(move || handle.clear()),
        Err(_) => leptos::logging::warn!("this browser refused an interval; health is read once"),
    }

    let state = move || {
        health.get().map(|result| match result.as_ref() {
            Ok(status) => (
                "Serving",
                status.to_string(),
                "bg-teal-100 text-teal-900 dark:bg-teal-900 dark:text-teal-100",
            ),
            Err(error) => (
                "Unreachable",
                error.to_string(),
                "bg-rose-100 text-rose-900 dark:bg-rose-900 dark:text-rose-100",
            ),
        })
    };

    view! {
        <Transition fallback=|| {
            view! { <Spinner size=SpinnerSize::ExtraTiny label="Checking the server" /> }
        }>
            {move || {
                state()
                    .map(|(word, detail, tint)| {
                        view! {
                            <span
                                class=format!("rounded-full px-2 py-0.5 text-xs font-medium {tint}")
                                title=detail
                            >
                                {word}
                            </span>
                        }
                    })
            }}
        </Transition>
    }
}
