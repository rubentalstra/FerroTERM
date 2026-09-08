//! Rendering a refused request, in the server's own words.

use leptos::prelude::*;

use crate::components::icon;
use crate::components::icon::Icon;
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
        <p class="flex items-center gap-tight font-medium text-danger">
            <Icon glyph=icon::FAILURE />
            "The request failed: "
            {status}
        </p>
        <p class="mt-tight font-mono text-small break-all text-muted">{url}</p>
    }
    .into_any();

    // The list is a whole-value replacement with no per-issue state, so it is
    // a plain `Vec`, which rebuilds every position. A `<For>` key it retained
    // would be moved rather than re-rendered, and the issue would keep its old
    // wording after a second refusal at the same position.
    let reported = view! {
        <ul class="mt-default space-y-tight">
            {move || {
                issues()
                    .into_iter()
                    .map(|line| {
                        view! {
                            <li class="text-body">
                                <span class="font-semibold">{line.severity}</span>
                                " ("
                                <span class="font-mono">{line.code}</span>
                                "): "
                                {line.text}
                                {detail_codes(&line.details)}
                            </li>
                        }
                            .into_any()
                    })
                    .collect::<Vec<AnyView>>()
            }}
        </ul>
    }
    .into_any();

    let evidence = view! {
        <Show when=move || body().is_some() fallback=|| ()>
            <pre class="mt-default overflow-x-auto rounded-md bg-inset p-default text-small text-fg">
                {body}
            </pre>
        </Show>
    }
    .into_any();

    view! {
        <div
            role="alert"
            class="rounded-md border border-danger-soft-fg/30 bg-danger-soft p-default text-danger-soft-fg"
        >
            {heading}
            {reported}
            {evidence}
        </div>
    }
}

/// The codes of `issue.details`, which classify the refusal.
///
/// A server states the class of a refusal as a coding beside the prose
/// (<https://hl7.org/fhir/R4B/operationoutcome.html>), and a reader deciding
/// what to do next needs the code, not only the sentence.
fn detail_codes(details: &[String]) -> AnyView {
    if details.is_empty() {
        return ().into_any();
    }
    let drawn: Vec<AnyView> = details
        .iter()
        .map(|coding| view! { <li class="font-mono break-all">{coding.clone()}</li> }.into_any())
        .collect();
    view! { <ul class="mt-tight ml-loose text-small text-muted">{drawn}</ul> }.into_any()
}
