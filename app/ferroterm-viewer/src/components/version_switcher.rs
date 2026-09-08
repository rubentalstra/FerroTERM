//! The switcher over the four FHIR roots the server mounts.

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::components::icon;
use crate::components::icon::Icon;
use crate::fhir::version::FhirVersion;
use crate::routes::version_link;
use crate::styles;

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
            class="flex items-center gap-0.5 rounded-lg border border-line bg-inset p-0.5"
        >
            <span class=format!("mr-1 inline-flex items-center gap-1 pl-1.5 {}", styles::EYEBROW)>
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
                                    "rounded-md bg-accent px-2 py-0.5 text-small font-semibold text-accent-fg"
                                } else {
                                    "rounded-md px-2 py-0.5 text-small font-medium text-muted hover:bg-raised hover:text-fg"
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
