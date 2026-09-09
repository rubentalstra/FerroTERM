//! Everything about this server and this viewer, on one screen.
//!
//! Three panes, each closed until a reader opens it. They are one screen
//! because none of them is about a code, and they are closed because a reader
//! comes here with one of the three questions, not all three.
//!
//! Each pane keeps the address it had. A link written before they were one
//! screen still opens the pane it named, through the redirects in `shell.rs`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

use crate::components::icon;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::pages::evidence;
use crate::pages::settings;
use crate::pages::versions;
use crate::styles;

/// One pane: the heading it is labelled by, its name, and its glyph.
struct Pane(&'static str, &'static str, Glyph);

/// The panes, in the order the screen draws them.
const PANES: [Pane; 3] = [
    Pane("versions", "The four FHIR versions", icon::VERSION),
    Pane("evidence", "The evidence this build ships", icon::EVIDENCE),
    Pane("settings", "Settings", icon::SETTINGS),
];

/// Shows the three panes about this server and this viewer.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn AboutPage() -> impl IntoView {
    // An address naming a pane opens it, and a link to another pane from
    // this screen is a fragment navigation that never re-runs this body, so
    // the fragment is followed rather than read once
    // (`leptos_router` 0.8.15 `use_location`).
    let named = use_location().hash;
    let bodies = [versions::pane(), evidence::pane(), settings::pane()];
    let panes: Vec<AnyView> = PANES
        .into_iter()
        .zip(bodies)
        .map(|(Pane(id, label, glyph), body)| pane(id, label, glyph, named, body))
        .collect();

    view! {
        <Title text="About this server" />
        <h1 class=styles::PAGE_TITLE>"About this server"</h1>
        <div class="mt-loose grid gap-default">{panes}</div>
    }
}

/// The element id one pane answers to.
fn anchor(id: &str) -> String {
    format!("about-{id}-heading")
}

/// One pane, closed until a reader opens it or an address names it.
///
/// A `<details>` rather than a tab list: the browser gives the disclosure its
/// keyboard contract and its announcement for free
/// (<https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/>).
///
/// The `id` is on the `<details>`, so an address naming a pane scrolls to it,
/// and the `name` groups the three under one accordion, which is what closes
/// the others when one opens
/// (<https://developer.mozilla.org/en-US/docs/Web/HTML/Element/details#name>).
fn pane(
    id: &'static str,
    label: &'static str,
    glyph: Glyph,
    named: Memo<String>,
    body: AnyView,
) -> AnyView {
    let opened = move || named.with(|named| named.trim_start_matches('#') == anchor(id));
    view! {
        <details id=anchor(id) name="about" open=opened class=styles::PANEL>
            <summary class=format!(
                "flex cursor-pointer items-center gap-default panel-p {}",
                styles::SECTION_TITLE,
            )>
                <Icon glyph=glyph />
                {label}
            </summary>
            <div class="border-t border-line panel-p">{body}</div>
        </details>
    }
    .into_any()
}
