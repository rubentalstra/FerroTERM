//! The chrome around every screen, and the routes it wraps.

use leptos::prelude::*;
use leptos_router::components::Route;
use leptos_router::components::Routes;
use leptos_router::hooks::use_location;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use leptos_router::path;

use crate::components::health::HealthIndicator;
use crate::components::theme_toggle::ThemeToggle;
use crate::components::version_switcher::VersionSwitcher;
use crate::fhir::version::FhirVersion;
use crate::pages::not_found::NotFoundPage;
use crate::pages::overview::OverviewPage;
use crate::pages::settings::SettingsPage;
use crate::routes::UI_BASE;
use crate::routes::ui_link;
use crate::settings::Settings;

/// The FHIR version the current address selects, for every screen to read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedVersion(pub(crate) Signal<FhirVersion>);

/// The shell's own query parameters.
#[derive(Clone, Debug, Params, PartialEq)]
struct ShellQuery {
    /// The FHIR version, as its path segment.
    fhir: Option<String>,
}

/// The header, the routed screen, and the footer.
///
/// The version is read reactively from the query on every render, because a
/// switcher link is a navigation that matches this same route: `leptos_router`
/// then updates the params without re-running this body, so an untracked read
/// taken at setup would go stale.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn Shell() -> impl IntoView {
    let settings = expect_context::<Settings>();
    let query = use_query::<ShellQuery>();
    // NOTE: A `?fhir=` value naming no served version is a link a reader typed,
    // so it reads as absent and the stored default applies.
    let version = Signal::derive(move || {
        query
            .read()
            .as_ref()
            .ok()
            .and_then(|query| query.fhir.as_deref())
            .and_then(|text| text.parse::<FhirVersion>().ok())
            .unwrap_or_else(|| settings.version.get())
    });
    provide_context(SelectedVersion(version));

    let brand = view! {
        <a
            href=move || ui_link("", version.get())
            class="flex items-baseline gap-2 font-semibold text-slate-900 dark:text-slate-50"
        >
            <span class="text-lg">"FerroTERM"</span>
            <span class="text-xs font-normal text-slate-500 dark:text-slate-400">"viewer"</span>
        </a>
    }
    .into_any();

    let location = use_location();
    let on = move |path: &str| {
        let target = format!("{UI_BASE}{path}");
        location.pathname.get().trim_end_matches('/') == target.trim_end_matches('/')
    };
    let links = view! {
        <nav aria-label="Sections" class="flex items-center gap-3 text-sm">
            <a
                href=move || ui_link("", version.get())
                aria-current=move || if on("") { Some("page") } else { None }
                class="text-slate-700 hover:underline dark:text-slate-200"
            >
                "Overview"
            </a>
            <a
                href=move || ui_link("settings", version.get())
                aria-current=move || if on("/settings") { Some("page") } else { None }
                class="text-slate-700 hover:underline dark:text-slate-200"
            >
                "Settings"
            </a>
        </nav>
    }
    .into_any();

    let status = view! {
        <div class="flex items-center gap-3">
            <VersionSwitcher selected=version />
            <HealthIndicator />
            <ThemeToggle />
        </div>
    }
    .into_any();

    let screens = view! {
        <Routes fallback=NotFoundPage>
            <Route path=path!("/") view=OverviewPage />
            <Route path=path!("/settings") view=SettingsPage />
        </Routes>
    }
    .into_any();

    view! {
        <div class="min-h-screen bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
            <header class="border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
                <div class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-3 px-4 py-3">
                    {brand} {links} {status}
                </div>
            </header>
            <main class="mx-auto max-w-6xl px-4 py-6">{screens}</main>
        </div>
    }
}
