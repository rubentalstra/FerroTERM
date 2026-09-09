//! Everything about this server and this viewer, on one screen.
//!
//! Three panes: what the four served roots declare, the figures the build
//! committed, and what this browser remembers about the reader. They are one
//! screen rather than three because none of them is about a code, and a reader
//! who came to ask about the server should not have to guess which of three
//! entries holds the answer.
//!
//! Each pane keeps the address it had. A link written before they were one
//! screen still opens the pane it named, through the redirects in `shell.rs`.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::icon;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::pages::evidence;
use crate::pages::settings;
use crate::pages::versions;
use crate::styles;

/// The pane a fragment names, as the link that jumps to it.
struct Pane(&'static str, &'static str, Glyph);

/// The panes, in the order the screen draws them.
const PANES: [Pane; 3] = [
    Pane(
        "about-versions-heading",
        "The four FHIR versions",
        icon::VERSION,
    ),
    Pane(
        "about-evidence-heading",
        "The evidence this build ships",
        icon::EVIDENCE,
    ),
    Pane("about-settings-heading", "Settings", icon::SETTINGS),
];

/// Shows the three panes about this server and this viewer.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn AboutPage() -> impl IntoView {
    let jumps: Vec<AnyView> = PANES
        .iter()
        .map(|Pane(id, label, glyph)| {
            view! {
                <li>
                    <a
                        href=format!("#{id}")
                        class=format!("inline-flex items-center gap-tight {}", styles::LINK)
                    >
                        <Icon glyph=*glyph />
                        {*label}
                    </a>
                </li>
            }
            .into_any()
        })
        .collect();

    view! {
        <Title text="About this server" />
        <h1 class=styles::PAGE_TITLE>"About this server"</h1>
        <p class=styles::LEAD>
            "What the four served roots declare, what the build measured before it shipped, and what this browser remembers about you. Nothing here is about a code."
        </p>
        <nav aria-label="The panes of this screen" class="mt-default">
            <ul class="flex flex-wrap gap-loose text-body">{jumps}</ul>
        </nav>
        {versions::pane()}
        {evidence::pane()}
        {settings::pane()}
    }
}
