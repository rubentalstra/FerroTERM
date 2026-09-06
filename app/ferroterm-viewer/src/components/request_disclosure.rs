//! Showing the FHIR request a section made, for a reader to repeat.

use leptos::prelude::*;

use crate::fhir::curl_line;

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
) -> impl IntoView {
    let curl = move || url.with(|url| curl_line(url));
    view! {
        <details class="mt-4 rounded border border-slate-200 text-xs dark:border-slate-800">
            <summary class="cursor-pointer px-3 py-2 font-medium text-slate-700 dark:text-slate-200">
                "The request this section made"
            </summary>
            <div class="border-t border-slate-200 px-3 py-2 dark:border-slate-800">
                <p class="text-slate-600 dark:text-slate-300">
                    "Select either line to copy it. Both reach the same public API this page uses."
                </p>
                <dl class="mt-2 grid gap-1">
                    <dt class="font-medium">"URL"</dt>
                    <dd>
                        <code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800">
                            {url}
                        </code>
                    </dd>
                    <dt class="mt-1 font-medium">"curl"</dt>
                    <dd>
                        <code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800">
                            {curl}
                        </code>
                    </dd>
                </dl>
                <p class="mt-2">
                    <a
                        href=move || url.get()
                        rel="external"
                        class="text-brand-700 underline dark:text-brand-300"
                    >
                        "Open the answer in this browser"
                    </a>
                </p>
            </div>
        </details>
    }
}
