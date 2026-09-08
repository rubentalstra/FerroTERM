//! The switcher over the four FHIR roots the server mounts.

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::components::icon;
use crate::components::icon::Icon;
use crate::fhir::version::FhirVersion;
use crate::routes::version_link;

/// Links to each served FHIR version, keeping the reader on the same page.
///
/// The links are plain anchors, which `leptos_router` intercepts through its
/// window click handler, so switching version is a client-side navigation that
/// matches the same route and only updates the query.
///
/// It is drawn as one segmented control in the top bar, because it changes
/// which root every screen reads from rather than moving the reader to a
/// screen of its own.
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
        <nav
            aria-label="FHIR version"
            class="flex items-center gap-1 rounded-lg border border-slate-200 bg-slate-100 p-1 dark:border-slate-700 dark:bg-slate-800"
        >
            <span class="mr-1 inline-flex items-center gap-1 text-xs text-slate-600 dark:text-slate-400">
                <Icon glyph=icon::VERSION class="h-3.5 w-3.5" />
                "FHIR"
            </span>
            <For each=move || FhirVersion::ALL key=|version| *version let:version>
                {
                    let href = move || {
                        location
                            .pathname
                            .with(|pathname| {
                                location
                                    .search
                                    .with(|search| version_link(pathname, search, version))
                            })
                    };
                    let active = move || selected.get() == version;
                    view! {
                        <a
                            href=href
                            aria-current=move || if active() { Some("page") } else { None }
                            class=move || {
                                if active() {
                                    "rounded-md bg-brand-700 px-2 py-1 text-xs font-semibold text-white dark:bg-brand-400 dark:text-slate-900"
                                } else {
                                    "rounded-md px-2 py-1 text-xs font-medium text-slate-700 hover:bg-white dark:text-slate-200 dark:hover:bg-slate-700"
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
