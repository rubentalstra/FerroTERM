//! Showing the FHIR request a section made, for a reader to repeat.

use leptos::prelude::*;

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
            <dt class="mt-1 font-medium">"Body"</dt>
            <dd>
                <code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800">
                    {sent}
                </code>
            </dd>
        }
        .into_any()
    });
    let open_line = body.is_none().then(|| {
        view! {
            <p class="mt-2">
                <a
                    href=move || url.get()
                    rel="external"
                    class="text-brand-700 underline dark:text-brand-300"
                >
                    "Open the answer in this browser"
                </a>
            </p>
        }
        .into_any()
    });
    view! {
        <details class="mt-4 rounded border border-slate-200 text-xs dark:border-slate-800">
            <summary class="cursor-pointer px-3 py-2 font-medium text-slate-700 dark:text-slate-200">
                {summary}
            </summary>
            <div class="border-t border-slate-200 px-3 py-2 dark:border-slate-800">
                <p class="text-slate-600 dark:text-slate-300">
                    "Select a line to copy it. Each one reaches the same public API this page uses."
                </p>
                <dl class="mt-2 grid gap-1">
                    <dt class="font-medium">"URL"</dt>
                    <dd>
                        <code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800">
                            {url}
                        </code>
                    </dd>
                    {sent_line}
                    <dt class="mt-1 font-medium">"curl"</dt>
                    <dd>
                        <code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800">
                            {curl}
                        </code>
                    </dd>
                </dl>
                {open_line}
            </div>
        </details>
    }
}
