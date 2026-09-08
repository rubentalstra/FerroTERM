//! The served code systems as one table, one row per served version.
//!
//! A deployment serves twenty systems as readily as five, so the overview is
//! an inventory a reader scans rather than a card each. It carries the five
//! facts a reader picks a system by; the grammar, the designation languages,
//! the filters and the published resource are on the system's own screen, one
//! click from the row.
//!
//! Nothing here names a system, assumes a hierarchy, or assumes a language.
//! Every cell is the capability statement rendered, so a system this server
//! has never served draws correctly with no change to this file.

use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use leptos_router::hooks::use_query_map;

use crate::components::shell::SelectedVersion;
use crate::fhir::concept::Hierarchy;
use crate::fhir::terminology::Artifact;
use crate::fhir::terminology::SystemCard;
use crate::fhir::terminology::VersionRow;
use crate::fhir::version::FhirVersion;
use crate::routes::BROWSE_PATH;
use crate::routes::OVERVIEW_PATH;
use crate::routes::UI_BASE;
use crate::routes::VALIDATE_PATH;
use crate::routes::VERSION_PARAM;
use crate::routes::system_link;
use crate::routes::system_tool_link;
use crate::styles;
use crate::url::RequestUrl;

/// The address parameter naming the column the table is sorted on.
const SORT_PARAM: &str = "sort";

/// The address parameter naming which way that column is sorted.
const DIRECTION_PARAM: &str = "dir";

/// The value that parameter carries for a descending sort.
const DESCENDING: &str = "desc";

/// A column the systems can be ordered by.
///
/// Only the system-level facts sort. A version is a row under its system, so
/// ordering the table by one would break the group it belongs to, and a
/// reader looking for a version is looking inside a system they already found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Column {
    /// The canonical the system is identified by.
    System,
    /// The content mode the server declared for it.
    Content,
    /// Whether the server answers subsumption for it.
    Subsumes,
}

impl Column {
    /// The value the address carries for this column.
    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Content => "content",
            Self::Subsumes => "subsumes",
        }
    }

    /// The heading a reader reads on it.
    fn label(self) -> &'static str {
        match self {
            Self::System => "Code system",
            Self::Content => "Content",
            Self::Subsumes => "Subsumes",
        }
    }

    /// Reads a column out of the address, or `None` for the order the server
    /// declared.
    fn read(text: &str) -> Option<Self> {
        [Self::System, Self::Content, Self::Subsumes]
            .into_iter()
            .find(|column| column.key() == text)
    }

    /// What this column sorts on, as the text it compares.
    ///
    /// A fact the server did not state sorts after every one it did, in both
    /// directions, so absence never lands in the middle of the answers.
    fn of(self, card: &SystemCard) -> (bool, String) {
        match self {
            Self::System => (card.url.is_empty(), card.url.clone()),
            Self::Content => (
                card.content.is_none(),
                card.content.clone().unwrap_or_default(),
            ),
            Self::Subsumes => (
                card.subsumption.is_none(),
                match card.subsumption {
                    Some(true) => "yes".to_owned(),
                    Some(false) => "no".to_owned(),
                    None => String::new(),
                },
            ),
        }
    }
}

/// How the table is ordered right now.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Order {
    /// The column, or `None` for the order the server declared.
    column: Option<Column>,
    /// Whether that column runs the other way.
    descending: bool,
}

impl Order {
    /// Reads the order out of the address.
    fn read(query: &dyn Fn(&str) -> Option<String>) -> Self {
        Self {
            column: query(SORT_PARAM)
                .as_deref()
                .map(str::trim)
                .and_then(Column::read),
            descending: query(DIRECTION_PARAM).as_deref().map(str::trim) == Some(DESCENDING),
        }
    }

    /// The order a click on `column` asks for.
    ///
    /// A first click sorts the column upward, and a second turns it around, so
    /// the two ends of a column are one click apart.
    fn toggled(self, column: Column) -> Self {
        Self {
            column: Some(column),
            descending: self.column == Some(column) && !self.descending,
        }
    }

    /// What a screen reader announces about `column` on this order.
    fn announced(self, column: Column) -> &'static str {
        if self.column != Some(column) {
            "none"
        } else if self.descending {
            "descending"
        } else {
            "ascending"
        }
    }

    /// The address this order is, on the overview of `version`.
    fn address(self, version: FhirVersion) -> String {
        let mut url = RequestUrl::new().segment(UI_BASE.trim_start_matches('/'));
        for part in OVERVIEW_PATH.split('/').filter(|part| !part.is_empty()) {
            url = url.segment(part);
        }
        url = url.query(VERSION_PARAM, version.segment());
        if let Some(column) = self.column {
            url = url.query(SORT_PARAM, column.key());
            if self.descending {
                url = url.query(DIRECTION_PARAM, DESCENDING);
            }
        }
        url.render("")
    }
}

/// The systems in the order the address asks for.
fn ordered(mut cards: Vec<SystemCard>, order: Order) -> Vec<SystemCard> {
    let Some(column) = order.column else {
        return cards;
    };
    // A stable sort, so systems that compare equal keep the order the server
    // declared them in rather than an order this function invented. Absence is
    // compared first and never reversed, which is what keeps a fact the server
    // did not state at the foot of the table in both directions.
    cards.sort_by(|left, right| {
        let (left_absent, left_key) = column.of(left);
        let (right_absent, right_key) = column.of(right);
        left_absent.cmp(&right_absent).then_with(|| {
            let ordering = left_key.cmp(&right_key);
            if order.descending {
                ordering.reverse()
            } else {
                ordering
            }
        })
    });
    cards
}

/// The served systems, one row per version.
#[component]
#[expect(
    unreachable_pub,
    reason = "the leptos component macro emits a pub props type, and a binary crate has no reachable public API"
)]
pub(crate) fn SystemTable(
    /// The systems to draw, as the capability statement declared them.
    cards: Vec<SystemCard>,
) -> impl IntoView {
    let SelectedVersion(version) = expect_context::<SelectedVersion>();
    let query = use_query_map();
    let order = Memo::new(move |_| query.with(|map| Order::read(&|name| map.get(name))));
    // The rows are a closure over the order, not a value read once: a sort
    // is a navigation onto this same route, which `leptos_router` 0.8.15
    // answers by updating the query without re-running this body
    // (`src/nested_router.rs`, the same-route-id branch).
    let declared = StoredValue::new(cards);
    let rows = move || {
        declared
            .with_value(|cards| ordered(cards.clone(), order.get()))
            .into_iter()
            .flat_map(|card| system_rows(&card, version))
            .collect::<Vec<AnyView>>()
    };
    view! {
        <div class=format!("mt-default overflow-x-auto {}", styles::PANEL)>
            <table class=styles::TABLE>
                <thead>
                    <tr>
                        {sortable(Column::System, order, version)} <th scope="col" class=styles::TH>
                            "Version"
                        </th> {sortable(Column::Content, order, version)}
                        {sortable(Column::Subsumes, order, version)}
                        <th scope="col" class=styles::TH>
                            "Read from"
                        </th> <th scope="col" class=styles::TH>
                            "Open in"
                        </th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

/// One heading a reader can order the table by.
///
/// `aria-sort` on the header cell is what a screen reader announces, and the
/// control inside it is a real button, so the column is reordered from the
/// keyboard with no key handler written here
/// (<https://www.w3.org/WAI/ARIA/apg/patterns/table/>).
fn sortable(column: Column, order: Memo<Order>, version: Signal<FhirVersion>) -> AnyView {
    let navigate = StoredValue::new(use_navigate());
    let click = move |_| {
        let target = order.get().toggled(column).address(version.get());
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
    let mark = move || match (order.get().column == Some(column), order.get().descending) {
        (false, _) => "",
        (true, false) => " \u{2191}",
        (true, true) => " \u{2193}",
    };
    view! {
        <th scope="col" class=styles::TH aria-sort=move || order.get().announced(column)>
            <button type="button" class="state-change cursor-pointer hover:text-fg" on:click=click>
                {column.label()}
                {mark}
            </button>
        </th>
    }
    .into_any()
}

/// Every row one system contributes: one per served version, or one saying the
/// server declared none.
fn system_rows(card: &SystemCard, version: Signal<FhirVersion>) -> Vec<AnyView> {
    let content = card.content.clone();
    let subsumption = card.subsumption;
    if card.versions.is_empty() {
        return vec![row(
            card,
            version,
            None,
            content.as_deref(),
            subsumption,
            true,
        )];
    }
    card.versions
        .iter()
        .enumerate()
        .map(|(index, served)| {
            row(
                card,
                version,
                Some(served),
                content.as_deref(),
                subsumption,
                index == 0,
            )
        })
        .collect()
}

/// One row: the system on its first, an indent mark on the rest.
fn row(
    card: &SystemCard,
    version: Signal<FhirVersion>,
    served: Option<&VersionRow>,
    content: Option<&str>,
    subsumption: Option<bool>,
    first: bool,
) -> AnyView {
    let system = system_cell(card, version, first);
    let code = served.and_then(|served| served.code.clone());
    let default = served.is_some_and(|served| served.is_default);
    let artifact = served.and_then(|served| served.artifact.clone());
    view! {
        <tr>
            <td class=styles::TD>{system}</td>
            <td class=styles::TD_TIGHT>{version_cell(code, default)}</td>
            <td class=styles::TD_TIGHT>{word_cell(content.map(str::to_owned))}</td>
            <td class=styles::TD_TIGHT>{yes_no_cell(subsumption)}</td>
            <td class=styles::TD>{artifact_cell(artifact.as_ref())}</td>
            <td class=styles::TD_TIGHT>{tools_cell(card, version, served)}</td>
        </tr>
    }
    .into_any()
}

/// The screens this version can be opened in.
///
/// A row hands its subject on rather than making a reader retype a canonical.
/// The browser is offered only where the version declares the direct-child
/// operator, because that is the one operator a tree walks
/// (<https://hl7.org/fhir/R5/codesystem-filter-operator.html>).
fn tools_cell(
    card: &SystemCard,
    version: Signal<FhirVersion>,
    served: Option<&VersionRow>,
) -> AnyView {
    if card.url.is_empty() {
        return absent("a screen needs the canonical this system was declared without");
    }
    let code = served.and_then(|served| served.code.clone());
    let walkable = served.is_some_and(|served| Hierarchy::of(served).walk().is_some());
    let link = |path: &'static str, label: &'static str| {
        let system = card.url.clone();
        let code = code.clone();
        let href = move || system_tool_link(path, &system, code.as_deref(), version.get());
        view! {
            <a href=href class=styles::BUTTON_QUIET>
                {label}
            </a>
        }
        .into_any()
    };
    let browse = walkable.then(|| link(BROWSE_PATH, "Browse"));
    view! { <span class="flex gap-tight">{browse} {link(VALIDATE_PATH, "Validate")}</span> }
        .into_any()
}

/// The system's canonical, as the link onto its screen, once per system.
fn system_cell(card: &SystemCard, version: Signal<FhirVersion>, first: bool) -> AnyView {
    if !first {
        return absent("the version above belongs to the same code system");
    }
    if card.url.is_empty() {
        return view! {
            <span class=styles::CODE_MUTED>
                "This server declared a code system without a canonical URI."
            </span>
        }
        .into_any();
    }
    let target = card.url.clone();
    let href = move || system_link(&target, version.get());
    let label = card.url.clone();
    view! {
        <a href=href class=format!("{} {}", styles::CODE, styles::LINK)>
            {label}
        </a>
    }
    .into_any()
}

/// The served version, with the default marked.
fn version_cell(code: Option<String>, default: bool) -> AnyView {
    let Some(code) = code.filter(|code| !code.is_empty()) else {
        return absent("this server declared no version for the code system");
    };
    let mark = default.then(|| view! { <span class=styles::BADGE>"default"</span> }.into_any());
    view! {
        <span class=styles::CODE>{code}</span>
        {mark}
    }
    .into_any()
}

/// A word the server stated, or the mark for one it did not.
fn word_cell(word: Option<String>) -> AnyView {
    match word.filter(|word| !word.is_empty()) {
        Some(word) => view! { <span>{word}</span> }.into_any(),
        None => absent("this FHIR version has no such element, or the server declared none"),
    }
}

/// A flag the server stated, or the mark for one it did not.
fn yes_no_cell(flag: Option<bool>) -> AnyView {
    match flag {
        Some(true) => view! { <span>"yes"</span> }.into_any(),
        Some(false) => view! { <span class=styles::ABSENT>"no"</span> }.into_any(),
        None => absent("the server declared neither yes nor no"),
    }
}

/// The artifact a version was read from.
///
/// A system the deployment built no index for, such as a registry the server
/// carries or a `CodeSystem` posted through the API, was read from none. Only
/// the artifact's own name is on the wire, so no path is shown.
fn artifact_cell(artifact: Option<&Artifact>) -> AnyView {
    let Some(artifact) = artifact else {
        return absent("this version was not read from a built artifact");
    };
    let name = artifact.name.clone();
    let release = artifact.release.clone();
    let release_line =
        release.map(|release| view! { <span class=styles::HINT>{release}</span> }.into_any());
    match name {
        Some(name) => view! {
            <span class=format!("block {}", styles::CODE)>{name}</span>
            {release_line}
        }
        .into_any(),
        None => absent("the server named no artifact for this version"),
    }
}

/// The mark for a fact the server did not state, with the sentence behind it.
///
/// A cell that spells absence out in prose makes "not" the densest word on the
/// screen; the reader who wants the sentence hovers or focuses the mark.
fn absent(why: &'static str) -> AnyView {
    view! {
        <span class=styles::ABSENT title=why>
            {styles::ABSENT_MARK}
        </span>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One card carrying only the facts the order compares.
    fn card(url: &str, content: Option<&str>, subsumption: Option<bool>) -> SystemCard {
        SystemCard {
            url: url.to_owned(),
            content: content.map(str::to_owned),
            subsumption,
            versions: Vec::new(),
        }
    }

    /// The three cards every case below orders.
    fn served() -> Vec<SystemCard> {
        vec![
            card("urn:b", Some("complete"), Some(false)),
            card("urn:a", None, Some(true)),
            card("urn:c", Some("not-present"), None),
        ]
    }

    /// The canonicals of an ordered answer, in the order it holds them.
    fn urls(cards: &[SystemCard]) -> Vec<&str> {
        cards.iter().map(|card| card.url.as_str()).collect()
    }

    #[test]
    fn an_address_naming_no_column_keeps_the_order_the_server_declared() {
        let order = Order::read(&|_| None);
        assert_eq!(order, Order::default());
        assert_eq!(
            urls(&ordered(served(), order)),
            ["urn:b", "urn:a", "urn:c"],
            "the server's own order is an answer, not an absence of one"
        );
    }

    #[test]
    fn a_column_orders_the_systems_and_turns_around_on_the_second_click() {
        let up = Order::default().toggled(Column::System);
        assert_eq!(urls(&ordered(served(), up)), ["urn:a", "urn:b", "urn:c"]);
        let down = up.toggled(Column::System);
        assert!(down.descending, "a second click on one column turns it");
        assert_eq!(urls(&ordered(served(), down)), ["urn:c", "urn:b", "urn:a"]);
    }

    #[test]
    fn moving_to_another_column_starts_it_upward() {
        let down = Order::default()
            .toggled(Column::System)
            .toggled(Column::System);
        let moved = down.toggled(Column::Content);
        assert_eq!(moved.column, Some(Column::Content));
        assert!(
            !moved.descending,
            "a column a reader has not sorted yet starts upward"
        );
    }

    #[test]
    fn a_fact_the_server_did_not_state_sorts_after_every_one_it_did() {
        let up = Order::default().toggled(Column::Content);
        assert_eq!(
            urls(&ordered(served(), up)).last().copied(),
            Some("urn:a"),
            "the system with no content mode sorts last going up"
        );
        let down = up.toggled(Column::Content);
        assert_eq!(
            urls(&ordered(served(), down)).last().copied(),
            Some("urn:a"),
            "and last going down, so absence never lands among the answers"
        );
    }

    #[test]
    fn the_order_round_trips_through_the_address() {
        for column in [Column::System, Column::Content, Column::Subsumes] {
            for descending in [false, true] {
                let order = Order {
                    column: Some(column),
                    descending,
                };
                let address = order.address(FhirVersion::default());
                let read = Order::read(&|name| {
                    let query = address
                        .split_once('?')
                        .expect("the address carries a query")
                        .1;
                    query.split('&').find_map(|pair| {
                        let (key, value) = pair.split_once('=')?;
                        (key == name).then(|| value.to_owned())
                    })
                });
                assert_eq!(read, order, "{} must survive the link", column.key());
            }
        }
    }

    #[test]
    fn a_screen_reader_is_told_which_column_is_sorted_and_which_way() {
        let order = Order::default().toggled(Column::Subsumes);
        assert_eq!(order.announced(Column::Subsumes), "ascending");
        assert_eq!(order.announced(Column::System), "none");
        assert_eq!(
            order.toggled(Column::Subsumes).announced(Column::Subsumes),
            "descending"
        );
    }
}
