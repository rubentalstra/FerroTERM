//! The FerroTERM lockup in the top left: the brand mark and the wordmark.
//!
//! The mark is the artwork in `assets/brand/ferroterm-icon.svg`, drawn inline
//! so it takes the theme's own brand roles and costs no request. It is a
//! concept at the centre with its related concepts around it, which is what a
//! terminology is for any code system this server holds.
//!
//! The `<svg>` is `aria-hidden` because the wordmark beside it already says
//! the name (<https://www.w3.org/TR/wai-aria-1.2/#aria-hidden>), so a screen
//! reader announces the link once.

use leptos::prelude::*;

use crate::styles;

/// The mark's edges, from the centre concept out to each related one.
const EDGES: &str = concat!(
    r#"<line x1="24" y1="24" x2="24" y2="9"/>"#,
    r#"<line x1="24" y1="24" x2="39" y2="23"/>"#,
    r#"<line x1="24" y1="24" x2="28" y2="39"/>"#,
    r#"<line x1="24" y1="24" x2="10" y2="31"/>"#,
);

/// The related concepts, and the centre one, which is the largest.
const NODES: &str = concat!(
    r#"<circle cx="39" cy="23" r="5"/>"#,
    r#"<circle cx="28" cy="39" r="5"/>"#,
    r#"<circle cx="10" cy="31" r="5"/>"#,
    r#"<circle cx="24" cy="24" r="6.6"/>"#,
);

/// Draws the brand mark, hidden from assistive technology.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Mark(
    /// Classes appended to the shared ones, for a mark that needs its own size.
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 48 48"
            aria-hidden="true"
            focusable="false"
            class=format!("h-7 w-7 shrink-0 {class}")
        >
            <g
                fill="none"
                class="stroke-mark"
                stroke-linecap="round"
                stroke-width="3.4"
                inner_html=EDGES
            ></g>
            <g stroke="none" class="fill-mark-node" inner_html=NODES></g>
            <circle cx="24" cy="9" r="5" class="fill-mark-accent" />
        </svg>
    }
}

/// The lockup the top bar carries: the mark, the name, and what this is.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Lockup() -> impl IntoView {
    view! {
        <span class="flex items-center gap-default">
            <Mark />
            <span class="flex items-baseline gap-tight">
                <span class="text-title font-semibold text-fg">"FerroTERM"</span>
                <span class=styles::EYEBROW>"viewer"</span>
            </span>
        </span>
    }
}
