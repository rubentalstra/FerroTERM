// SPDX-License-Identifier: BUSL-1.1
//! Journeys through the viewer the server serves at `/ui`.

use thirtyfour::prelude::*;
use thirtyfour::stringmatch::StringMatch;

use crate::harness::Journey;
use crate::harness::server;
use crate::harness::session;

/// The version switcher, found by the landmark a screen reader announces.
const SWITCHER: &str = "nav[aria-label='FHIR version']";

/// The switcher link marked as the page the reader is on.
const SELECTED_VERSION: &str = "nav[aria-label='FHIR version'] a[aria-current='page']";

/// The sidebar the screens are reached from, by the landmark it announces.
const SIDEBAR: &str = "nav[aria-label='Screens']";

/// The sidebar entry marked as the screen the reader is on.
const CURRENT_SCREEN: &str = "nav[aria-label='Screens'] a[aria-current='page']";

/// The overview screen's rendered FHIR base, found by the term beside it.
const RENDERED_BASE: &str = "//dt[normalize-space()='FHIR base']/following-sibling::dd[1]";

/// A code system card's heading link on the overview.
const SYSTEM_LINK: &str = "a[href^='/ui/systems/']";

/// The code system screen's capability pane, by the heading it is labelled by.
const CAPABILITY_PANE: &str = "section[aria-labelledby='system-capability-heading']";

/// The code system screen's published-resource pane.
const PUBLISHED_PANE: &str = "section[aria-labelledby='system-published-heading']";

/// The links a declared code system offers into the other screens.
const SYSTEM_TOOLS: &str = "nav[aria-label^='Screens for']";

/// A code system card whose version declares the direct-child operator.
///
/// The tree is drawn only for such a version, so the card is chosen by what
/// the capability statement declares rather than by which system it names. The
/// operator list folds into a `<details>`, and matching on the whole card's
/// text reads what a collapsed element still holds.
const WALKABLE_CARD: &str = "//article[contains(., 'child-of')]//h3//a";

/// The link from a code system's screen into the concept browser.
const BROWSE_LINK: &str = "nav[aria-label^='Screens for'] a[href*='/ui/browse']";

/// A concept the search answered, as the link that reads it.
const SEARCH_RESULT: &str = "section[aria-labelledby='browse-search-heading'] ul li a";

/// The tree once its first level has been answered: a row, or the sentence
/// saying the server answered no child.
///
/// The sentence is drawn from the answer rather than from the address, so its
/// arrival is what separates a concept with no children from a level that has
/// not been read yet.
const TREE_SETTLED: &str = "li[role='treeitem'], #browse-tree-empty";

/// One row of the tree.
const TREE_ROW: &str = "li[role='treeitem']";

/// The row that holds the tree's one tab stop.
const TAB_STOP: &str = "li[role='treeitem'][tabindex='0']";

/// The row the tree announces as selected.
const SELECTED_ROW: &str = "li[role='treeitem'][aria-selected='true']";

/// A parent of the concept being read, as the link that moves onto it.
const PARENT_LINK: &str = "nav[aria-label='Parents of this concept'] a";

/// The sidebar entry onto the version comparison.
const VERSIONS_LINK: &str = "nav[aria-label='Screens'] a[href^='/ui/versions']";

/// The version comparison, by the heading it is labelled by.
const COMPARISON: &str = "section[aria-labelledby='comparison-heading']";

/// A switcher link, by the version name a reader reads on it.
fn version_link(label: &str) -> String {
    format!("//nav[@aria-label='FHIR version']//a[normalize-space()='{label}']")
}

/// One root's cell on the `CodeSystem/$lookup` row, counting from the left.
///
/// The columns are the roots in release order, so the first cell is R4's and
/// the third is R5's.
fn lookup_cell(column: u8) -> String {
    format!("//th[normalize-space()='CodeSystem/$lookup']/following-sibling::td[{column}]")
}

/// A sidebar entry, by the label a reader reads on it.
fn screen_link(label: &str) -> String {
    format!("//nav[@aria-label='Screens']//a[normalize-space()='{label}']")
}

/// The sidebar reaches another screen and marks the one it reached.
///
/// The mark is the `aria-current="page"` a screen reader announces, and it is
/// computed from the address rather than from the click, so a reader who typed
/// the address gets the same mark. The click also proves the entry carries the
/// FHIR version the reader was already on.
#[tokio::test]
async fn the_sidebar_reaches_another_screen_and_marks_the_one_it_reached() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;

            journey
                .element(By::Css(SIDEBAR), "the sidebar the screens are reached from")
                .await;
            journey
                .text_becoming(
                    By::Css(CURRENT_SCREEN),
                    "Overview",
                    "the sidebar to mark the overview the reader opened",
                )
                .await;

            journey
                .element(By::XPath(screen_link("Value sets")), "the value sets entry")
                .await
                .click()
                .await?;

            journey
                .text_becoming(
                    By::Css(CURRENT_SCREEN),
                    "Value sets",
                    "the sidebar to mark value sets after the click",
                )
                .await;
            let address = journey
                .address_carrying("/ui/valuesets", "the value sets screen's own address")
                .await;
            assert!(
                address.contains("fhir=r4b"),
                "a sidebar entry keeps the version the reader was on: `{address}`"
            );
            assert_eq!(
                journey.count(By::Css(CURRENT_SCREEN)).await,
                1,
                "one entry is the screen the reader is on"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// The shell renders under `/ui`, and the switcher moves the whole page onto
/// another FHIR version.
///
/// This is the journey that proves the bundle the server embeds boots at all:
/// the switcher exists only once the WebAssembly module has mounted the
/// application, and the rendered FHIR base below it is read from the same
/// signal the address carries.
#[tokio::test]
async fn the_shell_renders_and_the_switcher_moves_the_page_onto_another_version() {
    let Some(base) = server() else {
        return;
    };
    // The session is quit whether the body returns, errors, or panics; a
    // session left behind holds the browser's only slot, and the next journey
    // then queues behind it until the grid reclaims it.
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;

            journey
                .element(By::Css(SWITCHER), "the FHIR version switcher")
                .await;
            journey
                .text_becoming(
                    By::Css(SELECTED_VERSION),
                    "R4B",
                    "the switcher to mark R4B, the default version, as current",
                )
                .await;
            let base_before = journey
                .text_becoming(
                    By::XPath(RENDERED_BASE),
                    StringMatch::new("/r4b").partial(),
                    "the overview to render the R4B FHIR base",
                )
                .await;
            let address_before = journey.address().await;
            assert!(
                base_before.ends_with("/r4b"),
                "the rendered FHIR base is the R4B root, not `{base_before}`"
            );
            assert!(
                !address_before.contains("fhir=r5"),
                "the journey starts off R5, and the address says `{address_before}`"
            );

            journey
                .element(By::XPath(version_link("R5")), "the R5 link")
                .await
                .click()
                .await?;

            journey
                .text_becoming(
                    By::Css(SELECTED_VERSION),
                    "R5",
                    "the switcher to mark R5 as current after the click",
                )
                .await;
            let base_after = journey
                .text_becoming(
                    By::XPath(RENDERED_BASE),
                    StringMatch::new("/r5").partial(),
                    "the overview to render the R5 FHIR base",
                )
                .await;
            let address_after = journey.address().await;
            assert!(
                base_after.ends_with("/r5"),
                "the rendered FHIR base moved to the R5 root, not to `{base_after}`"
            );
            assert_ne!(
                base_before, base_after,
                "switching version re-renders the page, so the FHIR base is not the one it was"
            );
            assert!(
                address_after.contains("fhir=r5"),
                "the switcher is a navigation, so the address carries the version: `{address_after}`"
            );
            assert_ne!(
                address_before, address_after,
                "a switched version is shareable, so the address changed with the page"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// A card on the overview opens that code system's own screen, and both panes
/// draw from the canonical the route carried.
///
/// This is the journey that proves the percent-encoded canonical survives the
/// route: the detail screen finds the system in the capability statement only
/// if the segment it was linked with decoded back to the canonical the card
/// named. The links row exists only for a system the capabilities declare.
#[tokio::test]
async fn a_card_opens_the_code_system_screen_and_both_panes_draw() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;

            let card = journey
                .element(By::Css(SYSTEM_LINK), "a code system link on the overview")
                .await;
            let canonical = card.text().await?;
            assert!(
                !canonical.is_empty(),
                "the card names the canonical it links to"
            );
            card.click().await?;

            journey
                .text_becoming(
                    By::Css("h1"),
                    StringMatch::new(canonical.clone()).full(),
                    "the detail screen to head with the canonical the card named",
                )
                .await;
            journey
                .element(By::Css(CAPABILITY_PANE), "the capability pane")
                .await;
            journey
                .element(By::Css(PUBLISHED_PANE), "the published-resource pane")
                .await;
            // The links row is drawn only for a system the capability
            // statement names, so finding it proves the canonical round-tripped
            // through the percent-encoded route segment.
            journey
                .element(By::Css(SYSTEM_TOOLS), "the links into the other screens")
                .await;

            let address = journey.address().await;
            assert!(
                address.contains("/ui/systems/"),
                "the screen is addressed under the systems route: `{address}`"
            );
            assert!(
                address.contains("fhir=r4b"),
                "the link carries the version the reader was on: `{address}`"
            );
            assert!(
                !address.contains("/ui/systems//"),
                "the canonical is one encoded segment, so it never splits: `{address}`"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// The address a card links to opens the same screen when it is loaded fresh.
///
/// A click and a typed address reach the router by different routes: the click
/// handler pushes the path after one `decodeURI` pass, while a fresh load is
/// read straight off `window.location`. The canonical is a percent-encoded
/// segment on both, so the shareable address has to be driven, not modelled.
#[tokio::test]
async fn the_address_a_card_links_to_opens_the_same_screen_when_it_is_loaded_fresh() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;

            let card = journey
                .element(By::Css(SYSTEM_LINK), "a code system link on the overview")
                .await;
            let canonical = card.text().await?;
            let address = card
                .prop("href")
                .await?
                .expect("an anchor resolves its own href");
            assert!(
                address.contains("/ui/systems/"),
                "the card links into the systems route: `{address}`"
            );

            // The server has never served this path, so this also drives the
            // single-page fallback that makes a deep link work at all.
            journey.reopen(&address).await;

            journey
                .text_becoming(
                    By::Css("h1"),
                    StringMatch::new(canonical.clone()).full(),
                    "the freshly loaded screen to head with the same canonical",
                )
                .await;
            journey
                .element(By::Css(SYSTEM_TOOLS), "the links into the other screens")
                .await;

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// Walks from the overview to a browse screen whose tree has a level drawn.
///
/// The card is chosen by the operator its version declares, so the walk knows
/// no code system. Where the concept the search answers first turns out to be
/// a leaf, the walk moves onto one of its parents, because a tree with no row
/// has no tab stop to press a key on.
async fn tree_with_a_level(journey: &Journey) -> WebDriverResult<()> {
    journey
        .element(
            By::XPath(WALKABLE_CARD),
            "a code system whose version declares the direct-child operator",
        )
        .await
        .click()
        .await?;
    journey
        .element(By::Css(BROWSE_LINK), "the link into the concept browser")
        .await
        .click()
        .await?;

    // An empty filter lists the first concepts the server answers, so the
    // search gives the tree an anchor with nothing typed into it.
    journey
        .element(By::Css(SEARCH_RESULT), "a concept the search answered")
        .await
        .click()
        .await?;
    journey
        .address_carrying("code=", "the address to carry the concept that was read")
        .await;

    // Waiting on the answered tree rather than on the busy indicator: the
    // indicator is gone the moment the empty walk before this concept
    // resolved, so a count taken then would read a level still in flight.
    journey
        .element(By::Css(TREE_SETTLED), "the first level of the tree")
        .await;
    if journey.count(By::Css(TREE_ROW)).await == 0 {
        journey
            .element(By::Css(PARENT_LINK), "a parent of this leaf concept")
            .await
            .click()
            .await?;
        journey
            .element(By::Css(TREE_ROW), "a tree row below the parent")
            .await;
    }
    Ok(())
}

/// The taxonomy tree opens a node and selects a concept from the keyboard
/// alone, and the address carries both moves.
///
/// This is the journey that proves the ARIA tree view pattern is implemented
/// rather than described: the tree keeps one tab stop, the right arrow opens
/// the node under it, `Enter` selects the concept, and both moves are
/// navigations, so a walk a reader made is a link they can share. Nothing here
/// names a code system. The card is picked by the operator its version
/// declares, which is the same fact the screen draws the tree from.
#[tokio::test]
async fn the_taxonomy_tree_is_walked_by_keyboard_alone() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;
            tree_with_a_level(&journey).await?;

            let stop = journey
                .element(By::Css(TAB_STOP), "the tree's one tab stop")
                .await;
            let row_id = stop
                .attr("id")
                .await?
                .expect("every tree row carries the id its focus is moved by");
            // The row's first span is the twist's `sr-only` word, not the code, so
            // the code is selected by the class that renders it.
            let row_code = stop.find(By::Css("span.font-mono")).await?.text().await?;
            assert!(
                !row_code.is_empty(),
                "the row names the code it draws, and the journey selects by it"
            );

            stop.send_keys(Key::Right).await?;
            let opened = journey
                .address_carrying(
                    "open=",
                    "the right arrow to open the node under the tab stop",
                )
                .await;
            assert!(
                opened.contains("code="),
                "opening a node leaves the concept being read alone: `{opened}`"
            );

            // The navigation redrew the tree, so the tab stop is read again
            // rather than carried across the render.
            journey
                .element(By::Css(TAB_STOP), "the tab stop after the node opened")
                .await
                .send_keys(Key::Enter)
                .await?;
            let selected = journey
                .address_carrying(
                    &format!("code={row_code}"),
                    "Enter to select the concept the tab stop was on",
                )
                .await;
            assert!(
                selected.contains("open="),
                "selecting a concept leaves the opened node open: `{selected}`"
            );

            let announced = journey
                .element(
                    By::Css(SELECTED_ROW),
                    "the row the tree announces as selected",
                )
                .await;
            assert_eq!(
                announced.attr("id").await?.as_deref(),
                Some(row_id.as_str()),
                "the row Enter selected is the row that held the tab stop"
            );
            assert_eq!(
                journey.count(By::Css(SELECTED_ROW)).await,
                1,
                "a tree announces one selected row, so a reader is never told of two"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}
/// The version comparison reads all four roots and shows where they differ.
///
/// The row this asserts on is the one the screen exists for: R4 and R4B
/// declare `$lookup` at the type level and R5 added the instance level
/// (<https://hl7.org/fhir/R5/codesystem-operation-lookup.html>). Every cell is
/// read from the capability statement the browser fetched from that root, so a
/// screen that rendered a remembered answer, or that blanked while one root
/// was still answering, fails here.
#[tokio::test]
async fn the_four_roots_are_compared_and_their_lookup_levels_differ() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;

            journey
                .element(
                    By::Css(VERSIONS_LINK),
                    "the header link onto the versions screen",
                )
                .await
                .click()
                .await?;

            journey
                .element(By::Css(COMPARISON), "the comparison the four reads fill")
                .await;
            let r4 = journey
                .text_becoming(
                    By::XPath(lookup_cell(1)),
                    "type",
                    "R4 to state the levels it answers $lookup at",
                )
                .await;
            let r5 = journey
                .text_becoming(
                    By::XPath(lookup_cell(3)),
                    StringMatch::new("instance").partial(),
                    "R5 to state the instance level R4 does not declare",
                )
                .await;
            assert_ne!(
                r4, r5,
                "the screen exists to show that two roots answer one operation differently"
            );
            assert_eq!(
                journey.count(By::Css("table")).await,
                2,
                "both comparison tables drew, so nothing blanked while the four reads settled"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}
