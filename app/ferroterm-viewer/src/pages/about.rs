//! Everything about this server and this viewer, on one screen.
//!
//! Three tabs, one shown at a time. They are one screen because none of them
//! is about a code, and a reader arrives with one of the three questions.
//!
//! Each tab keeps the address its pane had. A link written before they were
//! one screen still opens the tab it named, through the redirects in
//! `shell.rs`.

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

/// One tab: the id it answers to, its name, and its glyph.
struct Tab(&'static str, &'static str, Glyph);

/// The tabs, in the order the screen draws them.
///
/// The first is what a reader who named none of them gets.
const TABS: [Tab; 3] = [
    Tab("versions", "The four FHIR versions", icon::VERSION),
    Tab("evidence", "The evidence this build ships", icon::EVIDENCE),
    Tab("settings", "Settings", icon::SETTINGS),
];

/// The classes every tab carries, whichever one is being read.
const TAB_BASE: &str = "flex items-center gap-default rounded-t-md border-b-2 px-default py-tight text-small font-medium";

/// The classes the tab being read carries.
const TAB_ACTIVE: &str = "border-accent text-accent";

/// The classes every other tab carries.
const TAB_RESTING: &str = "state-change border-transparent text-muted hover:bg-inset hover:text-fg";

/// Shows the three tabs about this server and this viewer.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn AboutPage() -> impl IntoView {
    // An address naming a tab opens it, and a tab is a link to a fragment of
    // this same screen, which never re-runs this body, so the fragment is
    // followed rather than read once
    // (`leptos_router` 0.8.15 `use_location`).
    let named = use_location().hash;
    let shown = Memo::new(move |_| {
        named.with(|named| {
            let asked = named.trim_start_matches('#');
            TABS.iter()
                .position(|Tab(id, _, _)| anchor(id) == asked)
                .unwrap_or_default()
        })
    });

    let strip: Vec<AnyView> = TABS
        .iter()
        .enumerate()
        .map(|(index, Tab(id, label, glyph))| tab(index, id, label, *glyph, shown))
        .collect();
    let bodies = [versions::pane(), evidence::pane(), settings::pane()];
    let panes: Vec<AnyView> = TABS
        .iter()
        .zip(bodies)
        .enumerate()
        .map(|(index, (Tab(id, label, _), body))| pane(index, id, label, shown, body))
        .collect();

    view! {
        <Title text="About this server" />
        <h1 class=styles::PAGE_TITLE>"About this server"</h1>
        <nav
            aria-label="About this server"
            class="mt-loose flex flex-wrap gap-default border-b border-line"
        >
            {strip}
        </nav>
        {panes}
    }
}

/// The element id one tab answers to.
fn anchor(id: &str) -> String {
    format!("about-{id}-heading")
}

/// One tab, as the link that opens it.
///
/// A link rather than a tab widget: the tab being read is an address a reader
/// can send and the browser can go back to, the keyboard needs no handler of
/// ours, and `aria-current` is what a screen reader announces
/// (<https://www.w3.org/TR/wai-aria-1.2/#aria-current>).
fn tab(
    index: usize,
    id: &'static str,
    label: &'static str,
    glyph: Glyph,
    shown: Memo<usize>,
) -> AnyView {
    let here = move || shown.get() == index;
    view! {
        <a
            href=format!("#{}", anchor(id))
            aria-current=move || if here() { "true" } else { "false" }
            class=move || {
                format!("{TAB_BASE} {}", if here() { TAB_ACTIVE } else { TAB_RESTING })
            }
        >
            <Icon glyph=glyph />
            {label}
        </a>
    }
    .into_any()
}

/// One pane, drawn under the strip and shown only while its tab is the one
/// being read.
///
/// The three stay in the document so that every address this screen answers
/// finds the element it names. A hidden one is out of the accessibility tree,
/// which is what `hidden` means
/// (<https://html.spec.whatwg.org/multipage/interaction.html#the-hidden-attribute>).
fn pane(
    index: usize,
    id: &'static str,
    label: &'static str,
    shown: Memo<usize>,
    body: AnyView,
) -> AnyView {
    let heading = anchor(id);
    view! {
        <section
            id=heading.clone()
            aria-labelledby=format!("{heading}-name")
            hidden=move || shown.get() != index
            class=format!("mt-loose panel-p {}", styles::PANEL)
        >
            <h2 id=format!("{heading}-name") class="sr-only">
                {label}
            </h2>
            {body}
        </section>
    }
    .into_any()
}
