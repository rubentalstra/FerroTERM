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
use crate::density::Density;
use crate::fhir::FhirClient;
use crate::routes::UI_BASE;
use crate::settings::Settings;
use crate::theme::ThemeMode;

/// The class Tailwind's dark variant is defined against in `style/tailwind.css`.
const DARK_CLASS: &str = "dark";

/// The attribute the compact density variables are selected on.
const DENSITY_ATTRIBUTE: &str = "data-density";

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
    // for. The colour roles in `style/tailwind.css` are redefined under the
    // class this writes, and the density variables under the attribute.
    Effect::new(move |_| apply_document_theme(settings.theme.get()));
    Effect::new(move |_| apply_document_density(settings.density.get()));

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

/// Puts the density on the document element so the compact variables apply.
///
/// The comfortable density is the stylesheet's own `:root` block, so it is the
/// absence of the attribute rather than a second value to keep in step.
fn apply_document_density(density: Density) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let changed = match density {
        Density::Compact => root.set_attribute(DENSITY_ATTRIBUTE, density.key()),
        Density::Comfortable => root.remove_attribute(DENSITY_ATTRIBUTE),
    };
    if changed.is_err() {
        leptos::logging::warn!(
            "this browser refused the density attribute on the document element"
        );
    }
}
