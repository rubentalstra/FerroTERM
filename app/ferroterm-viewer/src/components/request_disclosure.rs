//! Showing the FHIR request a section made, for a reader to repeat.

use leptos::prelude::*;

use crate::components::icon;
use crate::components::icon::Icon;
use crate::fhir::curl_line;
use crate::fhir::curl_post_line;

/// Discloses the request one section issued, as a URL and as a `curl` line.
///
/// Both are plain selectable text rather than a copy button, because the
/// clipboard API is unavailable outside a secure context
/// (<https://developer.mozilla.org/en-US/docs/Web/API/Clipboard/writeText>) and
/// a terminology server on a plain-HTTP network is the ordinary deployment.
/// The anchor carries `rel="external"` so `leptos_router` lets the browser
/// follow it to the server instead of routing it inside the bundle.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn RequestDisclosure(
    /// The address the section read.
    #[prop(into)]
    url: Signal<String>,
    /// The `Parameters` body, for a section that invoked an operation with a
    /// `POST` because its input does not fit in a query.
    #[prop(optional, into)]
    body: Option<Signal<String>>,
    /// Which of a section's requests this one is, for a section that made
    /// several. It is fixed at setup, and the summary carries it.
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView {
    let summary = label.map_or_else(
        || "The request this section made".to_owned(),
        |label| format!("The {label} request this section made"),
    );
    let curl = move || match body {
        Some(body) => url.with(|url| body.with(|body| curl_post_line(url, body))),
        None => url.with(|url| curl_line(url)),
    };
    let sent = move || body.map(|body| body.get());
    // Both branches are decided once, by a prop fixed at setup, so they are
    // erased views rather than `<Show>`, which is generic over its children
    // and would instantiate this subtree twice.
    let sent_line = body.is_some().then(|| {
        view! {
            <dt class="mt-tight font-medium">"Body"</dt>
            <dd>
                <code class="block rounded-md bg-inset p-default wrap-break-word text-fg">
                    {sent}
                </code>
            </dd>
        }
        .into_any()
    });
    let open_line = body.is_none().then(|| {
        view! {
            <p class="mt-default">
                <a
                    href=move || url.get()
                    rel="external"
                    class="inline-flex items-center gap-tight text-accent underline"
                >
                    "Open the answer in this browser"
                    <Icon glyph=icon::EXTERNAL class="h-3.5 w-3.5" />
                </a>
            </p>
        }
        .into_any()
    });
    view! {
        <details class="mt-loose rounded-md border border-line text-small">
            <summary class="cursor-pointer px-default py-default font-medium text-muted">
                {summary}
            </summary>
            <div class="border-t border-line px-default py-default">
                <p class="text-muted">
                    "Select a line to copy it. Each one reaches the same public API this page uses."
                </p>
                <dl class="mt-default grid gap-tight">
                    <dt class="font-medium">"URL"</dt>
                    <dd>
                        <code class="block rounded-md bg-inset p-default wrap-break-word text-fg">
                            {url}
                        </code>
                    </dd>
                    {sent_line}
                    <dt class="mt-tight font-medium">"curl"</dt>
                    <dd>
                        <code class="block rounded-md bg-inset p-default wrap-break-word text-fg">
                            {curl}
                        </code>
                    </dd>
                </dl>
                {open_line}
            </div>
        </details>
    }
}
