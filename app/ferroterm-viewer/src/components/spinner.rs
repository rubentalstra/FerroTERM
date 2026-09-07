//! The busy indicator every `<Suspense>` and `<Transition>` fallback shows.

use leptos::prelude::*;

/// Announces that the viewer is waiting on the server, and says what for.
///
/// The ring is decorative and hidden from assistive technology; the label
/// carries the meaning, in a `status` live region so a screen reader hears it
/// without the focus moving (<https://www.w3.org/TR/wai-aria-1.2/#status>).
/// `style/tailwind.css` stops the rotation under `prefers-reduced-motion`.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Spinner(
    /// What the viewer is waiting for, read out as the indicator appears.
    label: &'static str,
    /// Renders the ring at the size of the surrounding text, for a spinner
    /// that sits inside a line rather than in a section of its own.
    #[prop(optional)]
    inline: bool,
) -> impl IntoView {
    let ring = if inline {
        "h-3 w-3 animate-spin rounded-full border border-current border-t-transparent"
    } else {
        "h-5 w-5 animate-spin rounded-full border-2 border-current border-t-transparent"
    };
    view! {
        <span
            role="status"
            class="inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-400"
        >
            <span class=ring aria-hidden="true"></span>
            {label}
        </span>
    }
}
