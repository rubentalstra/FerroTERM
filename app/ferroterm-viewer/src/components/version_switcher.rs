//! The switcher over the four FHIR roots the server mounts.

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::fhir::version::FhirVersion;
use crate::routes::version_link;

/// Links to each served FHIR version, keeping the reader on the same page.
///
/// The links are plain anchors, which `leptos_router` intercepts through its
/// window click handler, so switching version is a client-side navigation that
/// matches the same route and only updates the query.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn VersionSwitcher(
    /// The version the shell resolved for the current address.
    #[prop(into)]
    selected: Signal<FhirVersion>,
) -> impl IntoView {
    let location = use_location();
    view! {
        <nav aria-label="FHIR version" class="flex items-center gap-1">
            <span class="mr-1 text-xs text-slate-500 dark:text-slate-400">"FHIR"</span>
            <For each=move || FhirVersion::ALL key=|version| *version let:version>
                {
                    let href = move || version_link(&location.pathname.get(), version);
                    let active = move || selected.get() == version;
                    view! {
                        <a
                            href=href
                            aria-current=move || if active() { Some("page") } else { None }
                            class=move || {
                                if active() {
                                    "rounded px-2 py-1 text-xs font-semibold bg-teal-600 text-white"
                                } else {
                                    "rounded px-2 py-1 text-xs font-medium text-slate-700 hover:bg-slate-200 dark:text-slate-200 dark:hover:bg-slate-700"
                                }
                            }
                        >
                            {version.label()}
                        </a>
                    }
                }
            </For>
        </nav>
    }
}
