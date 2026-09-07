//! The chrome around every screen, and the routes it wraps.

use leptos::prelude::*;
use leptos_router::components::Route;
use leptos_router::components::Routes;
use leptos_router::hooks::use_location;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use leptos_router::path;

use crate::components::health::HealthIndicator;
use crate::components::icon;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::components::theme_toggle::ThemeToggle;
use crate::components::version_switcher::VersionSwitcher;
use crate::fhir::version::FhirVersion;
use crate::pages::browse::BrowsePage;
use crate::pages::code_system::CodeSystemPage;
use crate::pages::concept_maps::ConceptMapsPage;
use crate::pages::expand::ExpandPage;
use crate::pages::not_found::NotFoundPage;
use crate::pages::overview::OverviewPage;
use crate::pages::settings::SettingsPage;
use crate::pages::value_sets::ValueSetsPage;
use crate::pages::versions::VersionsPage;
use crate::routes::UI_BASE;
use crate::routes::ui_link;
use crate::settings::Settings;

/// The FHIR version the current address selects, for every screen to read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedVersion(pub(crate) Signal<FhirVersion>);

/// The screens the header links to, each as its path segment, its label, and
/// the glyph a reader recognises it by.
///
/// The overview sits at the base itself, so its segment is empty.
const SECTIONS: [(&str, &str, Glyph); 7] = [
    ("", "Overview", icon::OVERVIEW),
    ("browse", "Browse", icon::BROWSE),
    ("expand", "Expand", icon::EXPAND),
    ("valuesets", "Value sets", icon::VALUE_SETS),
    ("conceptmaps", "Concept maps", icon::CONCEPT_MAPS),
    ("versions", "FHIR versions", icon::VERSION),
    ("settings", "Settings", icon::SETTINGS),
];

/// The shell's own query parameters.
#[derive(Clone, Debug, Params, PartialEq)]
struct ShellQuery {
    /// The FHIR version, as its path segment.
    fhir: Option<String>,
}

/// The header's navigation, with the reader's own screen marked.
///
/// One link is written once and drawn per section, so the six entries cost one
/// view rather than six near-identical ones. Only the path is compared: the
/// FHIR version travels in the query and every link carries it.
fn section_links(version: Signal<FhirVersion>) -> AnyView {
    let location = use_location();
    let link = move |segment: &'static str, label: &'static str, glyph: Glyph| {
        let target = if segment.is_empty() {
            UI_BASE.to_owned()
        } else {
            format!("{UI_BASE}/{segment}")
        };
        let here = location.clone();
        view! {
            <a
                href=move || ui_link(segment, version.get())
                aria-current=move || {
                    (here.pathname.get().trim_end_matches('/') == target.trim_end_matches('/'))
                        .then_some("page")
                }
                class="inline-flex items-center gap-1 text-slate-700 hover:underline dark:text-slate-200"
            >
                <Icon glyph=glyph />
                {label}
            </a>
        }
        .into_any()
    };
    let items: Vec<AnyView> = SECTIONS
        .into_iter()
        .map(|(segment, label, glyph)| link(segment, label, glyph))
        .collect();
    view! {
        <nav aria-label="Sections" class="flex items-center gap-3 text-sm">
            {items}
        </nav>
    }
    .into_any()
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

    let links = section_links(version);

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
            <Route path=path!("/browse") view=BrowsePage />
            <Route path=path!("/expand") view=ExpandPage />
            <Route path=path!("/settings") view=SettingsPage />
            <Route path=path!("/systems/:url") view=CodeSystemPage />
            <Route path=path!("/conceptmaps") view=ConceptMapsPage />
            <Route path=path!("/valuesets") view=ValueSetsPage />
            <Route path=path!("/versions") view=VersionsPage />
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
