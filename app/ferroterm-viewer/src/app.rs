//! The application root: the theme, the router, and the shell.

// The diagnostic lands on macro output rather than on an item written here,
// so the expectation covers the module.
#![expect(
    clippy::same_name_method,
    reason = "leptos::component derives a TypedBuilder whose `builder` shadows a trait method"
)]

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_meta::provide_meta_context;
use leptos_router::components::Router;

use crate::components::shell::Shell;
use crate::fhir::FhirClient;
use crate::routes::UI_BASE;
use crate::settings::Settings;
use crate::theme::ThemeMode;

/// The class Tailwind's dark variant is defined against in `style/tailwind.css`.
const DARK_CLASS: &str = "dark";

/// Mounts the theme, the router, and everything below them.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn App() -> impl IntoView {
    provide_meta_context();

    let settings = Settings::load();
    settings.persist();
    provide_context(settings);
    provide_context(FhirClient::from_document());

    // The document element is the outside world, which is what an Effect is
    // for. Every theme-dependent style is a Tailwind `dark:` variant defined
    // against the class this writes.
    Effect::new(move |_| apply_document_theme(settings.theme.get()));

    view! {
        <Router base=UI_BASE>
            <Title formatter=|text| format!("{text} · FerroTERM viewer") />
            <Shell />
        </Router>
    }
}

/// Puts the theme on the document element so the Tailwind dark variant applies.
fn apply_document_theme(mode: ThemeMode) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let classes = root.class_list();
    let changed = match mode {
        ThemeMode::Dark => classes.add_1(DARK_CLASS),
        ThemeMode::Light => classes.remove_1(DARK_CLASS),
    };
    if changed.is_err() {
        leptos::logging::warn!("this browser refused the theme class on the document element");
    }
}
