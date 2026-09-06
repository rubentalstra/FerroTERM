//! The screen an address inside `/ui` that names no page reaches.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::shell::SelectedVersion;
use crate::routes::ui_link;

/// Says that the address names no screen, and offers the way back.
///
/// The fallback is scoped to the bundle's own prefix, so a FHIR path the
/// server does not serve still answers an `OperationOutcome` rather than this.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn NotFoundPage() -> impl IntoView {
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    view! {
        <Title text="Not found" />
        <h1 class="text-2xl font-semibold">"No such screen"</h1>
        <p class="mt-2 text-sm text-slate-600 dark:text-slate-300">
            "The viewer has no page at this address."
        </p>
        <p class="mt-4 text-sm">
            <a
                href=move || ui_link("", version.get())
                class="text-teal-700 underline dark:text-teal-300"
            >
                "Back to the overview"
            </a>
        </p>
    }
}
