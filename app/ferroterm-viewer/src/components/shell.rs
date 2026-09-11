//! The chrome around every screen, and the routes it wraps.
//!
//! The screens are reached from a left sidebar; the top bar carries what is
//! true of every screen at once, which is the FHIR version the reader is
//! looking through, the server's health, and the theme.

use leptos::ev::SubmitEvent;
use leptos::html::Input;
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::components::Route;
use leptos_router::components::Routes;
use leptos_router::hooks::use_location;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use leptos_router::path;

use crate::components::health::HealthIndicator;
use crate::components::icon;
use crate::components::icon::Glyph;
use crate::components::icon::Icon;
use crate::components::mark::Lockup;
use crate::components::spinner::Spinner;
use crate::components::theme_toggle::ThemeToggle;
use crate::components::version_switcher::VersionSwitcher;
use crate::fhir::version::FhirVersion;
use crate::find::QUERY_PARAM;
use crate::pages::about::AboutPage;
use crate::pages::browse::BrowsePage;
use crate::pages::code_system::CodeSystemPage;
use crate::pages::concept_maps::ConceptMapsPage;
use crate::pages::expand::ExpandPage;
use crate::pages::find::FindPage;
use crate::pages::not_found::NotFoundPage;
use crate::pages::overview::OverviewPage;
use crate::pages::translate::TranslatePage;
use crate::pages::validate::ValidatePage;
use crate::pages::value_sets::ValueSetsPage;
use crate::routes::ABOUT_PATH;
use crate::routes::CONCEPT_MAPS_PATH;
use crate::routes::EXPAND_PATH;
use crate::routes::FIND_PATH;
use crate::routes::OVERVIEW_PATH;
use crate::routes::TRANSLATE_PATH;
use crate::routes::UI_BASE;
use crate::routes::VALIDATE_PATH;
use crate::routes::VALUE_SETS_PATH;
use crate::routes::VERSION_PARAM;
use crate::routes::nav_section;
use crate::routes::ui_link;
use crate::settings::Settings;
use crate::styles;
use crate::url::RequestUrl;

/// The FHIR version the current address selects, for every screen to read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedVersion(pub(crate) Signal<FhirVersion>);

/// The element the navigation toggle opens and closes.
const NAV_ID: &str = "screen-nav";

/// The command bar's own control, which its label points at.
const COMMAND_ID: &str = "command-bar";

/// One screen in the sidebar: its path segment below the base, the label a
/// reader reads, and the glyph they recognise it by.
struct NavItem(&'static str, &'static str, Glyph);

/// One labelled group of the sidebar.
struct NavGroup(&'static str, &'static [NavItem]);

/// The screens a reader reads what this server holds on.
///
/// The concept browser is not among them. It browses one code system, and the
/// address of a browser with no system named is a screen that can only send a
/// reader to the overview, so the overview is where it is reached from.
const EXPLORE: [NavItem; 1] = [NavItem(OVERVIEW_PATH, "Overview", icon::OVERVIEW)];

/// The screens a reader asks this server a question on.
///
/// Each is named for the question rather than the operation that answers it.
/// A reader who wants to know whether a code is valid does not know that
/// `$validate-code` is the one to open.
const RUN: [NavItem; 3] = [
    NavItem(VALIDATE_PATH, "Check a code", icon::VALIDATE),
    NavItem(EXPAND_PATH, "List a value set", icon::EXPAND),
    NavItem(TRANSLATE_PATH, "Map a code", icon::CONCEPT_MAPS),
];

/// The screens that list what this server publishes.
const PUBLISH: [NavItem; 2] = [
    NavItem(VALUE_SETS_PATH, "Value sets", icon::VALUE_SETS),
    NavItem(CONCEPT_MAPS_PATH, "Concept maps", icon::CONCEPT_MAPS),
];

/// The one screen about this server and this viewer, rather than about a code.
const ABOUT: [NavItem; 1] = [NavItem(ABOUT_PATH, "About this server", icon::EVIDENCE)];

/// The sidebar, as the four groups in the order it renders them.
///
/// The order lives here rather than in the markup, so one table decides it and
/// the rendered sidebar cannot drift from it. The groups are what a reader
/// came to do: read what the server holds, ask it something, see what it
/// publishes, or ask about the server itself.
const NAV_GROUPS: [NavGroup; 4] = [
    NavGroup("Explore", &EXPLORE),
    NavGroup("Run", &RUN),
    NavGroup("Publish", &PUBLISH),
    NavGroup("About this server", &ABOUT),
];

/// The classes every sidebar link carries, whichever screen it leads to.
const ENTRY_BASE: &str =
    "flex items-center gap-default rounded-md px-default py-tight text-small font-medium";

/// The classes the entry for the screen being read carries.
const ENTRY_ACTIVE: &str = "bg-accent-soft text-accent-soft-fg";

/// The classes every other entry carries.
const ENTRY_RESTING: &str = "state-change text-muted hover:bg-inset hover:text-fg";

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

/// One labelled group of the sidebar.
///
/// The label is the group's accessible name through `aria-labelledby`, so a
/// screen reader announces which group a link is in rather than reading twelve
/// links in a row (<https://www.w3.org/WAI/ARIA/apg/patterns/landmarks/>).
fn nav_group(
    section: Memo<Option<&'static str>>,
    version: Signal<FhirVersion>,
    group: &'static NavGroup,
) -> AnyView {
    let NavGroup(label, items) = group;
    let id = format!("screen-nav-{}", label.split(' ').next().unwrap_or(label)).to_lowercase();
    let entries: Vec<AnyView> = items
        .iter()
        .map(|NavItem(segment, name, glyph)| nav_entry(section, version, segment, name, *glyph))
        .collect();
    view! {
        <li>
            <p id=id.clone() class=format!("px-default pt-default pb-tight {}", styles::EYEBROW)>
                {*label}
            </p>
            <ul aria-labelledby=id class="flex flex-col gap-tight">
                {entries}
            </ul>
        </li>
    }
    .into_any()
}

/// The sidebar every screen is reached from.
///
/// It is a landmark with its own name, and it stays in the document at every
/// width: below the `md` breakpoint it is hidden until the top bar's toggle
/// opens it, and from `md` up it is always shown. It scrolls on its own, so a
/// reader walking a long screen keeps every place they can go in view.
fn sidebar(version: Signal<FhirVersion>, open: RwSignal<bool>) -> AnyView {
    let location = use_location();
    let section = Memo::new(move |_| location.pathname.with(|path| nav_section(path)));
    let groups: Vec<AnyView> = NAV_GROUPS
        .iter()
        .map(|group| nav_group(section, version, group))
        .collect();
    view! {
        <aside
            id=NAV_ID
            class="w-full shrink-0 overflow-y-auto border-b border-line bg-raised md:block md:w-56 md:border-r md:border-b-0"
            class:hidden=move || !open.get()
        >
            <nav aria-label="Screens" class="p-default">
                <ul class="flex flex-col">{groups}</ul>
            </nav>
        </aside>
    }
    .into_any()
}

/// The command bar, on every screen.
///
/// It is centred in what the top bar has left between the mark and the
/// controls, where a reader looks for it. One field. What a reader types is read by its shape alone, in
/// `crate::find`, and the screen it opens offers what this root can do with
/// it. The bar is a form rather than a listbox that suggests as you type, so
/// every offer is a real link, the keyboard needs no handler of ours, and a
/// reader who has not seen the screen before can send the address of what they
/// found.
fn command_bar(version: Signal<FhirVersion>) -> AnyView {
    let typed: NodeRef<Input> = NodeRef::new();
    let navigate = StoredValue::new(use_navigate());
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        let asked = typed
            .get()
            .map(|input| input.value())
            .unwrap_or_default()
            .trim()
            .to_owned();
        let target = RequestUrl::new()
            .segment(UI_BASE.trim_start_matches('/'))
            .segment(FIND_PATH)
            .query(VERSION_PARAM, version.get().segment())
            .query(QUERY_PARAM, &asked)
            .render("");
        // NOTE: the router resolves a navigation against its base, so an
        // address that already carries the base is passed unresolved
        // (`leptos_router` 0.8.15 `matching/resolve_path.rs`).
        navigate.with_value(|navigate| {
            navigate(
                &target,
                NavigateOptions {
                    resolve: false,
                    ..NavigateOptions::default()
                },
            );
        });
    };
    view! {
        <form
            class="flex min-w-0 flex-1 items-center justify-center gap-default"
            on:submit=submit
            role="search"
        >
            <label for=COMMAND_ID class="sr-only">
                "Find a code, a canonical, or a phrase"
            </label>
            <input
                id=COMMAND_ID
                name="q"
                type="search"
                placeholder="A code, a canonical, or a phrase"
                class=format!("w-full min-w-0 max-w-md {}", styles::INPUT)
                node_ref=typed
            />
            <button type="submit" class=styles::BUTTON>
                <Icon glyph=icon::SEARCH />
                "Find"
            </button>
        </form>
    }
    .into_any()
}

/// The top bar: the mark, the command bar, and the FHIR version.
///
/// The version switcher sits here because it is not a screen a reader goes
/// to: it changes which server root every screen reads from, so it belongs
/// with the chrome that is true everywhere rather than in the list of places.
fn topbar(version: Signal<FhirVersion>, open: RwSignal<bool>) -> AnyView {
    view! {
        <header class="shrink-0 border-b border-line bg-raised">
            <div class="flex flex-wrap items-center gap-default px-loose py-default">
                <button
                    type="button"
                    aria-controls=NAV_ID
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    class=format!("{} md:hidden", styles::BUTTON)
                    on:click=move |_| open.update(|shown| *shown = !*shown)
                >
                    "Screens"
                </button>
                <a href=move || ui_link(OVERVIEW_PATH, version.get())>
                    <Lockup />
                </a>
                {command_bar(version)}
                <div class="flex items-center gap-default">
                    <VersionSwitcher selected=version />
                    <HealthIndicator />
                    <ThemeToggle />
                </div>
            </div>
        </header>
    }
    .into_any()
}

/// Sends an address a pane used to have to the pane it named.
///
/// A replacing navigation rather than a push, so the browser's back button
/// leaves the viewer the way it came in rather than bouncing off the address
/// that redirected (`leptos_router` 0.8.15 `NavigateOptions::replace`).
fn moved_to(pane: &'static str) -> impl IntoView {
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let navigate = StoredValue::new(use_navigate());
    Effect::new(move |_| {
        let target = format!("{}#{pane}", ui_link(ABOUT_PATH, version.get()));
        navigate.with_value(|navigate| {
            navigate(
                &target,
                NavigateOptions {
                    resolve: false,
                    replace: true,
                    ..NavigateOptions::default()
                },
            );
        });
    });
    view! { <Spinner label="Opening the pane this address names" /> }
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
            <Route path=path!("/about") view=AboutPage />
            <Route path=path!("/systems/:url") view=CodeSystemPage />
            <Route path=path!("/conceptmaps") view=ConceptMapsPage />
            <Route path=path!("/translate") view=TranslatePage />
            <Route path=path!("/valuesets") view=ValueSetsPage />
            <Route path=path!("/find") view=FindPage />
            // The three panes were three screens. A link written then still
            // opens the pane it named, which is the fragment each carries.
            <Route path=path!("/versions") view=|| moved_to("about-versions-heading") />
            <Route path=path!("/evidence") view=|| moved_to("about-evidence-heading") />
            <Route path=path!("/settings") view=|| moved_to("about-settings-heading") />
        </Routes>
    }
    .into_any();

    view! {
        <div class=format!(
            "flex h-screen flex-col overflow-hidden {}",
            styles::PAGE,
        )>
            {bar}
            <div class="flex min-h-0 flex-1 flex-col md:flex-row">
                {nav} <main class="min-w-0 flex-1 overflow-y-auto px-loose py-loose">
                    <div class="mx-auto max-w-6xl">{screens}</div>
                </main>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::ABOUT;
    use super::EXPLORE;
    use super::NAV_GROUPS;
    use super::NavItem;
    use crate::routes::UI_BASE;
    use crate::routes::nav_section;

    /// The path segment of every entry, in the order the sidebar draws them.
    fn entries() -> Vec<&'static str> {
        NAV_GROUPS
            .iter()
            .flat_map(|group| group.1.iter().map(|NavItem(segment, _, _)| *segment))
            .collect()
    }

    #[test]
    fn the_sidebar_opens_on_the_overview_and_ends_on_the_server_itself() {
        let listed = entries();
        assert_eq!(
            listed.first().copied(),
            Some(""),
            "the base itself is the first entry: {listed:?}"
        );
        assert_eq!(
            listed.last().copied(),
            Some("about"),
            "the questions about the server come after the questions about a code: {listed:?}"
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
    fn every_group_is_labelled_and_holds_a_screen() {
        for group in &NAV_GROUPS {
            assert!(
                !group.0.is_empty(),
                "a group with no label reads as a run of links"
            );
            assert!(
                !group.1.is_empty(),
                "the `{}` group draws a label over nothing",
                group.0
            );
        }
    }

    #[test]
    fn the_server_facts_are_the_last_group_and_the_reading_screens_the_first() {
        assert_eq!(
            NAV_GROUPS.first().map(|group| group.1.len()),
            Some(EXPLORE.len()),
            "a reader arrives to read what the server holds"
        );
        assert_eq!(
            NAV_GROUPS.last().map(|group| group.1.len()),
            Some(ABOUT.len()),
            "the screens about the server come after the screens about a code"
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
