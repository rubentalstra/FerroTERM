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

/// A switcher link, by the version name a reader reads on it.
fn version_link(label: &str) -> String {
    format!("//nav[@aria-label='FHIR version']//a[normalize-space()='{label}']")
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
