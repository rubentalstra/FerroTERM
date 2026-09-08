//! The button every screen uses for an action that stays on the page.

use leptos::ev::MouseEvent;
use leptos::prelude::*;

/// The classes every button shares, so one control reads the same everywhere.
const BASE: &str = crate::styles::BUTTON_QUIET;

/// A button that runs Rust on activation and never navigates.
///
/// It is a real `<button type="button">`, so it is keyboard reachable and
/// operable with Enter and Space without any of that being written here.
/// Anything that goes somewhere is an `<a>` instead, which the router
/// intercepts.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Button(
    /// Runs when the reader activates the button.
    on_click: impl FnMut(MouseEvent) + 'static,
    /// Classes appended to the shared ones, for a button that needs its own
    /// colour or spacing.
    #[prop(optional)]
    class: &'static str,
    /// What the button shows.
    children: Children,
) -> impl IntoView {
    view! {
        <button type="button" class=format!("{BASE} {class}") on:click=on_click>
            {children()}
        </button>
    }
}
