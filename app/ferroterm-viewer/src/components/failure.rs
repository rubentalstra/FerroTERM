//! Rendering a refused request, in the server's own words.

use leptos::prelude::*;

use crate::fhir::error::FhirError;
use crate::fhir::outcome::OperationOutcome;

/// Shows why a request failed, with the server's `OperationOutcome` verbatim.
///
/// A failure is never rendered as an empty section: the reader sees the URL
/// that was asked for, the status that came back, and every issue the server
/// reported, so they can reproduce the request themselves.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Failure(
    /// The failure to explain.
    #[prop(into)]
    error: Signal<FhirError>,
) -> impl IntoView {
    let status = move || {
        error.with(|error| {
            error
                .status()
                .map_or_else(|| "no answer".to_owned(), |status| status.to_string())
        })
    };
    let url = move || error.with(|error| error.url().to_owned());
    let issues = move || {
        error.with(|error| {
            error
                .outcome()
                .map(OperationOutcome::lines)
                .unwrap_or_default()
        })
    };
    let body = move || {
        error.with(|error| match error {
            FhirError::Status { body, .. } if !body.is_empty() => Some(body.clone()),
            FhirError::Transport { message, .. } | FhirError::Decode { message, .. } => {
                Some(message.clone())
            }
            FhirError::Status { .. } | FhirError::Refused { .. } => None,
        })
    };

    let heading = view! {
        <p class="font-medium text-rose-700 dark:text-rose-300">"The request failed: " {status}</p>
        <p class="mt-1 font-mono text-xs break-all text-slate-600 dark:text-slate-300">{url}</p>
    }
    .into_any();

    let reported = view! {
        <ul class="mt-2 space-y-1">
            <For
                each=issues
                key=|line| (line.severity.clone(), line.code.clone(), line.text.clone())
                let:line
            >
                <li class="text-sm">
                    <span class="font-semibold">{line.severity.clone()}</span>
                    " ("
                    <span class="font-mono">{line.code.clone()}</span>
                    "): "
                    {line.text.clone()}
                </li>
            </For>
        </ul>
    }
    .into_any();

    let evidence = view! {
        <Show when=move || body().is_some() fallback=|| ()>
            <pre class="mt-2 overflow-x-auto rounded bg-slate-100 p-2 text-xs dark:bg-slate-800">
                {body}
            </pre>
        </Show>
    }
    .into_any();

    view! {
        <div
            role="alert"
            class="rounded-md border border-rose-300 bg-rose-50 p-3 dark:border-rose-800 dark:bg-rose-950"
        >
            {heading}
            {reported}
            {evidence}
        </div>
    }
}
