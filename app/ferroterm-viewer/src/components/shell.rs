//! The chrome around every screen, and the routes it wraps.
//!
//! The screens are reached from a left sidebar; the top bar carries what is
//! true of every screen at once, which is the FHIR version the reader is
//! looking through, the server's health, and the theme.

use leptos::prelude::*;
use leptos_router::components::Route;
use leptos_router::components::Routes;
use leptos_router::hooks::use_location;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use leptos_router::path;

use crate::components::health::HealthIndicator;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::components::theme_toggle::ThemeToggle;
use crate::components::version_switcher::VersionSwitcher;
use crate::fhir::version::FhirVersion;
use crate::pages::browse::BrowsePage;
use crate::pages::code_system::CodeSystemPage;
use crate::pages::concept_maps::ConceptMapsPage;
use crate::pages::evidence::EvidencePage;
use crate::pages::expand::ExpandPage;
use crate::pages::not_found::NotFoundPage;
use crate::pages::overview::OverviewPage;
use crate::pages::settings::SettingsPage;
use crate::pages::validate::ValidatePage;
use crate::pages::value_sets::ValueSetsPage;
use crate::pages::versions::VersionsPage;
use crate::routes::BROWSE_PATH;
use crate::routes::CONCEPT_MAPS_PATH;
use crate::routes::EVIDENCE_PATH;
use crate::routes::EXPAND_PATH;
use crate::routes::OVERVIEW_PATH;
use crate::routes::SETTINGS_PATH;
use crate::routes::VALIDATE_PATH;
use crate::routes::VALUE_SETS_PATH;
use crate::routes::VERSIONS_PATH;
use crate::routes::nav_section;
use crate::routes::ui_link;
use crate::settings::Settings;

/// The FHIR version the current address selects, for every screen to read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedVersion(pub(crate) Signal<FhirVersion>);

/// The element the navigation toggle opens and closes.
const NAV_ID: &str = "screen-nav";

/// One slot of the sidebar, in the order it is drawn.
enum NavSlot {
    /// A screen: its path segment below the base, its label, and the glyph a
    /// reader recognises it by.
    Item(&'static str, &'static str, Glyph),
    /// The hairline between the two groups.
    Divider,
}

/// The sidebar, in the order it renders: the screens over what the server
/// serves, a divider, then the screens about the viewer itself.
///
/// The order lives here rather than in the markup, so one table decides it and
/// the rendered sidebar cannot drift from it. The divider belongs to the
/// second group, whose entries are the ones a new screen is least likely to
/// land between.
const NAV_SLOTS: [NavSlot; 10] = [
    NavSlot::Item(OVERVIEW_PATH, "Overview", crate::components::icon::OVERVIEW),
    NavSlot::Item(BROWSE_PATH, "Browse", crate::components::icon::BROWSE),
    NavSlot::Item(EXPAND_PATH, "Expand", crate::components::icon::EXPAND),
    NavSlot::Item(VALIDATE_PATH, "Validate", crate::components::icon::VALIDATE),
    NavSlot::Item(
        VALUE_SETS_PATH,
        "Value sets",
        crate::components::icon::VALUE_SETS,
    ),
    NavSlot::Item(
        CONCEPT_MAPS_PATH,
        "Concept maps",
        crate::components::icon::CONCEPT_MAPS,
    ),
    NavSlot::Divider,
    NavSlot::Item(
        VERSIONS_PATH,
        "FHIR versions",
        crate::components::icon::VERSION,
    ),
    NavSlot::Item(EVIDENCE_PATH, "Evidence", crate::components::icon::EVIDENCE),
    NavSlot::Item(SETTINGS_PATH, "Settings", crate::components::icon::SETTINGS),
];

/// The classes every sidebar link carries, whichever screen it leads to.
const ENTRY_BASE: &str = "flex items-center rounded-md px-3 py-2 text-sm font-medium";

/// The classes the entry for the screen being read carries.
const ENTRY_ACTIVE: &str = "bg-brand-50 text-brand-700 dark:bg-brand-900/40 dark:text-brand-100";

/// The classes every other entry carries.
const ENTRY_RESTING: &str = "text-slate-700 hover:bg-slate-200 hover:text-slate-900 \
                             dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-slate-50";

/// The shell's own query parameters.
#[derive(Clone, Debug, Params, PartialEq)]
struct ShellQuery {
    /// The FHIR version, as its path segment.
    fhir: Option<String>,
}

/// One sidebar `<li>`: the link, the mark on the screen being read, and the
/// styling of both.
///
/// Every entry draws through this one function, so two of them cannot drift
/// apart and only one place decides what a sidebar link looks like. Only the
/// path decides which entry is marked: the FHIR version travels in the query
/// and every link carries it.
fn nav_entry(
    section: Memo<Option<&'static str>>,
    version: Signal<FhirVersion>,
    segment: &'static str,
    label: &'static str,
    glyph: Glyph,
) -> AnyView {
    let active = move || section.get() == Some(segment);
    view! {
        <li>
            <a
                href=move || ui_link(segment, version.get())
                aria-current=move || active().then_some("page")
                class=move || {
                    let state = if active() { ENTRY_ACTIVE } else { ENTRY_RESTING };
                    format!("{ENTRY_BASE} {state}")
                }
            >
                <Icon glyph=glyph />
                {label}
            </a>
        </li>
    }
    .into_any()
}

/// The hairline between the sidebar's two groups.
///
/// It draws a grouping the labels already carry, so it says nothing to a
/// screen reader and is hidden from one.
fn nav_divider() -> AnyView {
    view! { <li aria-hidden="true" class="my-2 border-t border-slate-200 dark:border-slate-800"></li> }
        .into_any()
}

/// The sidebar every screen is reached from.
///
/// It is a landmark with its own name, and it stays in the document at every
/// width: below the `md` breakpoint it is hidden until the top bar's toggle
/// opens it, and from `md` up it is always shown.
fn sidebar(version: Signal<FhirVersion>, open: RwSignal<bool>) -> AnyView {
    let location = use_location();
    let section = Memo::new(move |_| location.pathname.with(|path| nav_section(path)));
    let slots: Vec<AnyView> = NAV_SLOTS
        .into_iter()
        .map(|slot| match slot {
            NavSlot::Item(segment, label, glyph) => {
                nav_entry(section, version, segment, label, glyph)
            }
            NavSlot::Divider => nav_divider(),
        })
        .collect();
    view! {
        <aside
            id=NAV_ID
            class="w-full shrink-0 border-b border-slate-200 bg-white md:block md:w-56 md:border-r md:border-b-0 dark:border-slate-800 dark:bg-slate-900"
            class:hidden=move || !open.get()
        >
            <nav aria-label="Screens" class="p-3">
                <ul class="flex flex-col gap-1">{slots}</ul>
            </nav>
        </aside>
    }
    .into_any()
}

/// The top bar: the wordmark, the navigation toggle, and the FHIR version.
///
/// The version switcher sits here because it is not a screen a reader goes
/// to: it changes which server root every screen reads from, so it belongs
/// with the chrome that is true everywhere rather than in the list of places.
fn topbar(version: Signal<FhirVersion>, open: RwSignal<bool>) -> AnyView {
    view! {
        <header class="border-b border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <div class="flex flex-wrap items-center gap-3 px-4 py-3">
                <button
                    type="button"
                    aria-controls=NAV_ID
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    class="rounded-md border border-slate-300 px-2 py-1 text-sm font-medium text-slate-700 md:hidden dark:border-slate-700 dark:text-slate-200"
                    on:click=move |_| open.update(|shown| *shown = !*shown)
                >
                    "Screens"
                </button>
                <a
                    href=move || ui_link(OVERVIEW_PATH, version.get())
                    class="flex items-baseline gap-2 font-semibold text-slate-900 dark:text-slate-50"
                >
                    <span class="text-lg">"FerroTERM"</span>
                    <span class="text-xs font-normal text-slate-500 dark:text-slate-400">
                        "viewer"
                    </span>
                </a>
                <div class="ml-auto flex items-center gap-3">
                    <VersionSwitcher selected=version />
                    <HealthIndicator />
                    <ThemeToggle />
                </div>
            </div>
        </header>
    }
    .into_any()
}

/// The chrome, the routed screen, and the sidebar beside it.
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

    let nav_open = RwSignal::new(false);
    let bar = topbar(version, nav_open);
    let nav = sidebar(version, nav_open);

    let screens = view! {
        <Routes fallback=NotFoundPage>
            <Route path=path!("/") view=OverviewPage />
            <Route path=path!("/browse") view=BrowsePage />
            <Route path=path!("/expand") view=ExpandPage />
            <Route path=path!("/validate") view=ValidatePage />
            <Route path=path!("/settings") view=SettingsPage />
            <Route path=path!("/systems/:url") view=CodeSystemPage />
            <Route path=path!("/conceptmaps") view=ConceptMapsPage />
            <Route path=path!("/valuesets") view=ValueSetsPage />
            <Route path=path!("/versions") view=VersionsPage />
            <Route path=path!("/evidence") view=EvidencePage />
        </Routes>
    }
    .into_any();

    view! {
        <div class="flex min-h-screen flex-col bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
            {bar}
            <div class="flex flex-1 flex-col md:flex-row">
                {nav} <main class="min-w-0 flex-1 px-4 py-6">
                    <div class="mx-auto max-w-6xl">{screens}</div>
                </main>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::NAV_SLOTS;
    use super::NavSlot;
    use crate::routes::UI_BASE;
    use crate::routes::nav_section;

    /// The path segment of every entry, in the order the sidebar draws them.
    fn entries() -> Vec<&'static str> {
        NAV_SLOTS
            .iter()
            .filter_map(|slot| match slot {
                NavSlot::Item(segment, _, _) => Some(*segment),
                NavSlot::Divider => None,
            })
            .collect()
    }

    #[test]
    fn the_sidebar_opens_on_the_overview_and_ends_on_settings() {
        let listed = entries();
        assert_eq!(
            listed.first().copied(),
            Some(""),
            "the base itself is the first entry: {listed:?}"
        );
        assert_eq!(
            listed.last().copied(),
            Some("settings"),
            "what a reader sets for themselves is the last entry: {listed:?}"
        );
    }

    #[test]
    fn no_two_entries_lead_to_the_same_screen() {
        let listed = entries();
        for (index, segment) in listed.iter().enumerate() {
            assert!(
                !listed.iter().skip(index + 1).any(|other| other == segment),
                "two entries lead to `{segment}`: {listed:?}"
            );
        }
    }

    #[test]
    fn the_divider_separates_two_groups_it_never_ends() {
        let dividers = NAV_SLOTS
            .iter()
            .filter(|slot| matches!(slot, NavSlot::Divider))
            .count();
        assert_eq!(
            dividers, 1,
            "one hairline, so the sidebar reads as two groups"
        );
        assert!(
            !matches!(NAV_SLOTS.first(), Some(NavSlot::Divider)),
            "a hairline at either end draws a group with nothing in it"
        );
        assert!(
            !matches!(NAV_SLOTS.last(), Some(NavSlot::Divider)),
            "a hairline at either end draws a group with nothing in it"
        );
    }

    #[test]
    fn every_entry_marks_itself_when_a_reader_is_on_it() {
        for segment in entries() {
            let address = format!("{UI_BASE}/{segment}");
            assert_eq!(
                nav_section(&address),
                Some(segment),
                "the entry for `{address}` marks some other screen"
            );
        }
    }
}
