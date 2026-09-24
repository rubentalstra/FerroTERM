// SPDX-License-Identifier: BUSL-1.1
//! Composing a local value set in the editor bundle, and what the reader
//! bundle does not carry.
//!
//! The shape is the one L-03 asks for: one include of a value set this server
//! already publishes, one include of the composer's own codes, a preview of
//! the unsaved definition, and a save. The published value set here is the
//! fixture the deployment loads from disk, which has no write path, so it also
//! stands for the national content the editor never offers to change.
//!
//! The write journeys sign in through the stub issuer against the deployment
//! that configures one, because a write needs a token the server will take.
//! The token lives in the page that holds it, so once signed in these journeys
//! move between screens by pressing links the router intercepts.

use thirtyfour::prelude::*;
use thirtyfour::stringmatch::StringMatch;

use crate::harness::Journey;
use crate::harness::SignedIn;
use crate::harness::choose;
use crate::harness::server;
use crate::harness::session;
use crate::harness::sign_in;
use crate::harness::signed_in;

/// The sidebar link into the composer.
const COMPOSER_LINK: &str = "//a[normalize-space()='Compose a value set']";

/// The canonical the round-trip journey composes under.
///
/// Synthetic: the repository distributes no terminology content, and each
/// journey writes the resource it then reads back. One canonical per journey,
/// because the journeys run beside each other against one server.
const CANONICAL: &str = "https://terminology.example/e2e-composed";

/// That canonical, percent-encoded into a query parameter.
const CANONICAL_PARAM: &str = "https%3A%2F%2Fterminology.example%2Fe2e-composed";

/// The canonical the keyboard journey composes under.
const KEYBOARD_CANONICAL: &str = "https://terminology.example/e2e-composed-keyboard";

/// The value set the fixture publishes, which the composer draws in whole.
const PUBLISHED: &str = "https://ferroterm.eu/fhir/ValueSet/e2e-taxonomy-all";

/// The code system the fixture publishes, which the own codes come from.
const SYSTEM: &str = "https://ferroterm.eu/fhir/CodeSystem/e2e-taxonomy";

/// That system, percent-encoded into a query parameter.
const SYSTEM_PARAM: &str = "https%3A%2F%2Fferroterm.eu%2Ffhir%2FCodeSystem%2Fe2e-taxonomy";

/// One code of that system, which the composer names in its second include.
const CODE: &str = "ca-leaf";

/// The canonical field of the composer.
const URL_FIELD: &str = "#compose-url";

/// The live region the screen announces every outcome in.
const REPORT: &str = "#composer-report";

/// The control that saves the whole value set.
const SAVE: &str = "//button[normalize-space()='Save this value set']";

/// The control that runs the preview.
const PREVIEW: &str = "//button[normalize-space()='Run the preview']";

/// The control that adds an include.
const ADD_INCLUDE: &str = "//button[normalize-space()='Add an include']";

/// Every clause's own panel, which is one fieldset per include or exclude.
const CLAUSES: &str = "fieldset legend";

/// The picker that adds a value set this root publishes, on the first clause.
const PUBLISHED_PICKER: &str = "select[id^='compose-add-vs-']";

/// The picker that names a clause's code system.
const SYSTEM_PICKER: &str = "select[id^='compose-system-']";

/// The search that finds a code in that system.
const CODE_SEARCH: &str = "input[id^='compose-search-']";

/// The notice a screen that cannot be saved carries.
const READ_ONLY: &str = "//p[contains(text(), 'read-only') or contains(text(), 'no permission') \
     or contains(text(), 'Sign in to save')]";

/// Picks `value` in the `index`-th control matching `selector`.
///
/// The option itself is pressed, which is what a reader does and what fires
/// the control's own `change`, so the journey drives the real widget rather
/// than writing a property on it.
async fn pick(journey: &Journey, selector: &str, index: usize, value: &str) -> WebDriverResult<()> {
    let controls = journey.all(By::Css(selector)).await?;
    let control = controls
        .get(index)
        .unwrap_or_else(|| panic!("the form draws a `{selector}` at position {index}"));
    let option = control
        .find(By::Css(format!("option[value='{value}']")))
        .await
        .unwrap_or_else(|error| panic!("`{selector}` has no option `{value}`: {error}"));
    option.click().await?;
    Ok(())
}

/// Signs in under `profile` and opens the composer from the sidebar.
///
/// The screen is reached by pressing the link rather than by loading its
/// address, because a load would drop the token the sign-in just put in the
/// page.
async fn open_composer(journey: &Journey, deployment: &SignedIn, profile: &str, tag: &str) {
    journey
        .element(By::Css("header a[href^='/ui']"), "the shell mark")
        .await;
    choose(journey, deployment, profile, tag).await;
    sign_in(journey, deployment).await;
    journey
        .element(By::XPath(COMPOSER_LINK), "the way to the composer")
        .await
        .click()
        .await
        .unwrap_or_else(|error| panic!("the composer link refused the press: {error}"));
    journey
        .element(By::Css(URL_FIELD), "the composer form")
        .await;
}

/// Composes the L-03 shape: a published value set, and one own code.
async fn compose(journey: &Journey, canonical: &str) -> WebDriverResult<()> {
    journey
        .element(By::Css(URL_FIELD), "the canonical field")
        .await
        .send_keys(canonical)
        .await?;

    // The first include draws the published value set in whole.
    journey
        .count_becoming(By::Css(PUBLISHED_PICKER), 1, "the first clause")
        .await;
    pick(journey, PUBLISHED_PICKER, 0, PUBLISHED).await?;
    journey
        .element(
            By::XPath("(//button[normalize-space()='Add'])[1]"),
            "the control that adds the picked value set",
        )
        .await
        .click()
        .await?;
    journey
        .count_becoming(
            By::Css("input[id^='compose-valueset-']"),
            1,
            "the value set the clause draws in",
        )
        .await;

    // The second include names one code of the fixture's own code system.
    journey
        .element(By::XPath(ADD_INCLUDE), "the control that adds an include")
        .await
        .click()
        .await?;
    journey
        .count_becoming(By::Css(SYSTEM_PICKER), 2, "the second clause")
        .await;
    pick(journey, SYSTEM_PICKER, 1, SYSTEM).await?;
    let searches = journey.all(By::Css(CODE_SEARCH)).await?;
    searches
        .get(1)
        .expect("the second clause carries its own search")
        .send_keys(format!("{CODE}\n"))
        .await?;
    journey
        .element(
            By::XPath(format!(
                "//li[contains(., '{CODE}')]//button[normalize-space()='Add']"
            )),
            "the search result for that code",
        )
        .await
        .click()
        .await?;
    journey
        .count_becoming(
            By::Css("input[id^='compose-display-']"),
            1,
            "the code the clause now names",
        )
        .await;
    Ok(())
}

/// Reads the value set back off the server and validates a code through it.
///
/// This is a fresh page load, so the page holds no token and the form opens
/// read-only: what it shows is the server's own answer, seen the way a reader
/// who never signed in sees it.
async fn the_definition_survives(
    journey: &Journey,
    base: &str,
    address: &str,
) -> WebDriverResult<()> {
    journey.reopen(address).await;
    journey
        .count_becoming(
            By::Css("input[id^='compose-valueset-']"),
            1,
            "the saved value set to be read back",
        )
        .await;
    let read = journey
        .element(By::Css(URL_FIELD), "the canonical field")
        .await
        .prop("value")
        .await?
        .unwrap_or_default();
    assert_eq!(read, CANONICAL, "the canonical came back as it was saved");
    let clauses = journey.count(By::Css(CLAUSES)).await;
    assert!(
        clauses >= 2,
        "both includes came back: {clauses} clause legends"
    );

    journey
        .reopen(&format!(
            "{base}/ui/editor/validate?fhir=r4b&on=valueset&url={CANONICAL_PARAM}&code={CODE}&system={SYSTEM_PARAM}"
        ))
        .await;
    let validated = journey
        .text_becoming(
            By::Css("main"),
            StringMatch::new("result").partial(),
            "$validate-code to answer through the saved value set",
        )
        .await;
    assert!(
        validated.contains("true"),
        "the code is in the value set the composer saved: `{validated}`"
    );
    Ok(())
}

/// A whole local value set is composed, previewed, saved, and read back.
#[tokio::test]
async fn a_value_set_over_a_published_one_is_composed_previewed_and_saved() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui/editor").await;
            open_composer(&journey, &deployment, "writer", "composes-a-value-set").await;
            compose(&journey, CANONICAL).await?;

            // The rules the server reads the definition by are on the screen.
            let rules = journey
                .element(
                    By::Css("section[aria-labelledby='compose-includes-heading']"),
                    "the includes",
                )
                .await
                .text()
                .await?;
            assert!(
                rules.contains("union of them all"),
                "the includes say they union: `{rules}`"
            );
            assert!(
                rules.contains("is in the value set named here"),
                "and the criteria inside one include say they intersect: `{rules}`"
            );

            journey
                .element(By::XPath(PREVIEW), "the control that runs the preview")
                .await
                .click()
                .await?;
            let counted = journey
                .text_becoming(
                    By::Css(REPORT),
                    StringMatch::new("selects").partial(),
                    "the preview to announce what the definition selects",
                )
                .await;
            assert!(
                counted.contains("concepts"),
                "the count is announced in the live region: `{counted}`"
            );
            let previewed = journey
                .element(
                    By::Css("section[aria-labelledby='compose-preview-heading']"),
                    "the preview",
                )
                .await
                .text()
                .await?;
            assert!(
                previewed.contains(CODE),
                "the code the composer named is in the expansion: `{previewed}`"
            );

            journey
                .element(By::XPath(SAVE), "the control that saves")
                .await
                .click()
                .await?;
            journey
                .text_becoming(
                    By::Css(REPORT),
                    StringMatch::new("Saved").partial(),
                    "the screen to announce the save",
                )
                .await;
            let address = journey
                .address_carrying(
                    "id=",
                    "the address to name the value set the server created",
                )
                .await;

            the_definition_survives(&journey, &deployment.base, &address).await?;

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// One include is composed with the keyboard alone.
///
/// Every control is reached by tabbing and operated by typing or by Enter, so
/// nothing on the screen is mouse-only (<https://www.w3.org/TR/WCAG22/#keyboard>).
#[tokio::test]
async fn one_include_is_composed_with_the_keyboard_alone() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui/editor").await;
            open_composer(
                &journey,
                &deployment,
                "writer",
                "composes-with-the-keyboard",
            )
            .await;

            journey.tab_to("compose-url", "the canonical field").await;
            journey.type_here(KEYBOARD_CANONICAL).await?;
            assert_eq!(
                journey.focused().await?,
                "compose-url",
                "typing leaves the focus where it was"
            );

            // The picker that adds a published value set is reached by tabbing
            // on from the canonical, and chosen by typing its name.
            journey
                .tab_to(
                    "compose-add-vs-0",
                    "the picker that adds a published value set",
                )
                .await;
            journey.type_here("Every concept").await?;
            let picked = journey
                .element(By::Css(PUBLISHED_PICKER), "the picker")
                .await
                .prop("value")
                .await?
                .unwrap_or_default();
            assert_eq!(
                picked, PUBLISHED,
                "typing the name of an option selects it, with no pointer"
            );

            journey.tab(1).await?;
            assert_eq!(
                journey.focused().await?,
                "the control that adds it",
                "the add control is the next tab stop"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// A reader who signed in without a write scope is offered no save, and the
/// content the server holds no writable record of opens read-only.
#[tokio::test]
async fn a_reader_without_the_scope_composes_nothing() {
    let Some(deployment) = signed_in() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &deployment.base, "/ui/editor").await;
            open_composer(&journey, &deployment, "reader", "composes-nothing").await;

            let said = journey
                .element(By::XPath(READ_ONLY), "the notice that nothing can be saved")
                .await
                .text()
                .await?;
            assert!(
                said.contains("no permission"),
                "the reader is told why nothing can be saved: `{said}`"
            );
            assert_eq!(
                journey.count(By::XPath(SAVE)).await,
                0,
                "a reader with no write scope is offered no save"
            );

            // A national value set is the fixture this deployment loads from
            // disk, which the server holds no writable record of, so it opens
            // to read under every role.
            journey
                .reopen(&format!(
                    "{}/ui/editor/compose?fhir=r4b&id=e2e-taxonomy-all",
                    deployment.base
                ))
                .await;
            let disabled = journey
                .element(By::Css(URL_FIELD), "the canonical field")
                .await
                .prop("disabled")
                .await?
                .unwrap_or_default();
            assert_eq!(
                disabled, "true",
                "content the server serves from its loaded indexes opens read-only"
            );
            assert_eq!(
                journey.count(By::XPath(SAVE)).await,
                0,
                "and offers no save either"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}

/// The composer is not in the reader bundle at all.
///
/// The deployment this drives publishes no issuer, so it also shows that the
/// editor bundle without a token offers nothing to save.
#[tokio::test]
async fn the_reader_bundle_carries_no_composer() {
    let Some(base) = server() else {
        return;
    };
    let outcome = session()
        .await
        .run_and_quit(|driver| async move {
            let journey = Journey::open(driver, &base, "/ui").await;
            journey
                .element(By::Css("header a[href^='/ui']"), "the shell mark")
                .await;
            assert_eq!(
                journey.count(By::XPath(COMPOSER_LINK)).await,
                0,
                "the reader bundle's sidebar leads to no authoring screen"
            );

            journey.reopen(&format!("{base}/ui/editor/compose")).await;
            journey
                .element(By::Css(URL_FIELD), "the composer in the editor bundle")
                .await;
            assert_eq!(
                journey.count(By::XPath(SAVE)).await,
                0,
                "without a token the composer offers no save"
            );

            journey.no_console_errors().await;
            Ok::<(), WebDriverError>(())
        })
        .await;
    outcome.expect("the journey ran and the browser session ended cleanly");
}
