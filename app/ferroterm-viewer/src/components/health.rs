//! The health indicator, which polls `GET /health`.

use std::time::Duration;

use leptos::prelude::*;

use crate::components::icon;
use crate::components::icon::Icon;
use crate::components::reading::Reading;
use crate::fhir::FhirClient;

/// How often the indicator asks the server whether it is still up.
const POLL: Duration = Duration::from_secs(15);

/// Shows whether the server that served this bundle is answering.
///
/// The resource refetches on every tick, so it is read under a transition
/// boundary: a `<Suspense>` would flash its fallback on each poll
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
                icon::SERVING,
                "Serving",
                status.to_string(),
                crate::styles::BADGE_OK,
            ),
            Err(error) => (
                icon::UNREACHABLE,
                "Unreachable",
                error.to_string(),
                crate::styles::BADGE_DANGER,
            ),
        })
    };

    view! {
        <Reading inline=true label="Checking the server">
            {move || {
                state()
                    .map(|(glyph, word, detail, tint)| {
                        view! {
                            <span class=tint title=detail>
                                <Icon glyph=glyph class="h-3.5 w-3.5" />
                                {word}
                            </span>
                        }
                    })
            }}
        </Reading>
    }
}
